//! Candidate、source version 与 execution aggregate entities。

pub(crate) mod candidate {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "candidate_projection")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub candidate_id: String,
        pub candidate_schema_version: i64,
        pub runtime_credential_secret_id: Option<String>,
        pub target_source_identity: String,
        pub expected_installed_revision: i64,
        pub package_artifact_hash: String,
        pub plan_artifact_hash: String,
        pub definition_hash: String,
        pub plan_hash: String,
        pub profile_json: String,
        pub required_grant_json: String,
        pub diagnostics_json: String,
        pub expires_at_ms: i64,
        pub status: String,
        pub stream_version: i64,
        pub created_at_ms: i64,
        pub consumed_at_ms: Option<i64>,
        #[sea_orm(belongs_to, from = "runtime_credential_secret_id", to = "secret_id")]
        pub credential: BelongsTo<Option<super::super::artifact::secret_artifact::Entity>>,
    }
}

pub(crate) mod source {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "source_projection")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub source_identity: String,
        pub version: String,
        pub profile_json: String,
        pub grant_json: String,
        pub package_artifact_hash: String,
        pub plan_artifact_hash: String,
        pub definition_hash: String,
        pub plan_hash: String,
        pub cookie_namespace: String,
        pub runtime_credential_secret_id: Option<String>,
        pub revision: i64,
        pub updated_global_seq: i64,
        #[sea_orm(belongs_to, from = "runtime_credential_secret_id", to = "secret_id")]
        pub credential: BelongsTo<Option<super::super::artifact::secret_artifact::Entity>>,
    }
}

pub(crate) mod source_version {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "source_versions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub source_identity: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub source_revision: i64,
        pub version: String,
        pub profile_json: String,
        pub grant_json: String,
        pub base_url: String,
        pub package_artifact_hash: String,
        pub plan_artifact_hash: String,
        pub definition_hash: String,
        pub plan_hash: String,
        pub cookie_namespace: String,
        pub runtime_credential_secret_id: Option<String>,
        pub schema_version: i64,
        pub installed_at_ms: i64,
        #[sea_orm(belongs_to, from = "runtime_credential_secret_id", to = "secret_id")]
        pub credential: BelongsTo<Option<super::super::artifact::secret_artifact::Entity>>,
    }
}

pub(crate) mod execution {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "execution_projection")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub execution_id: String,
        pub source_identity: String,
        pub source_revision: i64,
        pub source_version: String,
        pub plan_hash: String,
        pub plan_artifact_hash: String,
        pub status: String,
        pub pinned: i64,
        pub archive_available: i64,
        pub gc_state: String,
        pub started_at_ms: i64,
        pub finished_at_ms: Option<i64>,
        pub revision: i64,
        pub updated_global_seq: i64,
        #[sea_orm(
            belongs_to,
            from = "(source_identity, source_revision)",
            to = "(source_identity, source_revision)"
        )]
        pub source: BelongsTo<super::source_version::Entity>,
    }
}
