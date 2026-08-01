//! Current `SeaORM` relational model。

pub mod archive;
pub mod artifact;
pub mod core;
pub mod document;
pub mod lifecycle;
pub mod projection;

use sea_orm::{ConnectionTrait, Schema, SchemaBuilder};

macro_rules! active_model_behavior {
    ($($active_model:path),+ $(,)?) => {
        $(impl sea_orm::ActiveModelBehavior for $active_model {})+
    };
}

active_model_behavior!(
    core::storage_schema_metadata::ActiveModel,
    core::event_counter::ActiveModel,
    core::event_stream::ActiveModel,
    core::event::ActiveModel,
    artifact::artifact_metadata::ActiveModel,
    artifact::event_artifact_ref::ActiveModel,
    artifact::vault_key_metadata::ActiveModel,
    artifact::secret_artifact::ActiveModel,
    artifact::secret_artifact_owner::ActiveModel,
    lifecycle::candidate::ActiveModel,
    lifecycle::source::ActiveModel,
    lifecycle::source_version::ActiveModel,
    lifecycle::execution::ActiveModel,
    archive::invocation_ledger::ActiveModel,
    archive::effect_capture::ActiveModel,
    archive::control_trace::ActiveModel,
    archive::source_checkpoint::ActiveModel,
    archive::library_checkpoint::ActiveModel,
    projection::source::ActiveModel,
    projection::item::ActiveModel,
    projection::collection::ActiveModel,
    projection::unit::ActiveModel,
    projection::asset::ActiveModel,
    projection::relation::ActiveModel,
    projection::action::ActiveModel,
    projection::hint::ActiveModel,
    projection::library::ActiveModel,
    document::rule_document::ActiveModel,
    document::rule_document_semantic::ActiveModel,
    document::rule_document_layout::ActiveModel,
    document::rule_document_provenance::ActiveModel,
);

/// 注册全部 current entities，供唯一 baseline migration 使用。
pub fn schema_builder<C>(database: &C) -> SchemaBuilder
where
    C: ConnectionTrait,
{
    Schema::new(database.get_database_backend())
        .builder()
        .register(core::storage_schema_metadata::Entity)
        .register(core::event_counter::Entity)
        .register(core::event_stream::Entity)
        .register(core::event::Entity)
        .register(artifact::artifact_metadata::Entity)
        .register(artifact::event_artifact_ref::Entity)
        .register(artifact::vault_key_metadata::Entity)
        .register(artifact::secret_artifact::Entity)
        .register(artifact::secret_artifact_owner::Entity)
        .register(lifecycle::candidate::Entity)
        .register(lifecycle::source::Entity)
        .register(lifecycle::source_version::Entity)
        .register(lifecycle::execution::Entity)
        .register(archive::invocation_ledger::Entity)
        .register(archive::effect_capture::Entity)
        .register(archive::control_trace::Entity)
        .register(archive::source_checkpoint::Entity)
        .register(archive::library_checkpoint::Entity)
        .register(projection::source::Entity)
        .register(projection::item::Entity)
        .register(projection::collection::Entity)
        .register(projection::unit::Entity)
        .register(projection::asset::Entity)
        .register(projection::relation::Entity)
        .register(projection::action::Entity)
        .register(projection::hint::Entity)
        .register(projection::library::Entity)
        .register(document::rule_document::Entity)
        .register(document::rule_document_semantic::Entity)
        .register(document::rule_document_layout::Entity)
        .register(document::rule_document_provenance::Entity)
}
