//! authoring diagnostic、credential slot 与 sensitive policy wire 合同。

use lj_rule_model::{
    AuthoringDiagnostic, CREDENTIAL_SCHEMA_VERSION, CredentialSlot, CredentialSlotId,
    CredentialSlotManifest, CredentialTargetIdentity, DiagnosticSeverity, RequestHeaderDisposition,
    SensitiveNamePolicy, SourceDocumentFormat, SupportClass, credential_sentinel,
    parse_credential_sentinel,
};
use serde_json::json;

#[test]
fn authoring_diagnostic_wire_uses_exact_six_fields_and_utf8_span() {
    let diagnostic = AuthoringDiagnostic::new(
        DiagnosticSeverity::Warning,
        "unknown_field",
        "/未知~1键~0",
        17,
        14,
        SupportClass::Unknown,
    );
    assert_eq!(
        serde_json::to_value(diagnostic).expect("diagnostic must serialize"),
        json!({
            "severity": "warning",
            "code": "unknown_field",
            "path": "/未知~1键~0",
            "byte_offset": 17,
            "byte_length": 14,
            "support": "unknown"
        })
    );
}

#[test]
fn credential_slot_wire_binds_format_document_revision_and_path() {
    let target = CredentialTargetIdentity {
        format: SourceDocumentFormat::Maccms10Endpoint,
        document_id: "document-opaque-id".to_string(),
        revision: 7,
    };
    let slot_id = CredentialSlotId::new();
    let slot = CredentialSlot {
        schema_version: CREDENTIAL_SCHEMA_VERSION,
        slot_id,
        target: target.clone(),
        path: "/headers/Authorization".to_string(),
        name: "Authorization".to_string(),
    };
    let manifest = CredentialSlotManifest {
        schema_version: CREDENTIAL_SCHEMA_VERSION,
        target,
        slots: vec![slot],
    };
    let wire = serde_json::to_value(manifest).expect("manifest must serialize");
    assert_eq!(wire["schema_version"], 1);
    assert_eq!(wire["target"]["format"], "maccms10_endpoint");
    assert_eq!(wire["target"]["document_id"], "document-opaque-id");
    assert_eq!(wire["target"]["revision"], 7);
    assert_eq!(wire["slots"][0]["path"], "/headers/Authorization");
    let sentinel = credential_sentinel(slot_id);
    assert_eq!(parse_credential_sentinel(&sentinel), Ok(Some(slot_id)));
}

#[test]
fn sensitive_policy_has_one_normalized_request_and_response_contract() {
    for name in [
        "Authorization",
        "Cookie",
        "X_TOKEN",
        "client-secret",
        "X_API_KEY",
    ] {
        assert_eq!(
            SensitiveNamePolicy::request_header_disposition(name),
            RequestHeaderDisposition::Credential
        );
        assert!(SensitiveNamePolicy::is_sensitive_response_header(name));
    }
    for name in ["Proxy-Authorization", "proxy_authorization", "Set-Cookie"] {
        assert_eq!(
            SensitiveNamePolicy::request_header_disposition(name),
            RequestHeaderDisposition::Blocked
        );
        assert!(SensitiveNamePolicy::is_sensitive_response_header(name));
    }
    assert_eq!(
        SensitiveNamePolicy::request_header_disposition("User-Agent"),
        RequestHeaderDisposition::Public
    );
}
