//! effect witness、完整性校验与安全 hash。
//!
//! witness 只保留可安全持久化的 URL、IP、hash、时序和 `QuickJS` host-call 元数据。它永远
//! 不含 query、body、cookie、token 或 authorization；replay 会依赖这些字段严格绑定当前
//! Plan/script/input，任何篡改都必须硬失败。

use std::net::IpAddr;
use std::sync::Arc;

use blake3::Hasher;
use lj_rule_model::{EffectKind, HttpMethod, canonical_json};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::contracts::{EffectFailure, EffectInput, EffectOutput, QuickJsOutput};

/// HTTP DNS target 的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpDnsTargetKind {
    /// SSRF 防护已完成 DNS 解析并固定目标地址。
    PinnedDns,
    /// 目标本身是 IP literal，未发生名称解析。
    IpLiteral,
    /// 测试或显式直连路径仅观察到 host，未执行 DNS 解析。
    DirectHost,
}

/// 安全 HTTP 请求 witness。
///
/// `safe_url` 只能包含 scheme、host、port 与 path，禁止 query、fragment 与 userinfo。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestWitness {
    /// 实际请求方法。
    pub method: HttpMethod,
    /// 去除 query、fragment 与 userinfo 的请求 URL。
    pub safe_url: String,
    /// 非敏感 request header 的稳定摘要；cookie/token/authorization 不得出现。
    pub headers: Vec<HttpRequestHeaderWitness>,
    /// request body 的逻辑摘要；实际 bytes 由独立 capture material 交给 C2。
    pub body: Option<HttpRequestBodyWitness>,
}

/// 一个非敏感 HTTP request header 的安全摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestHeaderWitness {
    /// 小写、非敏感 header 名称。
    pub name: String,
    /// header 值的 BLAKE3 hash；不保存原始值。
    pub value_hash: String,
}

/// HTTP request body 的安全摘要。
///
/// 原始 bytes 不在 witness 中；live capture 将其通过独立 body/secret artifact material 移交
/// C2，witness 只绑定 artifact 应有的逻辑内容 hash。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestBodyWitness {
    /// request body 的 BLAKE3 hash。
    pub hash: String,
    /// request body 的字节长度。
    pub byte_len: u64,
}

/// 一个手动跟随的 HTTP redirect hop。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRedirectWitness {
    /// 重定向响应状态码。
    pub status: u16,
    /// 去敏后的来源 URL。
    pub from_url: String,
    /// 去敏后的目标 URL。
    pub to_url: String,
}

/// HTTP target 的安全 DNS/IP witness。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpDnsTargetWitness {
    /// 请求时观察到的 host，不含端口、userinfo 或 query。
    pub host: String,
    /// 已解析或 literal 的 IP 地址文本；不含端口。
    pub addresses: Vec<String>,
    /// 地址列表的产生方式。
    pub kind: HttpDnsTargetKind,
}

/// HTTP effect 的可持久化安全 witness。
///
/// 不记录 request/response body、query、cookie、token、authorization 或其他 secret header。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpEffectWitness {
    /// 初始请求的安全描述。
    pub request: HttpRequestWitness,
    /// 已实际跟随的 redirect hops。
    pub redirects: Vec<HttpRedirectWitness>,
    /// 每个实际 target 的 DNS/IP witness。
    pub dns_targets: Vec<HttpDnsTargetWitness>,
    /// HTTP effect 已发生后的安全失败类别；成功时为 `None`。
    pub error: Option<super::contracts::HttpEffectErrorKind>,
    /// 从 effect 开始到最终 response/body/error 完成的耗时。
    pub duration_ms: u64,
}

/// `QuickJS` 内由 runtime 提供的安全 host 调用记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuickJsHostCall {
    /// `Date.now()` 返回的 epoch milliseconds。
    Time {
        /// 返回给脚本的 UTC epoch milliseconds。
        epoch_millis: i64,
    },
    /// `Math.random()` 返回值的 IEEE-754 bit pattern。
    Random {
        /// 返回给脚本的随机值 bit pattern。
        value_bits: u64,
    },
}

/// `QuickJS` host 调用的原始发生顺序。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickJsHostCallWitness {
    /// 从一开始递增的调用序号。
    pub sequence: u32,
    /// 实际调用及其安全结果。
    pub call: QuickJsHostCall,
}

/// `QuickJS` effect 的可持久化安全 witness。
///
/// script、input 与 output 都只保留 BLAKE3 hash；原始源码、输入、输出和引擎错误文本不在
/// witness 中。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickJsEffectWitness {
    /// 编译后脚本源码的 BLAKE3 hash。
    pub script_hash: String,
    /// effect 输入的 BLAKE3 hash。
    pub input_hash: String,
    /// 类型化 `QuickJS` 输出的 BLAKE3 hash。
    pub output_hash: String,
    /// 已归档的脚本失败类别；成功时为 `None`。
    pub error: Option<super::contracts::QuickJsErrorKind>,
    /// `Date.now` / `Math.random` 等 runtime host 调用的发生序列。
    pub host_calls: Vec<QuickJsHostCallWitness>,
    /// 从 effect 开始到 worker 返回的耗时。
    pub duration_ms: u64,
}

/// 无外部调用 Extract effect 的完整性 witness。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractEffectWitness {
    /// 上游 HTTP 输出的 BLAKE3 hash。
    pub input_hash: String,
    /// 解析和提取的耗时。
    pub duration_ms: u64,
}

/// effect 的类型化安全 witness。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectWitness {
    /// HTTP 请求、redirect、DNS target 与 timing 证据。
    Http(HttpEffectWitness),
    /// `QuickJS` script/input/output/hash/host-call/timing 证据。
    QuickJs(QuickJsEffectWitness),
    /// Extract input/timing 证据。
    Extract(ExtractEffectWitness),
}

/// witness 或其绑定输出违反安全/完整性合同。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EffectWitnessError {
    /// 输出与 witness 的 effect 类型不一致。
    #[error("effect witness 与输出类型不一致")]
    KindMismatch,
    /// HTTP witness URL 含 query、fragment、userinfo 或其他不安全格式。
    #[error("HTTP witness URL 不安全")]
    UnsafeHttpUrl,
    /// DNS target host/address 不符合安全文本约束。
    #[error("HTTP DNS witness 无效")]
    InvalidDnsTarget,
    /// `QuickJS` host 调用序号不连续。
    #[error("QuickJS host 调用序列无效")]
    InvalidHostCallSequence,
    /// hash 为空、不是 BLAKE3 hex，或无法 canonicalize。
    #[error("effect witness hash 无效")]
    InvalidHash,
    /// `QuickJS` witness output hash 与类型化输出不一致。
    #[error("QuickJS witness 输出 hash 不匹配")]
    QuickJsOutputHashMismatch,
}

impl EffectWitness {
    /// 返回此 witness 归属的 effect 类型。
    #[must_use]
    pub fn kind(&self) -> EffectKind {
        match self {
            Self::Http(_) => EffectKind::Http,
            Self::QuickJs(_) => EffectKind::QuickJs,
            Self::Extract(_) => EffectKind::Extract,
        }
    }

    /// 计算 witness 的 canonical BLAKE3 hash。
    ///
    /// # Errors
    ///
    /// witness 无法 canonicalize 时返回 [`EffectWitnessError::InvalidHash`]。
    pub fn canonical_hash(&self) -> Result<String, EffectWitnessError> {
        canonical_hash(self)
    }

    /// 校验 witness 的安全字段与内部结构。
    ///
    /// # Errors
    ///
    /// URL 含敏感部分、DNS target 无效、QuickJS hash/host-call 序列损坏时返回
    /// [`EffectWitnessError`]。
    pub fn validate(&self) -> Result<(), EffectWitnessError> {
        match self {
            Self::Http(witness) => validate_http_witness(witness),
            Self::QuickJs(witness) => validate_quickjs_witness(witness),
            Self::Extract(witness) => validate_hash(&witness.input_hash),
        }
    }
}

/// archive material 的敏感性。
///
/// request body 默认视为 secret；只有未来拥有明确 proven-safe 合同的 adapter 才可新增更低
/// 敏感性变体。C2 必须据此选择 Secret Artifact，而不是猜测 body 内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCaptureMaterialSensitivity {
    /// 原始 bytes 必须写入加密 Secret Artifact。
    Secret,
}

/// 不进入 witness 的 effect capture material。
///
/// 原始 HTTP request body 仅在 live handler → runtime → C2 archive 的短暂路径上传递。它不实现
/// serde，且默认被标记为 [`EffectCaptureMaterialSensitivity::Secret`]，禁止明文 Body Artifact。
#[derive(Clone, Default, PartialEq, Eq)]
pub struct EffectCaptureMaterial {
    request_body: Option<Arc<[u8]>>,
}

impl EffectCaptureMaterial {
    fn from_http_request_body(request_body: Option<Vec<u8>>) -> Self {
        Self {
            request_body: request_body.map(Arc::from),
        }
    }

    /// 返回仅供 archive 写入的原始 HTTP request body。
    #[must_use]
    pub fn request_body(&self) -> Option<&[u8]> {
        self.request_body.as_deref()
    }

    /// 返回 request body 的强制 archive 敏感性。
    #[must_use]
    pub fn request_body_sensitivity(&self) -> Option<EffectCaptureMaterialSensitivity> {
        self.request_body
            .as_ref()
            .map(|_| EffectCaptureMaterialSensitivity::Secret)
    }
}

/// 实际 handler 返回给 runtime 的类型化输出与安全 witness。
#[derive(Clone, PartialEq, Eq)]
pub struct CapturedEffectOutput {
    /// 可传给下游节点的类型化输出。
    pub output: EffectOutput,
    /// 与输出绑定、供 durable archive/replay 校验的安全 witness。
    pub witness: EffectWitness,
    capture_material: EffectCaptureMaterial,
}

impl CapturedEffectOutput {
    /// 创建 handler 已执行 effect 的输出/witness 对。
    #[must_use]
    pub fn new(output: EffectOutput, witness: EffectWitness) -> Self {
        Self {
            output,
            witness,
            capture_material: EffectCaptureMaterial::default(),
        }
    }

    /// 原始 bytes 不会进入 witness、tracing 或 delivery event；C2 archive 必须将其写入加密
    /// Secret Artifact，并且只在事件中保留 ref/hash。
    #[must_use]
    pub fn with_http_request_body(mut self, request_body: Option<Vec<u8>>) -> Self {
        self.capture_material = EffectCaptureMaterial::from_http_request_body(request_body);
        self
    }

    pub(crate) fn into_parts(self) -> (EffectOutput, EffectWitness, EffectCaptureMaterial) {
        (self.output, self.witness, self.capture_material)
    }

    /// 校验输出、witness 与 effect 类型之间的完整性关系。
    ///
    /// # Errors
    ///
    /// 输出/witness 类型不一致、witness 不安全，或 `QuickJS` output hash 不匹配时返回
    /// [`EffectWitnessError`]。
    pub fn validate(&self) -> Result<(), EffectWitnessError> {
        if self.output.kind() != self.witness.kind() {
            return Err(EffectWitnessError::KindMismatch);
        }
        self.witness.validate()?;
        match (&self.output, &self.witness) {
            (EffectOutput::Http(_), EffectWitness::Http(witness)) if witness.error.is_none() => {}
            (EffectOutput::QuickJs(output), EffectWitness::QuickJs(witness)) => {
                if witness.output_hash
                    != effect_output_hash(&EffectOutput::QuickJs(output.clone()))?
                {
                    return Err(EffectWitnessError::QuickJsOutputHashMismatch);
                }
                let expected_error = match output {
                    QuickJsOutput::Error(error) => Some(*error),
                    QuickJsOutput::Json(_) | QuickJsOutput::Raw(_) => None,
                };
                if witness.error != expected_error {
                    return Err(EffectWitnessError::QuickJsOutputHashMismatch);
                }
            }
            (
                EffectOutput::Extract(_) | EffectOutput::Failure(EffectFailure::Extract),
                EffectWitness::Extract(_),
            ) => {}
            (
                EffectOutput::Failure(EffectFailure::Http { error }),
                EffectWitness::Http(witness),
            ) if witness.error == Some(*error) => {}
            (
                EffectOutput::Failure(EffectFailure::QuickJs { error }),
                EffectWitness::QuickJs(witness),
            ) if witness.error == Some(*error) => {}
            _ => return Err(EffectWitnessError::KindMismatch),
        }
        if self.capture_material.request_body.is_some()
            && !matches!(
                self.output,
                EffectOutput::Http(_) | EffectOutput::Failure(EffectFailure::Http { .. })
            )
        {
            return Err(EffectWitnessError::KindMismatch);
        }
        Ok(())
    }
}

fn validate_http_witness(witness: &HttpEffectWitness) -> Result<(), EffectWitnessError> {
    validate_safe_http_url(&witness.request.safe_url)?;
    for header in &witness.request.headers {
        if !is_safe_header_name(&header.name) || is_sensitive_header_name(&header.name) {
            return Err(EffectWitnessError::InvalidHash);
        }
        validate_hash(&header.value_hash)?;
    }
    if let Some(body) = &witness.request.body {
        validate_hash(&body.hash)?;
    }
    for redirect in &witness.redirects {
        if !(300..400).contains(&redirect.status) {
            return Err(EffectWitnessError::UnsafeHttpUrl);
        }
        validate_safe_http_url(&redirect.from_url)?;
        validate_safe_http_url(&redirect.to_url)?;
    }
    for target in &witness.dns_targets {
        if target.host.trim().is_empty()
            || target.host.contains(['?', '#', '@', '\r', '\n'])
            || target.host.chars().any(char::is_whitespace)
            || target
                .addresses
                .iter()
                .any(|address| address.parse::<IpAddr>().is_err())
        {
            return Err(EffectWitnessError::InvalidDnsTarget);
        }
        if matches!(target.kind, HttpDnsTargetKind::IpLiteral) && target.addresses.len() != 1 {
            return Err(EffectWitnessError::InvalidDnsTarget);
        }
    }
    Ok(())
}

fn is_safe_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn is_sensitive_header_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "authorization" | "cookie" | "set-cookie" | "proxy-authorization"
    ) || name.contains("token")
        || name.contains("secret")
        || name.contains("api-key")
}

fn validate_quickjs_witness(witness: &QuickJsEffectWitness) -> Result<(), EffectWitnessError> {
    validate_hash(&witness.script_hash)?;
    validate_hash(&witness.input_hash)?;
    validate_hash(&witness.output_hash)?;
    for (index, call) in witness.host_calls.iter().enumerate() {
        if call.sequence
            != u32::try_from(index + 1).map_err(|_| EffectWitnessError::InvalidHostCallSequence)?
        {
            return Err(EffectWitnessError::InvalidHostCallSequence);
        }
    }
    Ok(())
}

fn validate_safe_http_url(url: &str) -> Result<(), EffectWitnessError> {
    let Some((scheme, remainder)) = url.split_once("://") else {
        return Err(EffectWitnessError::UnsafeHttpUrl);
    };
    let authority = remainder.split('/').next().unwrap_or_default();
    if !matches!(scheme, "http" | "https")
        || authority.is_empty()
        || authority.contains('@')
        || url.contains(['?', '#', '\r', '\n'])
    {
        return Err(EffectWitnessError::UnsafeHttpUrl);
    }
    Ok(())
}

fn validate_hash(hash: &str) -> Result<(), EffectWitnessError> {
    if hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(EffectWitnessError::InvalidHash)
    }
}

fn canonical_hash<T: Serialize>(value: &T) -> Result<String, EffectWitnessError> {
    let canonical = canonical_json(value).map_err(|_| EffectWitnessError::InvalidHash)?;
    let mut hasher = Hasher::new();
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

/// 计算类型化 effect 输出的 canonical BLAKE3 hash。
///
/// # Errors
///
/// 输出无法 canonicalize 时返回 [`EffectWitnessError::InvalidHash`]。
pub fn effect_output_hash(output: &EffectOutput) -> Result<String, EffectWitnessError> {
    canonical_hash(output)
}

/// 计算 effect 输入的安全 BLAKE3 hash。
///
/// 原始输入不会写入 witness；入口 intent 或已确认上游输出都只以 hash 表示。
///
/// # Errors
///
/// 输入无法 canonicalize 时返回 [`EffectWitnessError::InvalidHash`]。
pub fn effect_input_hash(input: &EffectInput) -> Result<String, EffectWitnessError> {
    match input {
        EffectInput::Intent(intent) => canonical_hash(intent),
        EffectInput::Output(output) => effect_output_hash(output),
    }
}

/// 计算编译后 `QuickJS` 源码的 BLAKE3 hash。
#[must_use]
pub fn quickjs_script_hash(code: &str) -> String {
    blake3::hash(code.as_bytes()).to_hex().to_string()
}

/// 计算任意非持久化值的 BLAKE3 hex。
///
/// 仅将结果 hash 用于 safe witness；调用方不得把原始 bytes 写入 tracing 或 delivery event。
#[must_use]
pub fn effect_bytes_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
