//! 敏感名称策略合同。

use lj_rule_model::{RequestHeaderDisposition, SensitiveNamePolicy};

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
