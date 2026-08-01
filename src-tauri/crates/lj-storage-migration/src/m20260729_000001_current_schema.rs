use sea_orm_migration::prelude::*;

use crate::{custom_schema, schema_metadata};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        schema_metadata::ensure_baseline_target_empty(connection).await?;
        lj_storage_entity::schema_builder(connection)
            .apply(connection)
            .await?;
        custom_schema::apply(connection).await?;
        schema_metadata::seed(connection).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        custom_schema::drop_all(manager.get_connection()).await
    }
}
