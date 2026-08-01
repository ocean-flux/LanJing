//! Media graph 与 library projection entities。

pub(crate) mod source {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projection_sources")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub payload_json: String,
        pub updated_global_seq: i64,
    }
}

pub(crate) mod item {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projection_items")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub source_identity: String,
        pub media_kind: String,
        pub title: String,
        pub completeness: String,
        pub payload_json: String,
        pub updated_global_seq: i64,
    }
}

pub(crate) mod collection {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projection_collections")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub source_identity: String,
        pub collection_kind: String,
        pub title: String,
        pub payload_json: String,
        pub updated_global_seq: i64,
    }
}

pub(crate) mod unit {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projection_units")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub source_identity: String,
        pub item_id: String,
        pub position: Option<i64>,
        pub payload_json: String,
        pub updated_global_seq: i64,
    }
}

pub(crate) mod asset {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projection_assets")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub source_identity: String,
        pub unit_id: Option<String>,
        pub asset_kind: String,
        pub payload_json: String,
        pub updated_global_seq: i64,
    }
}

pub(crate) mod relation {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projection_relations")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub source_identity: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub from_id: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub to_id: String,
        #[sea_orm(primary_key, auto_increment = false)]
        pub relation_kind: String,
        pub payload_json: String,
        pub updated_global_seq: i64,
    }
}

pub(crate) mod action {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projection_actions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub source_identity: String,
        pub intent: String,
        pub payload_json: String,
        pub updated_global_seq: i64,
    }
}

pub(crate) mod hint {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "projection_hints")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub resource_id: String,
        pub source_identity: String,
        pub payload_json: String,
        pub updated_global_seq: i64,
    }
}

pub(crate) mod library {
    use sea_orm::entity::prelude::*;
    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "library_projection")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub resource_id: String,
        pub favorite: i64,
        pub pinned: i64,
        pub last_opened_at: Option<String>,
        pub progress_json: Option<String>,
        pub updated_global_seq: i64,
    }
}
