//! Invocation archive 与 checkpoint entities。

pub(crate) mod invocation_ledger {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "execution_invocation_ledger")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[sea_orm(unique_key = "path")]
        pub execution_id: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub invocation_ordinal: i64,
        pub invocation_kind: String,
        pub node_id: String,
        #[sea_orm(unique_key = "path")]
        pub invocation_path_json: String,
        pub payload_id: String,
        #[sea_orm(
            belongs_to,
            from = "execution_id",
            to = "execution_id",
            on_delete = "Cascade"
        )]
        pub execution: BelongsTo<super::super::lifecycle::execution::Entity>,
    }
}

pub(crate) mod effect_capture {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "effect_captures")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[sea_orm(unique_key = "ordinal")]
        pub execution_id: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub effect_id: String,
        pub node_id: String,
        pub effect_kind: String,
        pub fingerprint: String,
        pub output_hash: String,
        pub witness_hash: Option<String>,
        pub output_artifact_hash: String,
        pub witness_artifact_hash: Option<String>,
        pub response_headers_secret_id: Option<String>,
        pub request_body_secret_id: Option<String>,
        pub global_seq: i64,
        pub invocation_path_json: String,
        #[sea_orm(unique_key = "ordinal")]
        pub invocation_ordinal: i64,
        #[sea_orm(
            belongs_to,
            from = "(execution_id, invocation_ordinal)",
            to = "(execution_id, invocation_ordinal)"
        )]
        pub invocation: BelongsTo<super::invocation_ledger::Entity>,
        #[sea_orm(
            belongs_to,
            from = "response_headers_secret_id",
            to = "secret_id",
            relation_enum = "ResponseHeaders"
        )]
        pub response_headers: BelongsTo<Option<super::super::artifact::secret_artifact::Entity>>,
        #[sea_orm(
            belongs_to,
            from = "request_body_secret_id",
            to = "secret_id",
            relation_enum = "RequestBody"
        )]
        pub request_body: BelongsTo<Option<super::super::artifact::secret_artifact::Entity>>,
        #[sea_orm(belongs_to, from = "global_seq", to = "global_seq")]
        pub event: BelongsTo<super::super::core::event::Entity>,
    }
}

pub(crate) mod control_trace {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "control_traces")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[sea_orm(unique_key = "path")]
        pub execution_id: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub invocation_ordinal: i64,
        #[sea_orm(unique_key = "path")]
        pub invocation_path_json: String,
        pub trace_hash: String,
        pub trace_json: String,
        #[sea_orm(
            belongs_to,
            from = "(execution_id, invocation_ordinal)",
            to = "(execution_id, invocation_ordinal)",
            on_delete = "Cascade"
        )]
        pub invocation: BelongsTo<super::invocation_ledger::Entity>,
    }
}

pub(crate) mod source_checkpoint {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "source_checkpoints")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub source_identity: String,
        pub source_revision: i64,
        pub global_seq: i64,
        pub artifact_hash: String,
        pub created_at_ms: i64,
    }
}

pub(crate) mod library_checkpoint {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "library_checkpoints")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub global_seq: i64,
        pub artifact_hash: String,
        pub created_at_ms: i64,
    }
}
