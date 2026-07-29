//! Legado 书源到 `RuleDefinition` 的来源适配器。
//!
//! Legado JSON、选择器、探索脚本和可继续动作格式全部由本模块持有。共享 Definition、
//! runtime 与 Tauri 只会看到标准 intent、受控 Flow、媒体 Delta 和不透明动作载荷。

pub(crate) mod parser;
pub mod translator;
pub mod types;

use std::fmt;

use lj_capability::IntentInput;
use lj_rule_model::{DiagnosticSeverity, SensitiveNamePolicy, canonical_json};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ImportError, ImportFormat, ImportedNativeRule};

/// `ContinueAction` 不透明载荷当前的 schema 版本。
pub const CONTINUE_ACTION_SCHEMA_VERSION: u32 = 1;
/// `ContinueAction` 默认有效期，过期载荷不得启动新的 HTTP effect。
pub const CONTINUE_ACTION_TTL_MS: i64 = 15 * 60 * 1_000;
const LEGADO_SOURCE_PREFIX: &str = "source:legado:";
const CONTINUE_ACTION_KIND: &str = "legado.explore";

/// Legado `ContinueAction` 的安全验证失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinueActionError {
    /// 输入不是版本化 opaque action object。
    InputInvalid,
    /// payload schema 版本不受支持。
    SchemaUnsupported,
    /// payload 属于另一个已安装来源。
    SourceMismatch,
    /// payload 的 action identity 与其受限 state 不匹配。
    IdentityMismatch,
    /// payload 已超过有效期。
    Expired,
    /// payload 完整性摘要与声明内容不一致。
    IntegrityInvalid,
    /// URL/state 为空、超长或包含敏感字段。
    StateInvalid,
}

impl ContinueActionError {
    /// 返回可用于 `RuleSystem` 诊断的稳定错误代码。
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InputInvalid => "continue_action_input_invalid",
            Self::SchemaUnsupported => "continue_action_schema_unsupported",
            Self::SourceMismatch => "continue_action_source_mismatch",
            Self::IdentityMismatch => "continue_action_identity_mismatch",
            Self::Expired => "continue_action_expired",
            Self::IntegrityInvalid => "continue_action_integrity_invalid",
            Self::StateInvalid => "continue_action_state_invalid",
        }
    }

    /// 返回不会回显来源 opaque state 的安全错误消息。
    #[must_use]
    pub const fn safe_message(self) -> &'static str {
        match self {
            Self::InputInvalid => "继续动作载荷格式无效",
            Self::SchemaUnsupported => "继续动作载荷版本不受支持",
            Self::SourceMismatch => "继续动作不属于请求来源",
            Self::IdentityMismatch => "继续动作身份与状态不匹配",
            Self::Expired => "继续动作已过期",
            Self::IntegrityInvalid => "继续动作完整性校验失败",
            Self::StateInvalid => "继续动作状态无效",
        }
    }
}

impl fmt::Display for ContinueActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.safe_message())
    }
}

impl std::error::Error for ContinueActionError {}

/// Legado 书源 Definition adapter。
pub struct LegadoImporter;

impl LegadoImporter {
    /// 严格解析 Legado JSON，并单向转换为原生规则。
    ///
    /// # Errors
    ///
    /// 输入超限、JSON/字段重复、已知行为不可执行、credential header 或映射无效时返回
    /// [`ImportError`]。错误和诊断永不包含输入片段或 credential value。
    pub fn import(&self, text: &str) -> Result<ImportedNativeRule, ImportError> {
        let outcome = crate::strict_json::analyze_legado_document(text);
        if outcome
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            return Err(ImportError::new(outcome.diagnostics));
        }
        let source = serde_json::from_str::<types::LegadoSourceJson>(text)
            .map_err(|_| ImportError::blocked("legado_field_invalid", text.len().min(1)))?;
        let headers = translator::parse_headers(source.header.as_deref())
            .map_err(|_| ImportError::blocked("legado_header_invalid", text.len().min(1)))?;
        let definition = translator::definition(&source, headers.safe)
            .map_err(|_| ImportError::blocked("legado_definition_invalid", text.len().min(1)))?;
        let credential_bytes = if headers.credentials.is_empty() {
            None
        } else {
            Some(serde_json::to_vec(&headers.credentials).map_err(|_| {
                ImportError::blocked("legado_credentials_invalid", text.len().min(1))
            })?)
        };
        let display_title = non_empty(&source.book_source_name);
        let display_group = source.book_source_group.as_deref().and_then(non_empty);
        Ok(ImportedNativeRule::new(
            definition,
            display_title,
            display_group,
            outcome.diagnostics,
            ImportFormat::Legado,
            text,
            credential_bytes,
        ))
    }

    /// 判断稳定来源身份是否由此 adapter 持有。
    #[must_use]
    pub fn owns_source(source_identity: &str) -> bool {
        source_identity.starts_with(LEGADO_SOURCE_PREFIX)
    }

    /// 把 discover mapper 的 Legado 原始状态封装为有版本、来源归属和完整性摘要的动作载荷。
    ///
    /// # Errors
    ///
    /// 原始记录没有受支持的动作 URL，或 state 会泄露敏感字段时返回
    /// [`ContinueActionError`]。
    pub fn seal_continue_action_payload(
        raw_payload: &Value,
        source_identity: &str,
        issued_at_ms: i64,
    ) -> Result<Value, ContinueActionError> {
        if !Self::owns_source(source_identity) {
            return Err(ContinueActionError::SourceMismatch);
        }
        let url = action_target(raw_payload)?;
        let expires_at_ms = issued_at_ms
            .checked_add(CONTINUE_ACTION_TTL_MS)
            .ok_or(ContinueActionError::StateInvalid)?;
        let claims = ContinueActionClaims {
            schema_version: CONTINUE_ACTION_SCHEMA_VERSION,
            source_identity: source_identity.to_string(),
            action_identity: action_identity(source_identity, &url),
            expires_at_ms,
            state: ContinueActionState { url },
        };
        let integrity = claims_integrity(&claims)?;
        serde_json::to_value(ContinueActionEnvelope { claims, integrity })
            .map_err(|_| ContinueActionError::InputInvalid)
    }

    /// 验证并解封装一个来源所属的 ContinueAction，返回 runtime 可消费的受限 URL state。
    ///
    /// # Errors
    ///
    /// schema、来源、action identity、有效期、完整性或受限 state 任一不匹配时返回
    /// [`ContinueActionError`]；调用方不得把失败降级为任意 JSON 执行。
    pub fn consume_continue_action(
        input: &IntentInput,
        expected_source_identity: &str,
        now_ms: i64,
    ) -> Result<IntentInput, ContinueActionError> {
        if !Self::owns_source(expected_source_identity) {
            return Err(ContinueActionError::SourceMismatch);
        }
        let IntentInput::Opaque(payload) = input else {
            return Err(ContinueActionError::InputInvalid);
        };
        let envelope = serde_json::from_value::<ContinueActionEnvelope>(payload.clone())
            .map_err(|_| ContinueActionError::InputInvalid)?;
        if envelope.claims.schema_version != CONTINUE_ACTION_SCHEMA_VERSION {
            return Err(ContinueActionError::SchemaUnsupported);
        }
        if envelope.claims.source_identity != expected_source_identity {
            return Err(ContinueActionError::SourceMismatch);
        }
        if envelope.claims.expires_at_ms <= now_ms {
            return Err(ContinueActionError::Expired);
        }
        validate_action_target(&envelope.claims.state.url)?;
        if envelope.claims.action_identity
            != action_identity(expected_source_identity, &envelope.claims.state.url)
        {
            return Err(ContinueActionError::IdentityMismatch);
        }
        if envelope.integrity != claims_integrity(&envelope.claims)? {
            return Err(ContinueActionError::IntegrityInvalid);
        }
        Ok(IntentInput::Opaque(
            json!({ "url": envelope.claims.state.url }),
        ))
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinueActionEnvelope {
    #[serde(flatten)]
    claims: ContinueActionClaims,
    integrity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContinueActionClaims {
    schema_version: u32,
    source_identity: String,
    action_identity: String,
    expires_at_ms: i64,
    state: ContinueActionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinueActionState {
    url: String,
}

fn claims_integrity(claims: &ContinueActionClaims) -> Result<String, ContinueActionError> {
    let canonical = canonical_json(claims).map_err(|_| ContinueActionError::IntegrityInvalid)?;
    Ok(blake3::hash(canonical.as_bytes()).to_hex().to_string())
}

fn action_identity(source_identity: &str, url: &str) -> String {
    let digest = blake3::hash(format!("{CONTINUE_ACTION_KIND}:{source_identity}:{url}").as_bytes());
    format!("{CONTINUE_ACTION_KIND}:{}", digest.to_hex())
}

fn action_target(raw_payload: &Value) -> Result<String, ContinueActionError> {
    let object = raw_payload
        .as_object()
        .ok_or(ContinueActionError::StateInvalid)?;
    let url = ["url", "href", "key"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .ok_or(ContinueActionError::StateInvalid)?;
    validate_action_target(url)?;
    Ok(url.trim().to_string())
}

fn validate_action_target(url: &str) -> Result<(), ContinueActionError> {
    let trimmed = url.trim();
    if trimmed.is_empty()
        || trimmed.len() > 8_192
        || trimmed.contains(['\r', '\n'])
        || SensitiveNamePolicy::url_contains_sensitive_query_name(trimmed)
    {
        return Err(ContinueActionError::StateInvalid);
    }
    Ok(())
}
