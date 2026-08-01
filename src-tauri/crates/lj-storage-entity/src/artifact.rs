//! Content artifact 与 owner-bound secret metadata entities。

pub(crate) mod artifact_metadata {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "artifact_metadata")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub hash: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub artifact_kind: String,
        pub codec: String,
        pub hash_algorithm: String,
        pub relative_path: String,
        pub stored_bytes: i64,
        pub ref_count: i64,
        pub created_at_ms: i64,
    }
}

pub(crate) mod event_artifact_ref {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "event_artifact_refs")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub global_seq: i64,
        #[sea_orm(primary_key, auto_increment = false)]
        pub hash: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub artifact_kind: String,
        #[sea_orm(
            belongs_to,
            from = "global_seq",
            to = "global_seq",
            on_delete = "Cascade"
        )]
        pub event: BelongsTo<super::super::core::event::Entity>,
        #[sea_orm(
            belongs_to,
            from = "(hash, artifact_kind)",
            to = "(hash, artifact_kind)"
        )]
        pub artifact: BelongsTo<super::artifact_metadata::Entity>,
    }
}

pub(crate) mod vault_key_metadata {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "vault_key_metadata")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        #[sea_orm(unique)]
        pub key_id: String,
        pub verifier: String,
        pub schema_version: i64,
        pub created_at_ms: i64,
    }
}

pub(crate) mod secret_artifact {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "secret_artifact_projection")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub secret_id: String,
        #[sea_orm(unique)]
        pub blob_locator: String,
        pub key_id: String,
        pub ciphertext_hash: String,
        pub stored_bytes: i64,
        pub ref_count: i64,
        pub schema_version: i64,
        pub created_at_ms: i64,
    }
}

pub(crate) mod secret_artifact_owner {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "secret_artifact_owners")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub owner_kind: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub owner_id: String,
        pub secret_id: String,
        pub created_at_ms: i64,
        #[sea_orm(belongs_to, from = "secret_id", to = "secret_id")]
        pub secret: BelongsTo<super::secret_artifact::Entity>,
    }
}
