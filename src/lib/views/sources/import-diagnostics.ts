import { m } from '$lib/i18n';

const importDiagnosticMessages: Record<string, () => string> = {
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
  known_field_ignored: m.sources_diagnostic_known_field_ignored,
  known_field_blocked: m.sources_diagnostic_known_field_blocked,
  unknown_field: m.sources_diagnostic_unknown_field,
  credential_request_header_blocked: m.sources_diagnostic_credential_request_header_blocked,
  credential_query_blocked: m.sources_diagnostic_credential_query_blocked,
};

export function localizeImportDiagnostic(code: string): string {
  return Object.hasOwn(importDiagnosticMessages, code)
    ? importDiagnosticMessages[code]!()
    : m.sources_diagnostic_unknown();
}
