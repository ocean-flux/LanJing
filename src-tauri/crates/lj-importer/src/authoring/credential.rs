//! 来源 credential split/mask/reconstitute codec。
//!
//! codec 通常只改 credential value token span；current-only Legado rebase 仅可新增一个根 `/header`
//! string property。slot 与 sentinel 严格绑定 format/document/revision/path；明文载体不实现 serde，
//! `Debug` 始终脱敏，任何 duplicate、owner/path/revision 失配都在恢复或 importer 前硬失败。

use std::collections::{HashMap, HashSet};
use std::fmt;

use lj_rule_model::{
    AuthoringDiagnostic, CREDENTIAL_SCHEMA_VERSION, CREDENTIAL_SENTINEL_PREFIX, CredentialSlot,
    CredentialSlotId, CredentialSlotManifest, CredentialTargetIdentity, DiagnosticSeverity,
    RequestHeaderDisposition, SensitiveNamePolicy, SourceDocumentFormat, SupportClass,
    credential_sentinel, parse_credential_sentinel,
};

use super::document::{
    JsonKind, ObjectProperty, ParsedDocument, Utf8ByteSpan, analyze_legado_document,
    locate_json_pointer, parse_strict_document, resolve_pointer,
};

/// 一个 slot 对应的明文 material。
///
/// 此类型不实现 serde；`Debug` 不显示 value。vault writer 应尽快把 `value` 交给加密边界。
pub struct CredentialSecret {
    slot_id: CredentialSlotId,
    value: String,
}

impl CredentialSecret {
    /// 从已解密 vault material 构造临时 secret。
    #[must_use]
    pub fn new(slot_id: CredentialSlotId, value: String) -> Self {
        Self { slot_id, value }
    }

    /// 返回关联 slot ID。
    #[must_use]
    pub const fn slot_id(&self) -> CredentialSlotId {
        self.slot_id
    }

    /// 显式借用明文；调用方不得写入 `Debug`、diagnostic、Event 或 tracing。
    #[must_use]
    pub fn expose_value(&self) -> &str {
        &self.value
    }

    /// 把临时载体交给加密 writer，避免额外复制。
    #[must_use]
    pub fn into_value(self) -> String {
        self.value
    }
}

impl fmt::Debug for CredentialSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialSecret")
            .field("slot_id", &self.slot_id)
            .field("value", &"<redacted>")
            .finish()
    }
}

/// split 后的 masked 原文、无 secret manifest 与临时明文集合。
pub struct CredentialSplit {
    /// 仅 credential value token 被 sentinel 替换的原文。
    pub masked_text: String,
    /// schema-v1 slot manifest。
    pub manifest: CredentialSlotManifest,
    /// 待进入加密 vault 的临时 material。
    pub secrets: Vec<CredentialSecret>,
}

impl fmt::Debug for CredentialSplit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialSplit")
            .field("masked_text_bytes", &self.masked_text.len())
            .field("target_format", &self.manifest.target.format)
            .field("revision", &self.manifest.target.revision)
            .field("slot_count", &self.manifest.slots.len())
            .field("secret_count", &self.secrets.len())
            .finish()
    }
}

/// 后端已按可信 base/current manifest 解析出的单一路径动作。
///
/// replacement 可能借用 credential plaintext，因此此类型故意不实现 `Debug`、`Clone` 或 serde。
enum CredentialRebaseAction<'a> {
    /// 把目标路径替换为后端选定的 current/local/replacement value。
    Replace(&'a str),
    /// 按来源格式清除目标 credential。
    Clear,
}

/// 一个 backend-derived credential path resolution。
///
/// `path` 必须来自可信 base/current manifest union；前端不能提供 slot owner。
pub struct CredentialRebaseResolution<'a> {
    path: &'a str,
    action: CredentialRebaseAction<'a>,
}

impl<'a> CredentialRebaseResolution<'a> {
    /// 构造一个不复制 plaintext 的 replacement resolution。
    #[must_use]
    pub const fn replace(path: &'a str, value: &'a str) -> Self {
        Self {
            path,
            action: CredentialRebaseAction::Replace(value),
        }
    }

    /// 构造一个 clear resolution。
    #[must_use]
    pub const fn clear(path: &'a str) -> Self {
        Self {
            path,
            action: CredentialRebaseAction::Clear,
        }
    }
}

/// rebase 后待进入 vault 的完整原文与重新 split 结果。
///
/// 完整原文可能含 credential plaintext，因此此类型故意不实现 `Debug`、`Clone` 或 serde。
pub struct CredentialRebase {
    raw_text: String,
    split: CredentialSplit,
}

impl CredentialRebase {
    /// 消费短生命周期结果，不复制 credential plaintext。
    #[must_use]
    pub fn into_parts(self) -> (String, CredentialSplit) {
        (self.raw_text, self.split)
    }
}

/// credential codec 的安全、可定位失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialCodecError {
    /// 复用 authoring diagnostic wire，不含明文。
    pub diagnostic: AuthoringDiagnostic,
}

impl fmt::Display for CredentialCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.diagnostic.code)
    }
}

impl std::error::Error for CredentialCodecError {}

/// Legado/Maccms10 共用的 schema-v1 slot codec。
pub struct CredentialSlotCodec;

impl CredentialSlotCodec {
    /// 按 target format 分离 credential，返回最小 token-span masked text。
    ///
    /// Legado 的 `header` 是嵌套 JSON string，因此 v1 把整个 `/header` value 绑定到一个 slot；
    /// Maccms10 endpoint 的 `/headers/<name>` 则逐 value 建 slot。非敏感原文、顺序与 whitespace
    /// 不变。
    ///
    /// # Errors
    ///
    /// 文档非法/超限/blocked、已有 sentinel、duplicate sensitive key、敏感 query，或 request
    /// header 只能用于 response/proxy hop 时返回 [`CredentialCodecError`]。
    pub fn split(
        text: &str,
        target: CredentialTargetIdentity,
    ) -> Result<CredentialSplit, CredentialCodecError> {
        validate_target_shape(&target)?;
        let mut candidates = match target.format {
            SourceDocumentFormat::Legado => {
                let outcome = analyze_legado_document(text);
                reject_blocking_diagnostics(&outcome.diagnostics)?;
                let document = outcome
                    .document
                    .ok_or_else(|| codec_error("invalid_json", "", 0, text.len().min(1)))?;
                reject_existing_sentinel(&document)?;
                collect_legado_credentials(&document)?
            }
            SourceDocumentFormat::Maccms10Endpoint => {
                let outcome = parse_strict_document(text);
                reject_blocking_diagnostics(&outcome.diagnostics)?;
                let document = outcome
                    .document
                    .ok_or_else(|| codec_error("invalid_json", "", 0, text.len().min(1)))?;
                reject_existing_sentinel(&document)?;
                collect_maccms_credentials(&document)?
            }
        };
        candidates.sort_by(|left, right| left.path.cmp(&right.path));
        let mut replacements = Vec::with_capacity(candidates.len());
        let mut slots = Vec::with_capacity(candidates.len());
        let mut secrets = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let slot_id = CredentialSlotId::new();
            let sentinel = credential_sentinel(slot_id);
            let replacement = serde_json::to_string(&sentinel).map_err(|_| {
                codec_error(
                    "credential_sentinel_invalid",
                    &candidate.path,
                    candidate.span.byte_offset,
                    candidate.span.byte_length,
                )
            })?;
            replacements.push((candidate.span, replacement));
            slots.push(CredentialSlot {
                schema_version: CREDENTIAL_SCHEMA_VERSION,
                slot_id,
                target: target.clone(),
                path: candidate.path,
                name: candidate.name,
            });
            secrets.push(CredentialSecret::new(slot_id, candidate.value));
        }
        let masked_text = apply_replacements(text, replacements)?;
        Ok(CredentialSplit {
            masked_text,
            manifest: CredentialSlotManifest {
                schema_version: CREDENTIAL_SCHEMA_VERSION,
                target,
                slots,
            },
            secrets,
        })
    }

    /// 校验 manifest/target/sentinel 唯一性并恢复完整作者原文。
    ///
    /// # Errors
    ///
    /// schema、format/document/revision/path、slot/secret 集合或 sentinel occurrence 任一不严格
    /// 匹配时返回 [`CredentialCodecError`]。
    pub fn reconstitute(
        masked_text: &str,
        manifest: &CredentialSlotManifest,
        secrets: &[CredentialSecret],
        expected_target: &CredentialTargetIdentity,
    ) -> Result<String, CredentialCodecError> {
        validate_manifest(manifest, expected_target)?;
        let outcome = parse_strict_document(masked_text);
        reject_blocking_diagnostics(&outcome.diagnostics)?;
        let document = outcome
            .document
            .ok_or_else(|| codec_error("invalid_json", "", 0, masked_text.len().min(1)))?;

        let mut slot_by_id = HashMap::with_capacity(manifest.slots.len());
        let mut paths = HashSet::with_capacity(manifest.slots.len());
        for slot in &manifest.slots {
            validate_slot(slot, &manifest.target)?;
            if slot_by_id.insert(slot.slot_id, slot).is_some() || !paths.insert(&slot.path) {
                return Err(codec_error("credential_slot_duplicate", &slot.path, 0, 0));
            }
        }
        let mut secret_by_id = HashMap::with_capacity(secrets.len());
        for secret in secrets {
            if secret_by_id.insert(secret.slot_id(), secret).is_some() {
                return Err(codec_error("credential_slot_duplicate", "", 0, 0));
            }
            if !slot_by_id.contains_key(&secret.slot_id()) {
                return Err(codec_error("credential_owner_mismatch", "", 0, 0));
            }
        }

        let occurrences = collect_sentinel_occurrences(&document)?;
        let mut occurrences_by_id: HashMap<CredentialSlotId, Vec<&SentinelOccurrence>> =
            HashMap::new();
        for occurrence in &occurrences {
            if !slot_by_id.contains_key(&occurrence.slot_id) {
                return Err(codec_error(
                    "credential_owner_mismatch",
                    &occurrence.path,
                    occurrence.span.byte_offset,
                    occurrence.span.byte_length,
                ));
            }
            occurrences_by_id
                .entry(occurrence.slot_id)
                .or_default()
                .push(occurrence);
        }

        let mut replacements = Vec::with_capacity(manifest.slots.len());
        for slot in &manifest.slots {
            let occurrences = occurrences_by_id
                .get(&slot.slot_id)
                .map_or(&[] as &[_], Vec::as_slice);
            if occurrences.is_empty() {
                return Err(codec_error("credential_sentinel_missing", &slot.path, 0, 0));
            }
            if occurrences.len() != 1 {
                let occurrence = occurrences[1];
                return Err(codec_error(
                    "credential_sentinel_duplicate",
                    &slot.path,
                    occurrence.span.byte_offset,
                    occurrence.span.byte_length,
                ));
            }
            let occurrence = occurrences[0];
            if occurrence.path != slot.path {
                return Err(codec_error(
                    "credential_path_mismatch",
                    &occurrence.path,
                    occurrence.span.byte_offset,
                    occurrence.span.byte_length,
                ));
            }
            let resolved = resolve_pointer(&document, &slot.path).map_err(|diagnostic| {
                codec_error(
                    "credential_path_mismatch",
                    &slot.path,
                    diagnostic.byte_offset,
                    diagnostic.byte_length,
                )
            })?;
            if resolved != occurrence.node {
                return Err(codec_error(
                    "credential_path_mismatch",
                    &slot.path,
                    occurrence.span.byte_offset,
                    occurrence.span.byte_length,
                ));
            }
            let secret = secret_by_id
                .get(&slot.slot_id)
                .ok_or_else(|| codec_error("credential_sentinel_missing", &slot.path, 0, 0))?;
            let replacement = serde_json::to_string(secret.expose_value()).map_err(|_| {
                codec_error(
                    "credential_sentinel_invalid",
                    &slot.path,
                    occurrence.span.byte_offset,
                    occurrence.span.byte_length,
                )
            })?;
            replacements.push((occurrence.span, replacement));
        }
        apply_replacements(masked_text, replacements)
    }
    /// 用旧 revision 的完整 slot 集合恢复原文，并为下一 revision 重新生成随机 slot/sentinel。
    ///
    /// # Errors
    ///
    /// 旧 target、manifest、secret 或 sentinel 不匹配，下一 revision 不连续，或重新 split 后
    /// 文档不再满足格式策略时返回 [`CredentialCodecError`]。
    pub fn resign(
        masked_text: &str,
        manifest: &CredentialSlotManifest,
        secrets: &[CredentialSecret],
        expected_target: &CredentialTargetIdentity,
        next_target: CredentialTargetIdentity,
    ) -> Result<CredentialSplit, CredentialCodecError> {
        validate_next_target(expected_target, &next_target)?;
        let text = Self::reconstitute(masked_text, manifest, secrets, expected_target)?;
        Self::split(&text, next_target)
    }

    /// 替换一个 owner-bound slot 的完整 secret，并为下一 revision 重新生成全部 sentinel。
    ///
    /// Legado 的单个 `/header` slot 接收完整 header JSON string；Maccms10 slot 接收对应 header
    /// value。`replacement` 只在当前调用栈内进入重组文本，随后立即由 [`Self::split`] 分离。
    ///
    /// # Errors
    ///
    /// slot 不属于当前 document/revision、旧文档无法严格恢复、replacement 违反格式 credential
    /// policy，或下一 revision 不连续时返回 [`CredentialCodecError`]。
    pub fn replace_credential(
        masked_text: &str,
        manifest: &CredentialSlotManifest,
        secrets: &[CredentialSecret],
        expected_target: &CredentialTargetIdentity,
        slot_id: CredentialSlotId,
        replacement: &str,
        next_target: CredentialTargetIdentity,
    ) -> Result<CredentialSplit, CredentialCodecError> {
        validate_next_target(expected_target, &next_target)?;
        let text = Self::reconstitute(masked_text, manifest, secrets, expected_target)?;
        let slot = manifest
            .slots
            .iter()
            .find(|slot| slot.slot_id == slot_id)
            .ok_or_else(|| codec_error("credential_owner_mismatch", "", 0, 0))?;
        let span = locate_json_pointer(&text, &slot.path)
            .map_err(|diagnostic| CredentialCodecError { diagnostic })?;
        let replacement = serde_json::to_string(&replacement).map_err(|_| {
            codec_error(
                "credential_replacement_invalid",
                &slot.path,
                span.byte_offset,
                span.byte_length,
            )
        })?;
        let text = apply_replacements(&text, vec![(span, replacement)])?;
        Self::split(&text, next_target)
    }

    /// 清除一个 owner-bound slot，并为下一 revision 重新生成仍存在的全部 sentinel。
    ///
    /// Legado v1 的 `/header` 是一个整体 secret，因此 clear 将它重置为空 JSON object string；
    /// Maccms10 只移除该 slot 对应的敏感 header property，其他 header 与原文布局保持不变。
    ///
    /// # Errors
    ///
    /// slot/target/sentinel 不匹配、Maccms property 无法唯一定位、下一 revision 不连续，或清除后
    /// 文档不再满足格式策略时返回 [`CredentialCodecError`]。
    pub fn clear_credential(
        masked_text: &str,
        manifest: &CredentialSlotManifest,
        secrets: &[CredentialSecret],
        expected_target: &CredentialTargetIdentity,
        slot_id: CredentialSlotId,
        next_target: CredentialTargetIdentity,
    ) -> Result<CredentialSplit, CredentialCodecError> {
        validate_next_target(expected_target, &next_target)?;
        let text = Self::reconstitute(masked_text, manifest, secrets, expected_target)?;
        let slot = manifest
            .slots
            .iter()
            .find(|slot| slot.slot_id == slot_id)
            .ok_or_else(|| codec_error("credential_owner_mismatch", "", 0, 0))?;
        let text = match expected_target.format {
            SourceDocumentFormat::Legado => {
                let span = locate_json_pointer(&text, &slot.path)
                    .map_err(|diagnostic| CredentialCodecError { diagnostic })?;
                let empty_header = serde_json::to_string("{}").map_err(|_| {
                    codec_error(
                        "credential_replacement_invalid",
                        &slot.path,
                        span.byte_offset,
                        span.byte_length,
                    )
                })?;
                apply_replacements(&text, vec![(span, empty_header)])?
            }
            SourceDocumentFormat::Maccms10Endpoint => {
                clear_maccms_header_property(&text, &slot.path)?
            }
        };
        Self::split(&text, next_target)
    }

    /// 在已恢复的 local 原文上按可信 path 应用 resolution，并为目标 owner 重新 split。
    ///
    /// 已存在的 credential 只改 value token；local 缺少 trusted Legado `/header` 时只在根对象
    /// 末尾增加一个 string property。Maccms clear 只移除一个 property，其余非敏感布局、未知字段、
    /// 顺序与 whitespace 均不重排或全量序列化。
    ///
    /// # Errors
    ///
    /// target format、支持路径/唯一性、本地 JSON、sentinel 或最终 credential policy 任一不匹配时
    /// 返回 [`CredentialCodecError`]。
    pub fn rebase_resolved_credentials(
        local_raw_text: &str,
        format: SourceDocumentFormat,
        resolutions: &[CredentialRebaseResolution<'_>],
        next_target: CredentialTargetIdentity,
    ) -> Result<CredentialRebase, CredentialCodecError> {
        validate_target_shape(&next_target)?;
        if next_target.format != format {
            return Err(codec_error("credential_owner_mismatch", "", 0, 0));
        }

        let mut seen_paths = HashSet::with_capacity(resolutions.len());
        for resolution in resolutions {
            if resolution.path.is_empty() || !seen_paths.insert(resolution.path) {
                return Err(codec_error(
                    "credential_resolution_duplicate",
                    resolution.path,
                    0,
                    0,
                ));
            }
            validate_rebase_resolution_path(format, resolution.path)?;
        }

        let outcome = parse_strict_document(local_raw_text);
        reject_blocking_diagnostics(&outcome.diagnostics)?;
        let document = outcome
            .document
            .ok_or_else(|| codec_error("invalid_json", "", 0, local_raw_text.len().min(1)))?;
        reject_existing_sentinel(&document)?;

        let mut raw_text = local_raw_text.to_string();
        for resolution in resolutions {
            match &resolution.action {
                CredentialRebaseAction::Replace(value) => {
                    raw_text =
                        replace_rebased_credential(&raw_text, format, resolution.path, value)?;
                }
                CredentialRebaseAction::Clear => match format {
                    SourceDocumentFormat::Legado => {
                        if let Some(cleared) = clear_legado_header(&raw_text)? {
                            raw_text = cleared;
                        }
                    }
                    SourceDocumentFormat::Maccms10Endpoint => {
                        raw_text = clear_maccms_header_property(&raw_text, resolution.path)?;
                    }
                },
            }
        }
        let split = Self::split(&raw_text, next_target)?;
        Ok(CredentialRebase { raw_text, split })
    }
}

fn validate_rebase_resolution_path(
    format: SourceDocumentFormat,
    path: &str,
) -> Result<(), CredentialCodecError> {
    match format {
        SourceDocumentFormat::Legado if path == "/header" => Ok(()),
        SourceDocumentFormat::Legado => Err(codec_error("credential_path_mismatch", path, 0, 0)),
        SourceDocumentFormat::Maccms10Endpoint => {
            let Some(encoded_name) = path.strip_prefix("/headers/") else {
                return Err(codec_error("credential_path_mismatch", path, 0, 0));
            };
            if encoded_name.is_empty() || encoded_name.contains('/') {
                return Err(codec_error("credential_path_mismatch", path, 0, 0));
            }
            let Some(name) = decode_pointer_tail(path) else {
                return Err(codec_error("credential_path_mismatch", path, 0, 0));
            };
            if escape_pointer_segment(&name) != encoded_name
                || SensitiveNamePolicy::request_header_disposition(&name)
                    != RequestHeaderDisposition::Credential
            {
                return Err(codec_error("credential_path_mismatch", path, 0, 0));
            }
            Ok(())
        }
    }
}

fn replace_rebased_credential(
    text: &str,
    format: SourceDocumentFormat,
    path: &str,
    value: &str,
) -> Result<String, CredentialCodecError> {
    let replacement = serde_json::to_string(value)
        .map_err(|_| codec_error("credential_replacement_invalid", path, 0, 0))?;
    match format {
        SourceDocumentFormat::Legado => replace_or_insert_legado_header(text, path, &replacement),
        SourceDocumentFormat::Maccms10Endpoint => {
            replace_existing_credential_value(text, path, &replacement)
        }
    }
}

fn replace_existing_credential_value(
    text: &str,
    path: &str,
    replacement: &str,
) -> Result<String, CredentialCodecError> {
    let span = locate_json_pointer(text, path).map_err(|diagnostic| {
        codec_error(
            "credential_path_mismatch",
            path,
            diagnostic.byte_offset,
            diagnostic.byte_length,
        )
    })?;
    apply_replacements(text, vec![(span, replacement.to_string())])
}

fn replace_or_insert_legado_header(
    text: &str,
    path: &str,
    replacement: &str,
) -> Result<String, CredentialCodecError> {
    let outcome = parse_strict_document(text);
    reject_blocking_diagnostics(&outcome.diagnostics)?;
    let document = outcome
        .document
        .ok_or_else(|| codec_error("invalid_json", "", 0, text.len().min(1)))?;
    if let Some(property) = unique_root_property(&document, "header")? {
        return apply_replacements(
            text,
            vec![(document.nodes[property.value].span, replacement.to_string())],
        );
    }
    insert_root_string_property(text, &document, "header", replacement, path)
}

fn clear_legado_header(text: &str) -> Result<Option<String>, CredentialCodecError> {
    let outcome = parse_strict_document(text);
    reject_blocking_diagnostics(&outcome.diagnostics)?;
    let document = outcome
        .document
        .ok_or_else(|| codec_error("invalid_json", "", 0, text.len().min(1)))?;
    let Some(property) = unique_root_property(&document, "header")? else {
        return Ok(None);
    };
    let span = document.nodes[property.value].span;
    let empty_header = serde_json::to_string("{}").map_err(|_| {
        codec_error(
            "credential_replacement_invalid",
            "/header",
            span.byte_offset,
            span.byte_length,
        )
    })?;
    apply_replacements(text, vec![(span, empty_header)]).map(Some)
}

fn insert_root_string_property(
    text: &str,
    document: &ParsedDocument,
    key: &str,
    value_token: &str,
    path: &str,
) -> Result<String, CredentialCodecError> {
    let root = &document.nodes[document.root];
    let JsonKind::Object(properties) = &root.kind else {
        return Err(codec_error(
            "credential_path_mismatch",
            path,
            root.span.byte_offset,
            root.span.byte_length,
        ));
    };
    if let Some(duplicate) = properties.iter().find(|property| property.key == key) {
        return Err(codec_error(
            "credential_path_mismatch",
            path,
            duplicate.key_span.byte_offset,
            duplicate.key_span.byte_length,
        ));
    }

    let root_end = root
        .span
        .byte_offset
        .checked_add(root.span.byte_length)
        .ok_or_else(|| codec_error("credential_path_mismatch", path, 0, 0))?;
    let Some(root_close) = root_end.checked_sub(1) else {
        return Err(codec_error("credential_path_mismatch", path, 0, 0));
    };
    if root_end > text.len()
        || text.as_bytes().get(root.span.byte_offset) != Some(&b'{')
        || text.as_bytes().get(root_close) != Some(&b'}')
    {
        return Err(codec_error(
            "credential_path_mismatch",
            path,
            root.span.byte_offset,
            root.span.byte_length,
        ));
    }

    let key_token = serde_json::to_string(key)
        .map_err(|_| codec_error("credential_replacement_invalid", path, 0, 0))?;
    if properties.is_empty() {
        let insertion_offset = root
            .span
            .byte_offset
            .checked_add(1)
            .ok_or_else(|| codec_error("credential_path_mismatch", path, 0, 0))?;
        let mut output = String::with_capacity(
            text.len()
                .saturating_add(key_token.len() + value_token.len() + 1),
        );
        output.push_str(&text[..insertion_offset]);
        output.push_str(&key_token);
        output.push(':');
        output.push_str(value_token);
        output.push_str(&text[insertion_offset..]);
        return Ok(output);
    }

    let last = properties
        .last()
        .ok_or_else(|| codec_error("credential_path_mismatch", path, 0, 0))?;
    let last_value = document.nodes[last.value].span;
    let insertion_offset = last_value
        .byte_offset
        .checked_add(last_value.byte_length)
        .ok_or_else(|| codec_error("credential_path_mismatch", path, 0, 0))?;
    let key_end = last
        .key_span
        .byte_offset
        .checked_add(last.key_span.byte_length)
        .ok_or_else(|| codec_error("credential_path_mismatch", path, 0, 0))?;
    if insertion_offset > root_close
        || key_end > last_value.byte_offset
        || !text.is_char_boundary(insertion_offset)
        || !text.is_char_boundary(key_end)
    {
        return Err(codec_error("credential_path_mismatch", path, 0, 0));
    }
    // 复用最后一个 property 的冒号两侧原始 bytes，避免把 ` : ` 局部改成 canonical `:`。
    let colon_style = &text[key_end..last_value.byte_offset];
    if !colon_style.as_bytes().contains(&b':') {
        return Err(codec_error("credential_path_mismatch", path, 0, 0));
    }

    let mut output = String::with_capacity(
        text.len()
            .saturating_add(key_token.len() + colon_style.len() + value_token.len() + 2),
    );
    output.push_str(&text[..insertion_offset]);
    // 多 property 复用末两项之间的 comma/newline/indent；单项则复用 `{` 后的首项缩进。
    if properties.len() >= 2 {
        let previous = &properties[properties.len() - 2];
        let previous_value = document.nodes[previous.value].span;
        let separator_start = previous_value
            .byte_offset
            .checked_add(previous_value.byte_length)
            .ok_or_else(|| codec_error("credential_path_mismatch", path, 0, 0))?;
        let separator_end = last.key_span.byte_offset;
        if separator_start > separator_end
            || separator_end > text.len()
            || !text.is_char_boundary(separator_start)
            || !text.is_char_boundary(separator_end)
        {
            return Err(codec_error("credential_path_mismatch", path, 0, 0));
        }
        let separator = &text[separator_start..separator_end];
        if !separator.as_bytes().contains(&b',') {
            return Err(codec_error("credential_path_mismatch", path, 0, 0));
        }
        output.push_str(separator);
    } else {
        let prefix_start = root
            .span
            .byte_offset
            .checked_add(1)
            .ok_or_else(|| codec_error("credential_path_mismatch", path, 0, 0))?;
        let prefix_end = last.key_span.byte_offset;
        if prefix_start > prefix_end
            || prefix_end > text.len()
            || !text.is_char_boundary(prefix_start)
            || !text.is_char_boundary(prefix_end)
        {
            return Err(codec_error("credential_path_mismatch", path, 0, 0));
        }
        output.push(',');
        output.push_str(&text[prefix_start..prefix_end]);
    }
    output.push_str(&key_token);
    output.push_str(colon_style);
    output.push_str(value_token);
    output.push_str(&text[insertion_offset..]);
    Ok(output)
}

struct SensitiveCandidate {
    path: String,
    name: String,
    value: String,
    span: Utf8ByteSpan,
}

struct SentinelOccurrence {
    slot_id: CredentialSlotId,
    path: String,
    node: usize,
    span: Utf8ByteSpan,
}

fn validate_target_shape(target: &CredentialTargetIdentity) -> Result<(), CredentialCodecError> {
    if target.document_id.trim().is_empty() {
        return Err(codec_error("credential_owner_mismatch", "", 0, 0));
    }
    Ok(())
}

fn validate_manifest(
    manifest: &CredentialSlotManifest,
    expected: &CredentialTargetIdentity,
) -> Result<(), CredentialCodecError> {
    if manifest.schema_version != CREDENTIAL_SCHEMA_VERSION {
        return Err(codec_error("credential_schema_unsupported", "", 0, 0));
    }
    validate_target_shape(expected)?;
    target_matches(&manifest.target, expected)
}

fn validate_slot(
    slot: &CredentialSlot,
    manifest_target: &CredentialTargetIdentity,
) -> Result<(), CredentialCodecError> {
    if slot.schema_version != CREDENTIAL_SCHEMA_VERSION {
        return Err(codec_error(
            "credential_schema_unsupported",
            &slot.path,
            0,
            0,
        ));
    }
    target_matches(&slot.target, manifest_target)?;
    match slot.target.format {
        SourceDocumentFormat::Legado => {
            if slot.path != "/header" || slot.name != "header" {
                return Err(codec_error("credential_path_mismatch", &slot.path, 0, 0));
            }
        }
        SourceDocumentFormat::Maccms10Endpoint => {
            let expected_prefix = "/headers/";
            if !slot.path.starts_with(expected_prefix)
                || SensitiveNamePolicy::request_header_disposition(&slot.name)
                    != RequestHeaderDisposition::Credential
            {
                return Err(codec_error("credential_path_mismatch", &slot.path, 0, 0));
            }
            let pointer_name = decode_pointer_tail(&slot.path)
                .ok_or_else(|| codec_error("credential_path_mismatch", &slot.path, 0, 0))?;
            if pointer_name != slot.name {
                return Err(codec_error("credential_path_mismatch", &slot.path, 0, 0));
            }
        }
    }
    Ok(())
}

fn target_matches(
    actual: &CredentialTargetIdentity,
    expected: &CredentialTargetIdentity,
) -> Result<(), CredentialCodecError> {
    if actual.format != expected.format {
        return Err(codec_error("credential_format_mismatch", "", 0, 0));
    }
    if actual.document_id != expected.document_id {
        return Err(codec_error("credential_owner_mismatch", "", 0, 0));
    }
    if actual.revision != expected.revision {
        return Err(codec_error("credential_revision_mismatch", "", 0, 0));
    }
    Ok(())
}

fn validate_next_target(
    current: &CredentialTargetIdentity,
    next: &CredentialTargetIdentity,
) -> Result<(), CredentialCodecError> {
    validate_target_shape(current)?;
    validate_target_shape(next)?;
    if current.format != next.format {
        return Err(codec_error("credential_format_mismatch", "", 0, 0));
    }
    if current.document_id != next.document_id {
        return Err(codec_error("credential_owner_mismatch", "", 0, 0));
    }
    let next_revision = current
        .revision
        .checked_add(1)
        .ok_or_else(|| codec_error("credential_revision_mismatch", "", 0, 0))?;
    if next.revision != next_revision {
        return Err(codec_error("credential_revision_mismatch", "", 0, 0));
    }
    Ok(())
}

fn clear_maccms_header_property(
    text: &str,
    slot_path: &str,
) -> Result<String, CredentialCodecError> {
    let outcome = parse_strict_document(text);
    reject_blocking_diagnostics(&outcome.diagnostics)?;
    let document = outcome
        .document
        .ok_or_else(|| codec_error("invalid_json", "", 0, text.len().min(1)))?;
    let headers = resolve_pointer(&document, "/headers")
        .map_err(|_| codec_error("credential_path_mismatch", slot_path, 0, 0))?;
    let target = resolve_pointer(&document, slot_path)
        .map_err(|_| codec_error("credential_path_mismatch", slot_path, 0, 0))?;
    let JsonKind::Object(properties) = &document.nodes[headers].kind else {
        return Err(codec_error("credential_path_mismatch", slot_path, 0, 0));
    };
    let position = properties
        .iter()
        .position(|property| property.value == target)
        .ok_or_else(|| codec_error("credential_path_mismatch", slot_path, 0, 0))?;
    let property = &properties[position];
    let value = document.nodes[property.value].span;
    let value_end = value
        .byte_offset
        .checked_add(value.byte_length)
        .ok_or_else(|| codec_error("credential_path_mismatch", slot_path, 0, 0))?;
    let (start, end) = if let Some(next) = properties.get(position + 1) {
        (property.key_span.byte_offset, next.key_span.byte_offset)
    } else if position > 0 {
        let previous = document.nodes[properties[position - 1].value].span;
        let previous_end = previous
            .byte_offset
            .checked_add(previous.byte_length)
            .ok_or_else(|| codec_error("credential_path_mismatch", slot_path, 0, 0))?;
        (previous_end, value_end)
    } else {
        (property.key_span.byte_offset, value_end)
    };
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return Err(codec_error("credential_path_mismatch", slot_path, 0, 0));
    }
    let mut cleared = String::with_capacity(text.len().saturating_sub(end - start));
    cleared.push_str(&text[..start]);
    cleared.push_str(&text[end..]);
    Ok(cleared)
}

fn collect_legado_credentials(
    document: &ParsedDocument,
) -> Result<Vec<SensitiveCandidate>, CredentialCodecError> {
    let Some(header) = unique_root_property(document, "header")? else {
        return Ok(Vec::new());
    };
    let node = &document.nodes[header.value];
    let JsonKind::String(value) = &node.kind else {
        return Err(codec_error(
            "invalid_field_type",
            "/header",
            node.span.byte_offset,
            node.span.byte_length,
        ));
    };
    let inner = parse_strict_document(value);
    let Some(inner_document) = inner.document else {
        return Err(codec_error(
            "invalid_field_value",
            "/header",
            node.span.byte_offset,
            node.span.byte_length,
        ));
    };
    if inner.diagnostics.iter().any(|diagnostic| {
        diagnostic.code != "duplicate_key" && diagnostic.severity == DiagnosticSeverity::Error
    }) {
        return Err(codec_error(
            "invalid_field_value",
            "/header",
            node.span.byte_offset,
            node.span.byte_length,
        ));
    }
    let JsonKind::Object(properties) = &inner_document.nodes[inner_document.root].kind else {
        return Err(codec_error(
            "invalid_field_value",
            "/header",
            node.span.byte_offset,
            node.span.byte_length,
        ));
    };
    let mut sensitive_names: Vec<&str> = Vec::new();
    let mut safe_names: Vec<&str> = Vec::new();
    let mut has_credential = false;
    for property in properties {
        let disposition = SensitiveNamePolicy::request_header_disposition(&property.key);
        let duplicate_sensitive = sensitive_names
            .iter()
            .any(|name| SensitiveNamePolicy::equivalent(name, &property.key));
        let duplicate_safe = safe_names.iter().any(|name| *name == property.key);
        match disposition {
            RequestHeaderDisposition::Public => {
                if duplicate_safe {
                    return Err(codec_error(
                        "duplicate_key",
                        "/header",
                        node.span.byte_offset,
                        node.span.byte_length,
                    ));
                }
                safe_names.push(&property.key);
            }
            RequestHeaderDisposition::Credential => {
                if duplicate_sensitive {
                    return Err(codec_error(
                        "credential_duplicate_sensitive_key",
                        "/header",
                        node.span.byte_offset,
                        node.span.byte_length,
                    ));
                }
                sensitive_names.push(&property.key);
                has_credential = true;
            }
            RequestHeaderDisposition::Blocked => {
                return Err(codec_error(
                    "credential_request_header_blocked",
                    "/header",
                    node.span.byte_offset,
                    node.span.byte_length,
                ));
            }
        }
        if !matches!(
            &inner_document.nodes[property.value].kind,
            JsonKind::String(_)
        ) {
            return Err(codec_error(
                "invalid_field_value",
                "/header",
                node.span.byte_offset,
                node.span.byte_length,
            ));
        }
    }
    if has_credential {
        Ok(vec![SensitiveCandidate {
            path: "/header".to_string(),
            name: "header".to_string(),
            value: value.clone(),
            span: node.span,
        }])
    } else {
        Ok(Vec::new())
    }
}

pub(crate) fn validate_legado_header(
    document: &ParsedDocument,
) -> Result<(), CredentialCodecError> {
    collect_legado_credentials(document).map(|_| ())
}

fn collect_maccms_credentials(
    document: &ParsedDocument,
) -> Result<Vec<SensitiveCandidate>, CredentialCodecError> {
    let Some(headers) = unique_root_property(document, "headers")? else {
        return Ok(Vec::new());
    };
    let node = &document.nodes[headers.value];
    let JsonKind::Object(properties) = &node.kind else {
        return Err(codec_error(
            "invalid_field_type",
            "/headers",
            node.span.byte_offset,
            node.span.byte_length,
        ));
    };
    let mut sensitive_names: Vec<&str> = Vec::new();
    let mut candidates = Vec::new();
    for property in properties {
        let disposition = SensitiveNamePolicy::request_header_disposition(&property.key);
        if disposition == RequestHeaderDisposition::Public {
            continue;
        }
        let path = format!("/headers/{}", escape_pointer_segment(&property.key));
        if sensitive_names
            .iter()
            .any(|name| SensitiveNamePolicy::equivalent(name, &property.key))
        {
            return Err(codec_error(
                "credential_duplicate_sensitive_key",
                &path,
                property.key_span.byte_offset,
                property.key_span.byte_length,
            ));
        }
        sensitive_names.push(&property.key);
        if disposition == RequestHeaderDisposition::Blocked {
            return Err(codec_error(
                "credential_request_header_blocked",
                &path,
                property.key_span.byte_offset,
                property.key_span.byte_length,
            ));
        }
        let value_node = &document.nodes[property.value];
        let JsonKind::String(value) = &value_node.kind else {
            return Err(codec_error(
                "invalid_field_type",
                &path,
                value_node.span.byte_offset,
                value_node.span.byte_length,
            ));
        };
        candidates.push(SensitiveCandidate {
            path,
            name: property.key.clone(),
            value: value.clone(),
            span: value_node.span,
        });
    }
    Ok(candidates)
}

fn unique_root_property<'a>(
    document: &'a ParsedDocument,
    key: &str,
) -> Result<Option<&'a ObjectProperty>, CredentialCodecError> {
    let JsonKind::Object(properties) = &document.nodes[document.root].kind else {
        return Err(codec_error("invalid_root", "", 0, 1));
    };
    let matching = properties
        .iter()
        .filter(|property| property.key == key)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [] => Ok(None),
        [property] => Ok(Some(*property)),
        [_, second, ..] => Err(codec_error(
            "duplicate_key",
            &format!("/{}", escape_pointer_segment(key)),
            second.key_span.byte_offset,
            second.key_span.byte_length,
        )),
    }
}

pub(crate) fn reject_existing_sentinel(
    document: &ParsedDocument,
) -> Result<(), CredentialCodecError> {
    let occurrences = collect_sentinel_occurrences(document)?;
    if let Some(occurrence) = occurrences.first() {
        return Err(codec_error(
            "credential_sentinel_invalid",
            &occurrence.path,
            occurrence.span.byte_offset,
            occurrence.span.byte_length,
        ));
    }
    Ok(())
}

fn collect_sentinel_occurrences(
    document: &ParsedDocument,
) -> Result<Vec<SentinelOccurrence>, CredentialCodecError> {
    let mut occurrences = Vec::new();
    let mut path = Vec::new();
    visit_sentinels(document, document.root, &mut path, &mut occurrences)?;
    Ok(occurrences)
}

fn visit_sentinels(
    document: &ParsedDocument,
    node_index: usize,
    path: &mut Vec<String>,
    occurrences: &mut Vec<SentinelOccurrence>,
) -> Result<(), CredentialCodecError> {
    let node = &document.nodes[node_index];
    match &node.kind {
        JsonKind::String(value) if value.starts_with(CREDENTIAL_SENTINEL_PREFIX) => {
            let slot_id = parse_credential_sentinel(value)
                .map_err(|_| {
                    codec_error(
                        "credential_sentinel_invalid",
                        &encode_pointer(path),
                        node.span.byte_offset,
                        node.span.byte_length,
                    )
                })?
                .ok_or_else(|| {
                    codec_error(
                        "credential_sentinel_invalid",
                        &encode_pointer(path),
                        node.span.byte_offset,
                        node.span.byte_length,
                    )
                })?;
            occurrences.push(SentinelOccurrence {
                slot_id,
                path: encode_pointer(path),
                node: node_index,
                span: node.span,
            });
        }
        JsonKind::Object(properties) => {
            for property in properties {
                path.push(property.key.clone());
                visit_sentinels(document, property.value, path, occurrences)?;
                path.pop();
            }
        }
        JsonKind::Array(children) => {
            for (index, child) in children.iter().enumerate() {
                path.push(index.to_string());
                visit_sentinels(document, *child, path, occurrences)?;
                path.pop();
            }
        }
        _ => {}
    }
    Ok(())
}

fn reject_blocking_diagnostics(
    diagnostics: &[AuthoringDiagnostic],
) -> Result<(), CredentialCodecError> {
    if let Some(diagnostic) = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(CredentialCodecError {
            diagnostic: diagnostic.clone(),
        });
    }
    Ok(())
}

fn apply_replacements(
    text: &str,
    mut replacements: Vec<(Utf8ByteSpan, String)>,
) -> Result<String, CredentialCodecError> {
    replacements.sort_by_key(|replacement| std::cmp::Reverse(replacement.0.byte_offset));
    let mut output = text.to_string();
    let mut previous_start = text.len();
    for (span, replacement) in replacements {
        let end = span
            .byte_offset
            .checked_add(span.byte_length)
            .ok_or_else(|| codec_error("credential_path_mismatch", "", 0, 0))?;
        if end > previous_start
            || end > output.len()
            || !output.is_char_boundary(span.byte_offset)
            || !output.is_char_boundary(end)
        {
            return Err(codec_error(
                "credential_path_mismatch",
                "",
                span.byte_offset,
                span.byte_length,
            ));
        }
        output.replace_range(span.byte_offset..end, &replacement);
        previous_start = span.byte_offset;
    }
    Ok(output)
}

fn codec_error(
    code: &str,
    path: &str,
    byte_offset: usize,
    byte_length: usize,
) -> CredentialCodecError {
    CredentialCodecError {
        diagnostic: AuthoringDiagnostic::new(
            DiagnosticSeverity::Error,
            code,
            path,
            byte_offset,
            byte_length,
            SupportClass::Blocked,
        ),
    }
}

fn escape_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn encode_pointer(path: &[String]) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut pointer = String::new();
    for segment in path {
        pointer.push('/');
        pointer.push_str(&escape_pointer_segment(segment));
    }
    pointer
}

fn decode_pointer_tail(pointer: &str) -> Option<String> {
    let tail = pointer.rsplit('/').next()?;
    let mut decoded = String::with_capacity(tail.len());
    let mut characters = tail.chars();
    while let Some(character) = characters.next() {
        if character != '~' {
            decoded.push(character);
            continue;
        }
        match characters.next()? {
            '0' => decoded.push('~'),
            '1' => decoded.push('/'),
            _ => return None,
        }
    }
    Some(decoded)
}
