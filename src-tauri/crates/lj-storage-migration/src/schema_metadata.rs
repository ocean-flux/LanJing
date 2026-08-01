use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement, Value};
use sea_orm_migration::prelude::DbErr;

use crate::MigrationError;

pub const CURRENT_SCHEMA_NAME: &str = "lanjing_current";
pub const CURRENT_SCHEMA_VERSION: i64 = 1;
const BASELINE_NAME: &str = "m20260729_000001_current_schema";

pub(crate) enum SchemaState {
    Empty,
    Current,
    Unknown,
}

#[derive(FromQueryResult)]
struct CountRow {
    count: i64,
}

#[derive(FromQueryResult)]
struct MarkerRow {
    name: String,
    version: i64,
    fingerprint: String,
}

#[derive(FromQueryResult)]
struct SchemaObjectRow {
    object_type: String,
    name: String,
    table_name: String,
    sql: Option<String>,
}

#[derive(FromQueryResult)]
struct MigrationRow {
    version: String,
    applied_at: i64,
}

pub(crate) async fn classify<C>(connection: &C) -> Result<SchemaState, DbErr>
where
    C: ConnectionTrait,
{
    let count = query_one::<CountRow, _>(
        connection,
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
        Vec::new(),
    )
    .await?
    .ok_or_else(|| DbErr::Custom("schema object count missing".to_string()))?
    .count;
    if count == 0 {
        return Ok(SchemaState::Empty);
    }
    let marker_exists = query_one::<CountRow, _>(
        connection,
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'storage_schema_metadata'",
        Vec::new(),
    )
    .await?
    .is_some_and(|row| row.count == 1);
    Ok(if marker_exists {
        SchemaState::Current
    } else {
        SchemaState::Unknown
    })
}

pub(crate) async fn ensure_baseline_target_empty<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let count = query_one::<CountRow, _>(
        connection,
        "SELECT COUNT(*) AS count FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND name <> 'seaql_migrations'",
        Vec::new(),
    )
    .await?
    .ok_or_else(|| DbErr::Custom("schema object count missing".to_string()))?
    .count;
    if count == 0 {
        Ok(())
    } else {
        Err(DbErr::Custom(
            "baseline requires empty database".to_string(),
        ))
    }
}

pub(crate) async fn seed<C>(connection: &C) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let fingerprint = fingerprint(connection).await?;
    execute(
        connection,
        "INSERT INTO storage_schema_metadata (id, schema_name, schema_version, schema_fingerprint) VALUES (1, ?, ?, ?)",
        vec![
            CURRENT_SCHEMA_NAME.into(),
            CURRENT_SCHEMA_VERSION.into(),
            fingerprint.into(),
        ],
    )
    .await
}

pub(crate) async fn validate<C>(connection: &C) -> Result<(), MigrationError>
where
    C: ConnectionTrait,
{
    let marker = query_one::<MarkerRow, _>(
        connection,
        "SELECT schema_name AS name, schema_version AS version, schema_fingerprint AS fingerprint FROM storage_schema_metadata WHERE id = 1",
        Vec::new(),
    )
    .await?
    .ok_or(MigrationError::CurrentSchemaRequired)?;
    if marker.name != CURRENT_SCHEMA_NAME || marker.version != CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::CurrentSchemaRequired);
    }
    let migrations = query_all::<MigrationRow, _>(
        connection,
        "SELECT version, applied_at FROM seaql_migrations ORDER BY version",
    )
    .await?;
    if migrations.len() != 1
        || migrations[0].version != BASELINE_NAME
        || migrations[0].applied_at <= 0
    {
        return Err(MigrationError::CurrentSchemaRequired);
    }
    if marker.fingerprint != fingerprint(connection).await? {
        return Err(MigrationError::SchemaCorrupt(
            "schema fingerprint mismatch".to_string(),
        ));
    }
    Ok(())
}

async fn fingerprint<C>(connection: &C) -> Result<String, DbErr>
where
    C: ConnectionTrait,
{
    let objects = query_all::<SchemaObjectRow, _>(
        connection,
        "SELECT type AS object_type, name, tbl_name AS table_name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND name <> 'seaql_migrations' ORDER BY type, name",
    )
    .await?;
    let mut hasher = blake3::Hasher::new();
    for object in objects {
        for part in [
            object.object_type.as_str(),
            object.name.as_str(),
            object.table_name.as_str(),
            object.sql.as_deref().unwrap_or_default(),
        ] {
            hasher.update(part.as_bytes());
            hasher.update(&[0]);
        }
        hasher.update(&[0xff]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

async fn execute<C>(connection: &C, sql: &str, values: Vec<Value>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    connection
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql,
            values,
        ))
        .await?;
    Ok(())
}

async fn query_one<T, C>(connection: &C, sql: &str, values: Vec<Value>) -> Result<Option<T>, DbErr>
where
    T: FromQueryResult,
    C: ConnectionTrait,
{
    connection
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql,
            values,
        ))
        .await?
        .map(|row| T::from_query_result(&row, ""))
        .transpose()
}

async fn query_all<T, C>(connection: &C, sql: &str) -> Result<Vec<T>, DbErr>
where
    T: FromQueryResult,
    C: ConnectionTrait,
{
    connection
        .query_all_raw(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await?
        .iter()
        .map(|row| T::from_query_result(row, ""))
        .collect()
}
