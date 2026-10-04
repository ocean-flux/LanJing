//! 目标解析与 DNS pin — 把请求 URL 换成实际要连的固定地址。
//!
//! 本模块**不做目标限制**: 任意可解析的 http(s) 主机都可请求, 包括环回与私网地址
//! (产品决定, 风险由用户承担, 见 `SECURITY.md`)。
//!
//! 保留的不变量:
//! - http(s)-only 与「不含用户凭据」由 `request::safe_url` 在上游统一执行;
//! - 自行 DNS 解析(异步, 带 timeout)后固定 IP, 请求只发往真正解析出的地址
//!   (DNS rebinding / TOCTOU 防护, ADO-P0-2);
//! - HTTP: IP 直连 URL, 避免 TOCTOU 窗口;
//! - HTTPS: `ClientBuilder::resolve` 固定 IP, 保留 SNI 和证书验证。
//!
//! # 与 HTTP 处理器的协作
//!
//! HTTP 处理器端使用 `Policy::none()` + 手动 redirect 循环,
//! 每跳重新调用 `resolve_and_pin` 解析并固定 IP。
//! HTTPS 每跳重建 client 并 `.resolve(host, pinned_addr)` 重新 pin,
//! 消除重定向 DNS rebinding TOCTOU 窗口。
//! DNS 解析超时由 `tokio::time::timeout` 外层兜底(防 hickory
//! `ResolverOpts::timeout` 某些场景不生效, 见 hickory-dns issue #1073)。

use std::net::{IpAddr, SocketAddr};
use std::sync::OnceLock;
use std::time::Duration;

use hickory_resolver::TokioResolver;
use lj_rule_model::Error;

/// DNS 解析超时(外层 `tokio::time::timeout` 兜底, 防 hickory `ResolverOpts::timeout`
/// 某些场景不生效, 见 hickory-dns issue #1073)。
const DNS_TIMEOUT: Duration = Duration::from_secs(10);

/// 共享异步 DNS 解析器(系统配置 + tokio runtime)。
/// 用 `OnceLock` 延迟初始化, 跨请求复用(含缓存)。初始化失败缓存错误
/// 并传播给调用方, 避免 panic。
fn shared_resolver() -> Result<&'static TokioResolver, Error> {
    static RESOLVER: OnceLock<Result<TokioResolver, String>> = OnceLock::new();
    let cached = RESOLVER.get_or_init(|| {
        // `builder_tokio` 读系统 /etc/resolv.conf 或 Windows 配置,
        // 配合 tokio runtime 异步查询。失败(如系统 DNS 配置不可读)时
        // 缓存错误, 后续调用复用同结果避免重复尝试。
        TokioResolver::builder_tokio()
            .map_err(|e| format!("hickory resolver 构建失败(系统 DNS 配置读取失败): {e}"))
            .and_then(|builder| {
                builder
                    .build()
                    .map_err(|e| format!("hickory resolver 构建失败: {e}"))
            })
    });
    cached.as_ref().map_err(|msg| Error::Other(msg.clone()))
}

/// 解析并固定后的目标信息, 含 DNS 解析结果用于防 rebinding。
#[derive(Debug, Clone)]
pub struct PinnedTarget {
    /// 请求 URL。
    /// HTTP: IP 直连 URL(如 `http://1.2.3.4:80/path?q=1`)
    /// HTTPS: 原始 URL(配合 `ClientBuilder::resolve` 使用)
    pub url: String,
    /// Host 请求头值(原始主机名, 仅含显式端口)。
    pub host_header: String,
    /// DNS 解析到的地址列表。
    /// HTTP: 用于 URL 中的 IP 替换
    /// HTTPS: 用于 `ClientBuilder::resolve` 配置
    pub addrs: Vec<SocketAddr>,
}

/// 解析 URL 主机并固定地址, 返回 `PinnedTarget`。
///
/// 1. 解析 URL 提取主机名
/// 2. DNS 解析主机名得到 IP 地址列表
/// 3. 返回 `PinnedTarget`:
///    - `url`: HTTP 用 IP 替换主机名(防 TOCTOU), HTTPS 保持原始 URL
///    - `host_header`: 原始主机名(含端口), 用于设置 HTTP Host 请求头
///    - `addrs`: DNS 解析结果, HTTPS 用于 `ClientBuilder::resolve`
///
/// # Errors
///
/// URL 无法解析、没有主机名或 DNS 未解析到任何地址时返回 [`Error::Other`]。
pub async fn resolve_and_pin(url: &str) -> Result<PinnedTarget, Error> {
    let parsed = url::Url::parse(url).map_err(|e| Error::Other(format!("URL 解析失败: {e}")))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| Error::Other("URL 无有效主机".into()))?;
    // `Url::host_str` 对 IPv6 literal 返回带方括号的 `[::1]`(Host header 需要这个形态),
    // 而 hickory 只接受裸地址, 所以解析时取 `Host::Ipv6` 的地址文本。
    let resolver_host = match parsed.host() {
        Some(url::Host::Ipv6(address)) => address.to_string(),
        _ => host.to_string(),
    };

    let port: u16 = parsed.port_or_known_default().unwrap_or(80);

    // DNS 解析: 用 hickory 异步解析后固定 IP, 使请求只发往本次真正解析出的地址
    // (KTD8, ADO-P0-2)。IP literal 由 hickory 直接返回, 不产生网络查询。
    let resolver = shared_resolver()?;
    let lookup = tokio::time::timeout(DNS_TIMEOUT, resolver.lookup_ip(resolver_host.as_str()))
        .await
        .map_err(|_| Error::Other(format!("DNS 解析超时: {host}")))?
        .map_err(|e| Error::Other(format!("DNS 解析失败: {e}")))?;

    let addrs: Vec<SocketAddr> = lookup.iter().map(|ip| SocketAddr::new(ip, port)).collect();

    if addrs.is_empty() {
        return Err(Error::Other("DNS 未解析到任何地址".into()));
    }

    // 构造 Host header(仅在显式指定端口时包含端口)
    let host_header = if let Some(explicit_port) = parsed.port() {
        format!("{host}:{explicit_port}")
    } else {
        host.to_string()
    };

    // HTTPS: 返回原始 URL + 地址列表, 由调用方用 ClientBuilder::resolve 固定 IP
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

    // ── resolve_and_pin 行为 ────────────────────────────

    #[tokio::test]
    async fn test_resolve_and_pin_loopback() {
        // 环回与私网不再是目标限制: 只要求解析成功并固定地址。
        let result = resolve_and_pin("http://127.0.0.1:8080/path").await;
        assert!(result.is_ok(), "127.0.0.1 应可解析并固定");
        let target = result.unwrap();
        assert!(target.url.contains("127.0.0.1"), "pinned URL 应为 IP 直连");
        assert_eq!(target.host_header, "127.0.0.1:8080");
        assert_eq!(target.addrs.len(), 1);
    }

    #[tokio::test]
    async fn test_resolve_and_pin_private_and_metadata_ranges() {
        for host in [
            "http://10.0.0.1/",
            "http://192.168.1.1/",
            "http://169.254.169.254/",
            "http://[::1]:8080/",
        ] {
            let result = resolve_and_pin(host).await;
            assert!(result.is_ok(), "{host} 应可解析并固定: {:?}", result.err());
        }
    }

    #[tokio::test]
    async fn test_resolve_and_pin_public() {
        let result = resolve_and_pin("http://8.8.8.8/").await;
        assert!(result.is_ok(), "公网 IP 8.8.8.8 应可解析并固定");
        let target = result.unwrap();
        assert!(target.url.contains("8.8.8.8"), "pinned URL 应包含 IP");
        assert_eq!(target.host_header, "8.8.8.8", "Host header 应包含原始主机");
        assert!(!target.addrs.is_empty(), "应包含 DNS 解析地址");
    }

    #[tokio::test]
    async fn test_resolve_and_pin_preserves_port() {
        let result = resolve_and_pin("http://8.8.8.8:8080/path?q=1").await;
        assert!(result.is_ok());
        let target = result.unwrap();
        assert!(target.url.contains(":8080"), "pinned URL 应保留端口");
        assert_eq!(
            target.host_header, "8.8.8.8:8080",
            "Host header 应包含原始端口"
        );
    }

    #[tokio::test]
    async fn test_resolve_and_pin_preserves_query() {
        let result = resolve_and_pin("http://8.8.8.8/path?a=1&b=2").await;
        assert!(result.is_ok());
        let target = result.unwrap();
        assert!(
            target.url.contains("?a=1&b=2"),
            "pinned URL 应保留 query string"
        );
    }

    #[tokio::test]
    async fn test_resolve_and_pin_dns_failure() {
        let result = resolve_and_pin("http://nonexistent-domain-zzz.example/").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_and_pin_no_host() {
        let result = resolve_and_pin("http:///path").await;
        assert!(result.is_err());
    }
}
