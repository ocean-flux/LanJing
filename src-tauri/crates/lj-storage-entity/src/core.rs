//! Schema marker 与 append-only event entities。

pub(crate) mod storage_schema_metadata {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "storage_schema_metadata")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub schema_name: String,
        pub schema_version: i64,
        pub schema_fingerprint: String,
    }
}

pub(crate) mod event_counter {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "event_counters")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: i64,
        pub next_global_seq: i64,
    }
}

pub(crate) mod event_stream {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "event_streams")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub stream_id: String,
        pub version: i64,
    }
}

pub(crate) mod event {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "events")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub global_seq: i64,
        pub stream_id: String,
        pub stream_version: i64,
        #[sea_orm(unique)]
        pub event_id: String,
        pub source_identity: Option<String>,
        pub event_type: String,
        pub schema_version: i32,
        pub correlation_id: Option<String>,
        pub causation_id: Option<String>,
        pub trace_id: String,
        pub occurred_at_ms: i64,
        pub payload_json: String,
        pub artifact_refs_json: String,
        pub secret_refs_json: String,
    }
}
