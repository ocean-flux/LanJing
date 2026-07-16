//! SSRF 防护 — 目标主机校验(KTD8)。
//!
//! 覆盖 `IPv4(RFC1918/环回/链路本地/云元数据/CGNAT/0.0.0.0/8)`
//! + `IPv6(::1/fe80::/fc00::/::ffff:0:0/IPv4-compatible)`。
//!
//! DNS rebinding 防护(hickory-resolver 异步 DNS + per-hop IP 固定):
//! - 自行 DNS 解析(异步,带 timeout)→ SSRF 校验 → IP 固定
//! - HTTP: IP 直连 URL,避免 TOCTOU 窗口(ADO-P0-2)
//! - HTTPS: `ClientBuilder::resolve` 固定 IP,保留 SNI 和证书验证
//!
//! # 与 HTTP 处理器的协作
//!
//! HTTP 处理器端使用 `Policy::none()` + 手动 redirect 循环,
//! 每跳重新调用 `validate_url_and_pin` 校验与 IP 固定。
//! HTTPS 每跳重建 client 并 `.resolve(host, pinned_addr)` 重新 pin,
//! 消除重定向 DNS rebinding TOCTOU 窗口。
//! DNS 解析超时由 `tokio::time::timeout` 外层兑底(防 hickory
//! `ResolverOpts::timeout` 某些场景不生效)。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::OnceLock;
use std::time::Duration;

use hickory_resolver::TokioResolver;
use lj_core::error::CoreError;

/// DNS 解析超时(外层 `tokio::time::timeout` 兑底,防 hickory `ResolverOpts::timeout`
/// 某些场景不生效,见 hickory-dns issue #1073)。
const DNS_TIMEOUT: Duration = Duration::from_secs(10);

/// 共享异步 DNS 解析器(系统配置 + tokio runtime)。
/// 用 `OnceLock` 延迟初始化,跨请求复用(含缓存)。初始化失败缓存错误
/// 并传播给调用方,避免 panic。
fn shared_resolver() -> Result<&'static TokioResolver, CoreError> {
    static RESOLVER: OnceLock<Result<TokioResolver, String>> = OnceLock::new();
    let cached = RESOLVER.get_or_init(|| {
        // `builder_tokio` 读系统 /etc/resolv.conf 或 Windows 配置,
        // 配合 tokio runtime 异步查询。失败(如系统 DNS 配置不可读)时
        // 缓存错误,后续调用复用同结果避免重复尝试。
        TokioResolver::builder_tokio()
            .map_err(|e| format!("hickory resolver 构建失败(系统 DNS 配置读取失败): {e}"))
            .map(hickory_resolver::ResolverBuilder::build)
    });
    cached
        .as_ref()
        .map_err(|msg| CoreError::SsrfBlocked(msg.clone()))
}

/// SSRF 校验后的目标信息,包含 DNS 解析结果用于防 rebinding。
#[derive(Debug, Clone)]
pub struct PinnedTarget {
    /// 请求 URL。
    /// HTTP: IP 直连 URL(如 `http://1.2.3.4:80/path?q=1`)
    /// HTTPS: 原始 URL(配合 `ClientBuilder::resolve` 使用)
    pub url: String,
    /// Host 请求头值(原始主机名,仅含显式端口)。
    pub host_header: String,
    /// DNS 解析到的地址列表。
    /// HTTP: 用于 URL 中的 IP 替换
    /// HTTPS: 用于 `ClientBuilder::resolve` 配置
    pub addrs: Vec<SocketAddr>,
}

/// 检查 IP 地址是否被 SSRF 防护阻断。
#[must_use]
pub fn is_blocked_ip(ip: &IpAddr) -> bool {
    match *ip {
        IpAddr::V4(v4) => is_blocked_ipv4(v4),
        IpAddr::V6(ref v6) => is_blocked_ipv6(v6),
    }
}

#[must_use]
fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback()
        || ip.is_link_local()
        || ip.is_private()
        || ip.is_unspecified()
        || ip.is_broadcast()
        // 0.0.0.0/8 — 未指定/本机地址范围(RFC 1122 Section 3.2.1.3)
        || octets[0] == 0
        // CGNAT 100.64.0.0/10（RFC 6598）
        || (octets[0] == 100 && (octets[1] & 0xc0) == 0x40)
        // AWS 元数据端点
        || octets == [169, 254, 169, 254]
        // 阿里云元数据端点
        || octets == [100, 100, 100, 200]
}

#[must_use]
fn is_blocked_ipv6(ip: &Ipv6Addr) -> bool {
    let segs = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || is_ipv6_link_local(ip)
        || is_ipv6_unique_local(ip)
        || is_ipv4_mapped_ipv6(ip)
        // IPv4-compatible IPv6 地址 ::a.b.c.d
        // (RFC 4291 Section 2.5.5.1, 已废弃但攻击面仍在)
        || (segs[0] == 0
            && segs[1] == 0
            && segs[2] == 0
            && segs[3] == 0
            && segs[4] == 0
            && segs[5] != 0xffff)
}

/// `fe80::/10` 链路本地 IPv6。
#[must_use]
const fn is_ipv6_link_local(ip: &Ipv6Addr) -> bool {
    let segs = ip.segments();
    (segs[0] & 0xffc0) == 0xfe80
}

/// `fc00::/7` 唯一本地 IPv6(ULA)。
#[must_use]
const fn is_ipv6_unique_local(ip: &Ipv6Addr) -> bool {
    let segs = ip.segments();
    (segs[0] & 0xfe00) == 0xfc00
}

/// `::ffff:0:0/96` IPv4-mapped IPv6 地址。
///
/// 提取内嵌的 IPv4 地址后委托 `is_blocked_ipv4` 校验，
/// 避免误伤公网 IPv4-mapped 地址。
#[must_use]
fn is_ipv4_mapped_ipv6(ip: &Ipv6Addr) -> bool {
    let segs = ip.segments();
    if segs[0] == 0
        && segs[1] == 0
        && segs[2] == 0
        && segs[3] == 0
        && segs[4] == 0
        && segs[5] == 0xffff
    {
        // 提取后 32 位作为 IPv4
        let v4 = Ipv4Addr::new(
            (segs[6] >> 8) as u8,
            (segs[6] & 0xff) as u8,
            (segs[7] >> 8) as u8,
            (segs[7] & 0xff) as u8,
        );
        return is_blocked_ipv4(v4);
    }
    false
}

/// 校验 URL 的目标主机安全并返回 `PinnedTarget`。
///
/// 1. 解析 URL 提取主机名
/// 2. DNS 解析主机名得到 IP 地址列表
/// 3. 任意一个 IP 被阻断则拒绝
/// 4. 返回 `PinnedTarget`:
///    - `url`: HTTP 用 IP 替换主机名(防 TOCTOU),HTTPS 保持原始 URL
///    - `host_header`: 原始主机名(含端口),用于设置 HTTP Host 请求头
///    - `addrs`: DNS 解析结果,HTTPS 用于 `ClientBuilder::resolve`
///
/// # Errors
///
/// 返回 `CoreError::SsrfBlocked` 当目标地址被 SSRF 防护阻断或 DNS 解析失败。
pub async fn validate_url_and_pin(url: &str) -> Result<PinnedTarget, CoreError> {
    let parsed =
        url::Url::parse(url).map_err(|e| CoreError::SsrfBlocked(format!("URL 解析失败: {e}")))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| CoreError::SsrfBlocked("URL 无有效主机".into()))?;

    let port: u16 = parsed.port_or_known_default().unwrap_or(80);

    // DNS 解析:用 hickory 异步解析防止 DNS rebinding (KTD8, ADO-P0-2)。
    // 外层 tokio::time::timeout 兑底(防 hickory ResolverOpts::timeout 不生效)。
    let resolver = shared_resolver()?;
    let lookup = tokio::time::timeout(DNS_TIMEOUT, resolver.lookup_ip(host))
        .await
        .map_err(|_| CoreError::SsrfBlocked(format!("DNS 解析超时: {host}")))?
        .map_err(|e| CoreError::SsrfBlocked(format!("DNS 解析失败: {e}")))?;

    let addrs: Vec<SocketAddr> = lookup.iter().map(|ip| SocketAddr::new(ip, port)).collect();

    if addrs.is_empty() {
        return Err(CoreError::SsrfBlocked("DNS 未解析到任何地址".into()));
    }

    // 校验所有解析出的 IP (负载均衡多 A 记录)
    for addr in &addrs {
        if is_blocked_ip(&addr.ip()) {
            return Err(CoreError::SsrfBlocked(format!(
                "目标地址被阻止: {host} ({})",
                addr.ip()
            )));
        }
    }

    // 构造 Host header(仅在显式指定端口时包含端口)
    let host_header = if let Some(explicit_port) = parsed.port() {
        format!("{host}:{explicit_port}")
    } else {
        host.to_string()
    };

    // HTTPS: 返回原始 URL + 地址列表,由调用方用 ClientBuilder::resolve 固定 IP
    if parsed.scheme() == "https" {
        return Ok(PinnedTarget {
            url: url.to_string(),
            host_header,
            addrs,
        });
    }

    // HTTP: 构造 IP 直连 URL 防 TOCTOU
    let ip = addrs[0].ip();
    let ip_str = match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => format!("[{v6}]"),
    };
    let mut pinned_url = format!("{}://{ip_str}:{port}", parsed.scheme());
    pinned_url.push_str(parsed.path());
    if let Some(query) = parsed.query() {
        pinned_url.push('?');
        pinned_url.push_str(query);
    }

    Ok(PinnedTarget {
        url: pinned_url,
        host_header,
        addrs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    // ── IPv4 阻断 ──────────────────────────────────────

    #[test]
    fn test_block_loopback_v4() {
        assert!(is_blocked_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"127.255.255.255".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_aws_metadata() {
        assert!(is_blocked_ip(&"169.254.169.254".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_alibaba_metadata() {
        assert!(is_blocked_ip(&"100.100.100.200".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_rfc1918() {
        assert!(is_blocked_ip(&"192.168.1.1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"10.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"172.16.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"172.31.255.255".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_link_local() {
        assert!(is_blocked_ip(&"169.254.1.1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_unspecified() {
        assert!(is_blocked_ip(&"0.0.0.0".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_zero_octet() {
        // 0.0.0.0/8 范围
        assert!(is_blocked_ip(&"0.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"0.255.255.255".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_cgnat() {
        // CGNAT 100.64.0.0/10
        assert!(is_blocked_ip(&"100.64.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"100.127.255.255".parse::<IpAddr>().unwrap()));
        // 100.128.0.0 是公网
        assert!(!is_blocked_ip(&"100.128.0.1".parse::<IpAddr>().unwrap()));
        // 100.63.255.255 不是 CGNAT
        assert!(!is_blocked_ip(&"100.63.255.255".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_broadcast() {
        assert!(is_blocked_ip(&"255.255.255.255".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_allow_public_v4() {
        assert!(!is_blocked_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));
        assert!(!is_blocked_ip(&"1.1.1.1".parse::<IpAddr>().unwrap()));
        assert!(!is_blocked_ip(
            &"114.114.114.114".parse::<IpAddr>().unwrap()
        ));
    }

    // ── IPv6 阻断 ──────────────────────────────────────

    #[test]
    fn test_block_loopback_v6() {
        assert!(is_blocked_ip(&"::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_unspecified_v6() {
        assert!(is_blocked_ip(&"::".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_link_local_v6() {
        assert!(is_blocked_ip(&"fe80::1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"feb0::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_unique_local_v6() {
        assert!(is_blocked_ip(&"fc00::1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"fd00::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_allow_public_v6() {
        assert!(!is_blocked_ip(
            &"2001:4860:4860::8888".parse::<IpAddr>().unwrap()
        ));
    }

    // ── IPv4-mapped IPv6 段 ─────────────────────────────

    #[test]
    fn test_block_ipv4_mapped_loopback() {
        assert!(is_blocked_ip(
            &"::ffff:127.0.0.1".parse::<IpAddr>().unwrap()
        ));
    }

    #[test]
    fn test_block_ipv4_mapped_private() {
        assert!(is_blocked_ip(
            &"::ffff:192.168.1.1".parse::<IpAddr>().unwrap()
        ));
    }

    #[test]
    fn test_allow_ipv4_mapped_public() {
        // 公网 IPv4-mapped 不应被误伤
        assert!(!is_blocked_ip(&"::ffff:8.8.8.8".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_ipv4_mapped_cgnat() {
        assert!(is_blocked_ip(
            &"::ffff:100.64.0.1".parse::<IpAddr>().unwrap()
        ));
    }

    // ── IPv4-compatible IPv6 段 ─────────────────────────

    #[test]
    fn test_block_ipv4_compatible() {
        // ::a.b.c.d 格式
        assert!(is_blocked_ip(&"::127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip(&"::192.168.1.1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_block_ipv4_compatible_zero() {
        // ::0.0.0.0 (unspecified 已在 is_unspecified 覆盖)
        assert!(is_blocked_ip(&"::0.0.0.0".parse::<IpAddr>().unwrap()));
    }

    // ── validate_url_and_pin 行为 ───────────────────────

    #[tokio::test]
    async fn test_validate_url_and_pin_blocked_host() {
        let result = validate_url_and_pin("http://127.0.0.1:8080/path").await;
        assert!(result.is_err(), "127.0.0.1 应被 SSRF 阻断");
    }

    #[tokio::test]
    async fn test_validate_url_and_pin_public() {
        let result = validate_url_and_pin("http://8.8.8.8/").await;
        assert!(result.is_ok(), "公网 IP 8.8.8.8 应通过 SSRF 校验");
        let target = result.unwrap();
        assert!(target.url.contains("8.8.8.8"), "pinned URL 应包含 IP");
        assert_eq!(target.host_header, "8.8.8.8", "Host header 应包含原始主机");
        assert!(!target.addrs.is_empty(), "应包含 DNS 解析地址");
    }

    #[tokio::test]
    async fn test_validate_url_and_pin_preserves_port() {
        let result = validate_url_and_pin("http://8.8.8.8:8080/path?q=1").await;
        assert!(result.is_ok());
        let target = result.unwrap();
        assert!(target.url.contains(":8080"), "pinned URL 应保留端口");
        assert_eq!(
            target.host_header, "8.8.8.8:8080",
            "Host header 应包含原始端口"
        );
    }

    #[tokio::test]
    async fn test_validate_url_and_pin_preserves_query() {
        let result = validate_url_and_pin("http://8.8.8.8/path?a=1&b=2").await;
        assert!(result.is_ok());
        let target = result.unwrap();
        assert!(
            target.url.contains("?a=1&b=2"),
            "pinned URL 应保留 query string"
        );
    }

    #[tokio::test]
    async fn test_validate_url_and_pin_dns_failure() {
        let result = validate_url_and_pin("http://nonexistent-domain-zzz.example/").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_url_and_pin_no_host() {
        let result = validate_url_and_pin("http:///path").await;
        assert!(result.is_err());
    }
}
