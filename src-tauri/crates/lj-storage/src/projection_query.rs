//! 规范化媒体 projection、library aggregate 与只读查询实现。
//!
//! 每个 Delta 在 writer transaction 内按稳定 resource ID O(delta) upsert/tombstone；绝不读取
//! 或重写整张 JSON graph。source/item/unit/asset 查询只走独立只读 connection，结果以稳定 SQL
//! 顺序返回。library 是用户独有 aggregate，共享 resource identity 但不复制 source Graph、Plan
//! 或 secret。

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;
use lj_media::{
    MediaAction, MediaAsset, MediaCollection, MediaGraphDelta, MediaItem, MediaRelation,
    MediaResourceId, MediaUnit, PresentationHint, SourceProfile,
};
use lj_rule_model::EventType;

use crate::event_store::{
    EventDraft, append_event_transaction, current_global_seq, database_error, deserialize,
    from_i64, idempotent_event, serialize, to_i64,
};
use crate::types::{
    CommitReceipt, LibraryEntry, LibraryProgress, LibraryProjection, LibraryProjectionEntry,
    LibraryUpdate, ProjectionDelta, ProjectionTombstones, SourceProjectionView, StorageError,
};

/// 原子更新 library projection 并追加其 resource stream Event。
pub(crate) fn process_library_update(
    conn: &mut SqliteConnection,
    request: LibraryUpdate,
) -> Result<CommitReceipt, StorageError> {
    let progress_json = request.entry.progress.as_ref().map(serialize).transpose()?;
    let entry_payload = serialize(&request.entry)?;
    let event = EventDraft {
        stream_id: library_stream_id(&request.entry.resource_id),
        expected_version: request.expected_version,
        event_id: request.event_id,
        event_type: EventType::Library,
        schema_version: 1,
        correlation_id: None,
        causation_id: None,
        trace_id: request.trace_id,
        occurred_at_ms: request.occurred_at_ms,
        payload: serde_json::json!({"kind": "updated", "entry": serde_json::from_str::<serde_json::Value>(&entry_payload).map_err(|_| StorageError::Serialization)?}),
        source_identity: None,
    };
    if let Some(receipt) = idempotent_event(conn, &event)? {
        return Ok(receipt);
    }
    let entry = request.entry;
    append_event_transaction(conn, &event, &[], move |conn, global_seq, _revision| {
        sql_query(
            "INSERT INTO library_projection (resource_id, favorite, pinned, last_opened_at, progress_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(resource_id) DO UPDATE SET favorite = excluded.favorite, pinned = excluded.pinned, last_opened_at = excluded.last_opened_at, progress_json = excluded.progress_json, updated_global_seq = excluded.updated_global_seq",
        )
        .bind::<Text, _>(&entry.resource_id.0)
        .bind::<Integer, _>(i32::from(entry.favorite))
        .bind::<Integer, _>(i32::from(entry.pinned))
        .bind::<Nullable<Text>, _>(entry.last_opened_at.as_deref())
        .bind::<Nullable<Text>, _>(progress_json.as_deref())
        .bind::<BigInt, _>(to_i64(global_seq)?)
        .execute(conn)
        .map_err(database_error)?;
        Ok(())
    })
}

/// 应用已通过 source ownership 验证的 Delta projection。
pub(crate) fn apply_projection_delta(
    conn: &mut SqliteConnection,
    source_identity: &str,
    delta: &ProjectionDelta,
    global_seq: u64,
) -> Result<(), StorageError> {
    for source in &delta.upserts.sources {
        upsert_projection_source(conn, source, global_seq)?;
    }
    for item in &delta.upserts.items {
        upsert_item(conn, item, global_seq)?;
    }
    for collection in &delta.upserts.collections {
        upsert_collection(conn, collection, global_seq)?;
    }
    for unit in &delta.upserts.units {
        upsert_unit(conn, unit, global_seq)?;
    }
    for asset in &delta.upserts.assets {
        upsert_asset(conn, asset, global_seq)?;
    }
    for relation in &delta.upserts.relations {
        upsert_relation(conn, relation, global_seq)?;
    }
    for action in &delta.upserts.actions {
        upsert_action(conn, action, global_seq)?;
    }
    for hint in &delta.upserts.hints {
        upsert_hint(conn, source_identity, hint, global_seq)?;
    }
    apply_tombstones(conn, &delta.tombstones)?;
    Ok(())
}

/// 拒绝任何跨 source 的 upsert 或 relation tombstone，防止投影 owner 被偷换。
pub(crate) fn validate_delta_source(
    delta: &ProjectionDelta,
    source_identity: &str,
) -> Result<(), StorageError> {
    for source in &delta.upserts.sources {
        ensure_source(&source.id, source_identity)?;
    }
    for item in &delta.upserts.items {
        ensure_source(&item.source_id, source_identity)?;
    }
    for collection in &delta.upserts.collections {
        ensure_source(&collection.source_id, source_identity)?;
    }
    for unit in &delta.upserts.units {
        ensure_source(&unit.source_id, source_identity)?;
    }
    for asset in &delta.upserts.assets {
        ensure_source(&asset.source_id, source_identity)?;
    }
    for relation in &delta.upserts.relations {
        ensure_source(&relation.source_id, source_identity)?;
    }
    for action in &delta.upserts.actions {
        ensure_source(&action.source_id, source_identity)?;
    }
    for relation in &delta.tombstones.relations {
        ensure_source(&relation.source_id, source_identity)?;
    }
    Ok(())
}

fn ensure_source(id: &MediaResourceId, expected: &str) -> Result<(), StorageError> {
    if id.0 == expected {
        Ok(())
    } else {
        Err(StorageError::InvalidInput("投影资源跨来源写入".to_string()))
    }
}

/// source projection 与 source install 共享的 profile upsert。
pub(crate) fn upsert_projection_source(
    conn: &mut SqliteConnection,
    source: &SourceProfile,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(source)?;
    sql_query(
        "INSERT INTO projection_sources (id, payload_json, updated_global_seq) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(&source.id.0)
    .bind::<Text, _>(&payload)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn upsert_item(
    conn: &mut SqliteConnection,
    item: &MediaItem,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(item)?;
    sql_query(
        "INSERT INTO projection_items (id, source_identity, media_kind, title, completeness, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, media_kind = excluded.media_kind, title = excluded.title, completeness = excluded.completeness, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(&item.id.0)
    .bind::<Text, _>(&item.source_id.0)
    .bind::<Text, _>(serialize(&item.media_kind)?)
    .bind::<Text, _>(&item.title)
    .bind::<Text, _>(serialize(&item.completeness)?)
    .bind::<Text, _>(&payload)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn upsert_collection(
    conn: &mut SqliteConnection,
    collection: &MediaCollection,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(collection)?;
    sql_query(
        "INSERT INTO projection_collections (id, source_identity, collection_kind, title, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, collection_kind = excluded.collection_kind, title = excluded.title, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(&collection.id.0)
    .bind::<Text, _>(&collection.source_id.0)
    .bind::<Text, _>(&collection.kind)
    .bind::<Text, _>(&collection.title)
    .bind::<Text, _>(&payload)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn upsert_unit(
    conn: &mut SqliteConnection,
    unit: &MediaUnit,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(unit)?;
    let position = unit.position.map(i64::from);
    sql_query(
        "INSERT INTO projection_units (id, source_identity, item_id, position, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, item_id = excluded.item_id, position = excluded.position, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(&unit.id.0)
    .bind::<Text, _>(&unit.source_id.0)
    .bind::<Text, _>(&unit.item_id.0)
    .bind::<Nullable<BigInt>, _>(position)
    .bind::<Text, _>(&payload)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn upsert_asset(
    conn: &mut SqliteConnection,
    asset: &MediaAsset,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(asset)?;
    sql_query(
        "INSERT INTO projection_assets (id, source_identity, unit_id, asset_kind, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, unit_id = excluded.unit_id, asset_kind = excluded.asset_kind, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(&asset.id.0)
    .bind::<Text, _>(&asset.source_id.0)
    .bind::<Nullable<Text>, _>(asset.unit_id.as_ref().map(|value| value.0.as_str()))
    .bind::<Text, _>(serialize(&asset.asset_kind)?)
    .bind::<Text, _>(&payload)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn upsert_relation(
    conn: &mut SqliteConnection,
    relation: &MediaRelation,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(relation)?;
    let kind = serialize(&relation.relation_kind)?;
    sql_query(
        "INSERT INTO projection_relations (source_identity, from_id, to_id, relation_kind, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(source_identity, from_id, to_id, relation_kind) DO UPDATE SET payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(&relation.source_id.0)
    .bind::<Text, _>(&relation.from_id.0)
    .bind::<Text, _>(&relation.to_id.0)
    .bind::<Text, _>(&kind)
    .bind::<Text, _>(&payload)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn upsert_action(
    conn: &mut SqliteConnection,
    action: &MediaAction,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(action)?;
    sql_query(
        "INSERT INTO projection_actions (id, source_identity, intent, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, intent = excluded.intent, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(&action.id.0)
    .bind::<Text, _>(&action.source_id.0)
    .bind::<Text, _>(serialize(&action.intent)?)
    .bind::<Text, _>(&payload)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn upsert_hint(
    conn: &mut SqliteConnection,
    source_identity: &str,
    hint: &PresentationHint,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(hint)?;
    sql_query(
        "INSERT INTO projection_hints (resource_id, source_identity, payload_json, updated_global_seq) VALUES (?, ?, ?, ?) ON CONFLICT(resource_id) DO UPDATE SET source_identity = excluded.source_identity, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind::<Text, _>(&hint.resource_id.0)
    .bind::<Text, _>(source_identity)
    .bind::<Text, _>(&payload)
    .bind::<BigInt, _>(to_i64(global_seq)?)
    .execute(conn)
    .map_err(database_error)?;
    Ok(())
}

fn apply_tombstones(
    conn: &mut SqliteConnection,
    tombstones: &ProjectionTombstones,
) -> Result<(), StorageError> {
    for id in &tombstones.sources {
        delete_by_id(conn, "projection_sources", "id", &id.0)?;
    }
    for id in &tombstones.items {
        delete_by_id(conn, "projection_items", "id", &id.0)?;
    }
    for id in &tombstones.collections {
        delete_by_id(conn, "projection_collections", "id", &id.0)?;
    }
    for id in &tombstones.units {
        delete_by_id(conn, "projection_units", "id", &id.0)?;
    }
    for id in &tombstones.assets {
        delete_by_id(conn, "projection_assets", "id", &id.0)?;
    }
    for id in &tombstones.actions {
        delete_by_id(conn, "projection_actions", "id", &id.0)?;
    }
    for id in &tombstones.hints {
        delete_by_id(conn, "projection_hints", "resource_id", &id.0)?;
    }
    for relation in &tombstones.relations {
        sql_query(
            "DELETE FROM projection_relations WHERE source_identity = ? AND from_id = ? AND to_id = ? AND relation_kind = ?",
        )
        .bind::<Text, _>(&relation.source_id.0)
        .bind::<Text, _>(&relation.from_id.0)
        .bind::<Text, _>(&relation.to_id.0)
        .bind::<Text, _>(&relation.relation_kind)
        .execute(conn)
        .map_err(database_error)?;
    }
    Ok(())
}

fn delete_by_id(
    conn: &mut SqliteConnection,
    table: &str,
    column: &str,
    id: &str,
) -> Result<(), StorageError> {
    let statement = format!("DELETE FROM {table} WHERE {column} = ?");
    sql_query(statement)
        .bind::<Text, _>(id)
        .execute(conn)
        .map_err(database_error)?;
    Ok(())
}

/// 在同一只读 transaction 取得 library global sequence 和按稳定 ID 排序的 entries。
pub(crate) fn library_projection_sync(
    conn: &mut SqliteConnection,
) -> Result<LibraryProjection, StorageError> {
    conn.transaction::<_, StorageError, _>(|conn| {
        Ok(LibraryProjection {
            global_seq: current_global_seq(conn)?,
            entries: list_library_projection_entries_sync(conn)?,
        })
    })
}

fn list_library_projection_entries_sync(
    conn: &mut SqliteConnection,
) -> Result<Vec<LibraryProjectionEntry>, StorageError> {
    let rows = sql_query(
        "SELECT library_projection.resource_id, library_projection.favorite, library_projection.pinned, library_projection.last_opened_at, library_projection.progress_json, library_projection.updated_global_seq, COALESCE(event_streams.version, -1) AS revision FROM library_projection LEFT JOIN event_streams ON event_streams.stream_id = 'library/' || library_projection.resource_id ORDER BY library_projection.resource_id ASC",
    )
    .load::<LibraryProjectionRow>(conn)
    .map_err(database_error)?;
    rows.into_iter()
        .map(library_projection_entry_from_row)
        .collect()
}

fn library_projection_entry_from_row(
    row: LibraryProjectionRow,
) -> Result<LibraryProjectionEntry, StorageError> {
    Ok(LibraryProjectionEntry {
        resource_id: MediaResourceId(row.resource_id),
        favorite: row.favorite != 0,
        pinned: row.pinned != 0,
        last_opened_at: row.last_opened_at,
        progress: row
            .progress_json
            .as_deref()
            .map(|json| deserialize::<LibraryProgress>(json.as_bytes()))
            .transpose()?,
        revision: from_i64(row.revision, "library revision")?,
        updated_global_seq: from_i64(row.updated_global_seq, "library projection global sequence")?,
    })
}

/// checkpoint 使用的稳定 library entries（resource ID 升序）。
pub(crate) fn list_library_entries_sync(
    conn: &mut SqliteConnection,
) -> Result<Vec<LibraryEntry>, StorageError> {
    let rows = sql_query(
        "SELECT resource_id, favorite, pinned, last_opened_at, progress_json FROM library_projection ORDER BY resource_id ASC",
    )
    .load::<LibraryRow>(conn)
    .map_err(database_error)?;
    rows.into_iter().map(library_from_row).collect()
}

pub(crate) fn get_library_entry_sync(
    conn: &mut SqliteConnection,
    resource_id: &str,
) -> Result<Option<LibraryEntry>, StorageError> {
    let row = sql_query(
        "SELECT resource_id, favorite, pinned, last_opened_at, progress_json FROM library_projection WHERE resource_id = ?",
    )
    .bind::<Text, _>(resource_id)
    .get_result::<LibraryRow>(conn)
    .optional()
    .map_err(database_error)?;
    row.map(library_from_row).transpose()
}

fn library_from_row(row: LibraryRow) -> Result<LibraryEntry, StorageError> {
    Ok(LibraryEntry {
        resource_id: MediaResourceId(row.resource_id),
        favorite: row.favorite != 0,
        pinned: row.pinned != 0,
        last_opened_at: row.last_opened_at,
        progress: row
            .progress_json
            .as_deref()
            .map(|json| deserialize::<LibraryProgress>(json.as_bytes()))
            .transpose()?,
    })
}

/// 汇聚一个 source 的规范化资源；不会回读历史 graph JSON。
pub(crate) fn source_projection_sync(
    conn: &mut SqliteConnection,
    source_identity: &str,
) -> Result<SourceProjectionView, StorageError> {
    let profile =
        get_payload_by_id::<SourceProfile>(conn, "projection_sources", "id", source_identity)?;
    let sources = profile.clone().into_iter().collect();
    Ok(SourceProjectionView {
        profile,
        delta: MediaGraphDelta {
            sources,
            items: payloads_by_source(conn, "projection_items", source_identity)?,
            collections: payloads_by_source(conn, "projection_collections", source_identity)?,
            units: payloads_by_source(conn, "projection_units", source_identity)?,
            assets: payloads_by_source(conn, "projection_assets", source_identity)?,
            relations: payloads_by_source(conn, "projection_relations", source_identity)?,
            actions: payloads_by_source(conn, "projection_actions", source_identity)?,
            hints: payloads_by_source(conn, "projection_hints", source_identity)?,
        },
    })
}

pub(crate) fn get_payload_by_id<T: serde::de::DeserializeOwned>(
    conn: &mut SqliteConnection,
    table: &str,
    column: &str,
    id: &str,
) -> Result<Option<T>, StorageError> {
    let statement = format!("SELECT payload_json FROM {table} WHERE {column} = ?");
    let row = sql_query(statement)
        .bind::<Text, _>(id)
        .get_result::<JsonRow>(conn)
        .optional()
        .map_err(database_error)?;
    row.map(|value| deserialize(value.payload_json.as_bytes()))
        .transpose()
}

fn payloads_by_source<T: serde::de::DeserializeOwned>(
    conn: &mut SqliteConnection,
    table: &str,
    source_identity: &str,
) -> Result<Vec<T>, StorageError> {
    payloads_by_column(conn, table, "source_identity", source_identity)
}

pub(crate) fn payloads_by_column<T: serde::de::DeserializeOwned>(
    conn: &mut SqliteConnection,
    table: &str,
    column: &str,
    value: &str,
) -> Result<Vec<T>, StorageError> {
    let statement =
        format!("SELECT payload_json FROM {table} WHERE {column} = ? ORDER BY payload_json ASC");
    let rows = sql_query(statement)
        .bind::<Text, _>(value)
        .load::<JsonRow>(conn)
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| deserialize(row.payload_json.as_bytes()))
        .collect()
}

fn library_stream_id(resource_id: &MediaResourceId) -> String {
    format!("library/{}", resource_id.0)
}

#[derive(QueryableByName)]
struct JsonRow {
    #[diesel(sql_type = Text)]
    payload_json: String,
}

#[derive(QueryableByName)]
struct LibraryRow {
    #[diesel(sql_type = Text)]
    resource_id: String,
    #[diesel(sql_type = Integer)]
    favorite: i32,
    #[diesel(sql_type = Integer)]
    pinned: i32,
    #[diesel(sql_type = Nullable<Text>)]
    last_opened_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    progress_json: Option<String>,
}

#[derive(QueryableByName)]
struct LibraryProjectionRow {
    #[diesel(sql_type = Text)]
    resource_id: String,
    #[diesel(sql_type = Integer)]
    favorite: i32,
    #[diesel(sql_type = Integer)]
    pinned: i32,
    #[diesel(sql_type = Nullable<Text>)]
    last_opened_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    progress_json: Option<String>,
    #[diesel(sql_type = BigInt)]
    updated_global_seq: i64,
    #[diesel(sql_type = BigInt)]
    revision: i64,
}
