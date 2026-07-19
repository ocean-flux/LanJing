//! execution-only HTTP 凭据与敏感 header 解码。
//!
//! 凭据只能在 live HTTP adapter 构造请求时短暂解码；它不实现 `Debug` 或 serde，不进入
//! Plan、effect fingerprint、事件、tracing 或 replay task。replay 启动前由 runtime 主动清空
//! 该字段，防止历史重放接触 live source secret。

use std::collections::BTreeMap;
use std::sync::Arc;

use thiserror::Error;

/// execution 期间使用的 HTTP 凭据载体。
#[derive(Clone, Default)]
pub struct HttpExecutionCredentials {
    cookie_namespace: String,
    secret_bytes: Option<Arc<[u8]>>,
}

impl HttpExecutionCredentials {
    /// 接管已解密的 source secret bytes，供单次 execution 的 live HTTP effect 使用。
    ///
    /// `secret_bytes` 的约定格式为 C5 写入的 JSON `BTreeMap<String, String>`；解析被延后
    /// 到 [`Self::decode_headers`]，从而保证 replay 不会读取或解码凭据。
    #[must_use]
    pub fn from_source_secret(cookie_namespace: String, secret_bytes: Option<Vec<u8>>) -> Self {
        Self {
            cookie_namespace,
            secret_bytes: secret_bytes.map(Arc::from),
        }
    }

    /// 返回 C2 为 pinned source version 分配的 cookie namespace。
    #[must_use]
    pub fn cookie_namespace(&self) -> &str {
        &self.cookie_namespace
    }

    /// 仅供 live HTTP adapter 解码敏感 header。
    ///
    /// # Errors
    ///
    /// secret 存在但 cookie namespace 为空，或 bytes 不是 C5 约定的 JSON string header map
    /// 时返回 [`HttpCredentialsError`]。错误不包含 secret 内容。
    pub fn decode_headers(&self) -> Result<SecretHttpHeaders, HttpCredentialsError> {
        let Some(secret_bytes) = &self.secret_bytes else {
            return Ok(SecretHttpHeaders::default());
        };
        if self.cookie_namespace.trim().is_empty() {
            return Err(HttpCredentialsError::MissingCookieNamespace);
        }
        let headers = serde_json::from_slice::<BTreeMap<String, String>>(secret_bytes)
            .map_err(|_| HttpCredentialsError::InvalidSecretEncoding)?;
        Ok(SecretHttpHeaders { headers })
    }
}

/// 临时解码后的敏感 HTTP header 集合。
///
/// 此类型同样不实现 `Debug` 或 serde；HTTP adapter 应仅在构造请求期间借用它。
#[derive(Default)]
pub struct SecretHttpHeaders {
    headers: BTreeMap<String, String>,
}

impl SecretHttpHeaders {
    /// 按稳定名称顺序迭代敏感 header，避免复制其值。
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.headers
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }
}

/// source secret 无法安全作为 execution HTTP header 使用时的错误。
#[derive(Debug, Error)]
pub enum HttpCredentialsError {
    /// secret 存在但 C2 提供的 cookie namespace 为空。
    #[error("source secret 缺少 cookie namespace")]
    MissingCookieNamespace,

    /// secret bytes 不符合 C5 约定的 JSON string header map。
    #[error("source secret header 编码无效")]
    InvalidSecretEncoding,
}
