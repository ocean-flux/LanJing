    use lj_rule_model::RULE_CONTRACT_SCHEMA_VERSION;

    #[test]
    fn non_current_plan_shape_is_serialization_error() {
        // schema=1 历史线性 Plan：顶层 kind + 未 tagged config，边为 [from,to] 二元组。
        let non_current_plan = serde_json::json!({
            "contract": "execution_plan",
            "schema_version": RULE_CONTRACT_SCHEMA_VERSION,
            "compiler_version": "non-current-storage-test@1",
            "definition_hash": "non-current-definition-hash",
            "plan_hash": "non-current-plan-hash",
            "nodes": [{
                "id": "00000000-0000-0000-0000-000000000001",
                "kind": "js",
                "config": {
                    "code": "1",
                    "output": "json"
                }
            }],
            "edges": [[
                "00000000-0000-0000-0000-000000000001",
                "00000000-0000-0000-0000-000000000002"
            ]],
            "intent_entries": {},
            "effects": [],
            "capability_requirements": []
        });
        let error = read_execution_plan_artifact(
            &serde_json::to_vec(&non_current_plan).expect("serialize non-current Plan fixture"),
        )
        .expect_err("non-current Plan must fail current typed ingest");

        assert!(matches!(error, StorageError::Serialization));
    }

    #[test]
    fn non_current_package_shape_is_serialization_error() {
        let non_current_package = serde_json::json!({
            "contract": "rule_package",
            "schema_version": RULE_CONTRACT_SCHEMA_VERSION,
            "source_identity": { "id": "source:non-current-package" },
            "version": "non-current-definition-hash",
            "definition": {
                "contract": "rule_definition",
                "schema_version": RULE_CONTRACT_SCHEMA_VERSION,
                "flow": {
                    "nodes": [{
                        "id": "00000000-0000-0000-0000-000000000001",
                        "kind": "Js",
                        "js_code": "return input"
                    }],
                    "edges": []
                }
            }
        });
        let error = read_rule_package_artifact(
            &serde_json::to_vec(&non_current_package)
                .expect("serialize non-current package fixture"),
        )
        .expect_err("non-current package must fail current typed ingest");
        assert!(matches!(error, StorageError::Serialization));
    }

    #[test]
    fn unknown_package_and_plan_versions_are_typed_schema_unsupported() {
        let unknown_version = u32::MAX;
        let package_error = read_rule_package_artifact(
            &serde_json::to_vec(&serde_json::json!({
                "contract": "rule_package",
                "schema_version": unknown_version
            }))
            .expect("serialize unknown package"),
        )
        .expect_err("unknown package version must fail");
        assert!(matches!(
            package_error,
            StorageError::ContractSchemaUnsupported {
                contract: lj_rule_model::SchemaContract::RulePackage,
                version,
            } if version == unknown_version
        ));

        let nested_package_error = read_rule_package_artifact(
            &serde_json::to_vec(&serde_json::json!({
                "contract": "rule_package",
                "schema_version": RULE_CONTRACT_SCHEMA_VERSION,
                "source_identity": { "id": "source:nested-unknown" },
                "version": "unknown-definition",
                "definition": {
                    "contract": "rule_definition",
                    "schema_version": unknown_version,
                    "flow": {
                        "nodes": [{
                            "id": "00000000-0000-0000-0000-000000000001",
                            "kind": "Js",
                            "js_code": "return input"
                        }],
                        "edges": []
                    }
                }
            }))
            .expect("serialize nested unknown package"),
        )
        .expect_err("unknown nested Definition version must fail");
        assert!(matches!(
            nested_package_error,
            StorageError::ContractSchemaUnsupported {
                contract: lj_rule_model::SchemaContract::RuleDefinition,
                version,
            } if version == unknown_version
        ));

        let plan_error = read_execution_plan_artifact(
            &serde_json::to_vec(&serde_json::json!({
                "contract": "execution_plan",
                "schema_version": unknown_version
            }))
            .expect("serialize unknown Plan"),
        )
        .expect_err("unknown Plan version must fail");
        assert!(matches!(
            plan_error,
            StorageError::ContractSchemaUnsupported {
                contract: lj_rule_model::SchemaContract::ExecutionPlan,
                version,
            } if version == unknown_version
        ));
    }
