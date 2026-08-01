//! 原生规则文档 aggregate entities。
//!
//! 每个文档一个 `rule_documents` 主行，语义快照、布局快照与导入 provenance 各自独立成表；
//! provenance 的原文以 secret artifact 形式保存，表内只留随机 `secret_id` 引用。

pub(crate) mod rule_document {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "rule_documents")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub document_id: String,
        pub format: String,
        pub title: String,
        #[sea_orm(unique)]
        pub source_identity: String,
        pub state: String,
        pub semantic_revision: i64,
        pub layout_revision: i64,
        pub link_revision: i64,
        pub created_at_ms: i64,
        pub updated_at_ms: i64,
    }
}

pub(crate) mod rule_document_semantic {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "rule_document_semantics")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub document_id: String,
        pub revision: i64,
        pub definition_hash: String,
        pub definition_json: String,
        pub manifest_json: String,
        pub updated_at_ms: i64,
        #[sea_orm(
            belongs_to,
            from = "document_id",
            to = "document_id",
            on_delete = "Cascade"
        )]
        pub document: BelongsTo<super::rule_document::Entity>,
    }
}

pub(crate) mod rule_document_layout {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "rule_document_layouts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub document_id: String,
        pub revision: i64,
        pub layout_json: String,
        pub updated_at_ms: i64,
        #[sea_orm(
            belongs_to,
            from = "document_id",
            to = "document_id",
            on_delete = "Cascade"
        )]
        pub document: BelongsTo<super::rule_document::Entity>,
    }
}

pub(crate) mod rule_document_provenance {
    use sea_orm::entity::prelude::*;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "rule_document_provenances")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub document_id: String,
        pub format: String,
        pub adapter_version: String,
        pub input_hash: String,
        pub source_text_secret_id: Option<String>,
        pub diagnostics_json: String,
        pub imported_at_ms: i64,
        #[sea_orm(
            belongs_to,
            from = "document_id",
            to = "document_id",
            on_delete = "Cascade"
        )]
        pub document: BelongsTo<super::rule_document::Entity>,
        #[sea_orm(
            belongs_to,
            from = "source_text_secret_id",
            to = "secret_id",
            on_delete = "SetNull"
        )]
        pub source_text_secret: BelongsTo<Option<super::super::artifact::secret_artifact::Entity>>,
    }
}
