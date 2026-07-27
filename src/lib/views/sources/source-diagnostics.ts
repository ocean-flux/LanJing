import { m } from '$lib/i18n';
import type { AuthoringDiagnosticCode } from '$lib/rules/authoring';

type DiagnosticMessage = () => string;

const authoringDiagnosticMessages = {
  invalid_json: m.sources_diagnostic_invalid_json,
  invalid_root: m.sources_diagnostic_invalid_root,
  duplicate_key: m.sources_diagnostic_duplicate_key,
  document_bytes_exceeded: m.sources_diagnostic_document_bytes_exceeded,
  depth_exceeded: m.sources_diagnostic_depth_exceeded,
  node_count_exceeded: m.sources_diagnostic_node_count_exceeded,
  property_count_exceeded: m.sources_diagnostic_property_count_exceeded,
  property_name_utf8_bytes_exceeded: m.sources_diagnostic_property_name_utf8_bytes_exceeded,
  string_utf8_bytes_exceeded: m.sources_diagnostic_string_utf8_bytes_exceeded,
  required_field_missing: m.sources_diagnostic_required_field_missing,
  invalid_field_type: m.sources_diagnostic_invalid_field_type,
  known_field_preserved: m.sources_diagnostic_known_field_preserved,
  known_field_blocked: m.sources_diagnostic_known_field_blocked,
  unknown_field: m.sources_diagnostic_unknown_field,
  pointer_missing: m.sources_diagnostic_pointer_missing,
  pointer_ambiguous: m.sources_diagnostic_pointer_ambiguous,
  credential_schema_unsupported: m.sources_diagnostic_credential_schema_unsupported,
  credential_slot_duplicate: m.sources_diagnostic_credential_slot_duplicate,
  credential_sentinel_invalid: m.sources_diagnostic_credential_sentinel_invalid,
  credential_sentinel_missing: m.sources_diagnostic_credential_sentinel_missing,
  credential_sentinel_duplicate: m.sources_diagnostic_credential_sentinel_duplicate,
  credential_format_mismatch: m.sources_diagnostic_credential_format_mismatch,
  credential_owner_mismatch: m.sources_diagnostic_credential_owner_mismatch,
  credential_revision_mismatch: m.sources_diagnostic_credential_revision_mismatch,
  credential_path_mismatch: m.sources_diagnostic_credential_path_mismatch,
  credential_duplicate_sensitive_key: m.sources_diagnostic_credential_duplicate_sensitive_key,
  credential_request_header_blocked: m.sources_diagnostic_credential_request_header_blocked,
  credential_query_blocked: m.sources_diagnostic_credential_query_blocked,
  pointer_invalid: m.sources_diagnostic_pointer_invalid,
  epoch_stale: m.sources_diagnostic_epoch_stale,
  patch_operation_invalid: m.sources_diagnostic_patch_operation_invalid,
} satisfies Record<AuthoringDiagnosticCode, DiagnosticMessage>;

export function localizeSourceDiagnostic(code: string): string {
  if (!Object.hasOwn(authoringDiagnosticMessages, code)) {
    return m.sources_diagnostic_unknown();
  }
  return authoringDiagnosticMessages[code as AuthoringDiagnosticCode]();
}
