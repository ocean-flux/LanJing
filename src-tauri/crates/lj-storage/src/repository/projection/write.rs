/// 原子更新 library projection 并追加其 resource stream Event。
pub(crate) async fn process_library_update(
    conn: &mut DatabaseSession,
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
    if let Some(receipt) = idempotent_event(conn, &event).await? {
        return Ok(receipt);
    }
    let entry = request.entry;
    append_event_transaction(conn, &event, &[], move |conn, global_seq, _revision| Box::pin(async move {
        statement(
            "INSERT INTO library_projection (resource_id, favorite, pinned, last_opened_at, progress_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(resource_id) DO UPDATE SET favorite = excluded.favorite, pinned = excluded.pinned, last_opened_at = excluded.last_opened_at, progress_json = excluded.progress_json, updated_global_seq = excluded.updated_global_seq",
        )
        .bind(&entry.resource_id.0)
        .bind(i32::from(entry.favorite))
        .bind(i32::from(entry.pinned))
        .bind(entry.last_opened_at.as_deref())
        .bind(progress_json.as_deref())
        .bind(to_i64(global_seq)?)
        .execute(conn).await
        .map_err(database_error)?;
        Ok(())
    }))
    .await
}

/// 应用已通过 source ownership 验证的 Delta projection。
pub(crate) async fn apply_projection_delta(
    conn: &mut DatabaseSession,
    source_identity: &str,
    delta: &ProjectionDelta,
    global_seq: u64,
) -> Result<(), StorageError> {
    for source in &delta.upserts.sources {
        upsert_projection_source(conn, source, global_seq).await?;
    }
    for item in &delta.upserts.items {
        upsert_item(conn, item, global_seq).await?;
    }
    for collection in &delta.upserts.collections {
        upsert_collection(conn, collection, global_seq).await?;
    }
    for unit in &delta.upserts.units {
        upsert_unit(conn, unit, global_seq).await?;
    }
    for asset in &delta.upserts.assets {
        upsert_asset(conn, asset, global_seq).await?;
    }
    for relation in &delta.upserts.relations {
        upsert_relation(conn, relation, global_seq).await?;
    }
    for action in &delta.upserts.actions {
        upsert_action(conn, action, global_seq).await?;
    }
    for hint in &delta.upserts.hints {
        upsert_hint(conn, source_identity, hint, global_seq).await?;
    }
    apply_tombstones(conn, source_identity, &delta.tombstones).await?;
    Ok(())
}

pub(crate) async fn validate_delta_source(
    conn: &mut DatabaseSession,
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
        ensure_existing_owner(
            conn,
            "projection_collections",
            "id",
            &collection.id.0,
            source_identity,
        ).await?;
    }
    for unit in &delta.upserts.units {
        ensure_source(&unit.source_id, source_identity)?;
        ensure_existing_owner(conn, "projection_units", "id", &unit.id.0, source_identity).await?;
    }
    for asset in &delta.upserts.assets {
        ensure_source(&asset.source_id, source_identity)?;
        ensure_existing_owner(
            conn,
            "projection_assets",
            "id",
            &asset.id.0,
            source_identity,
        ).await?;
    }
    for relation in &delta.upserts.relations {
        ensure_source(&relation.source_id, source_identity)?;
    }
    for action in &delta.upserts.actions {
        ensure_source(&action.source_id, source_identity)?;
        ensure_existing_owner(
            conn,
            "projection_actions",
            "id",
            &action.id.0,
            source_identity,
        ).await?;
    }
    for hint in &delta.upserts.hints {
        ensure_existing_owner(
            conn,
            "projection_hints",
            "resource_id",
            &hint.resource_id.0,
            source_identity,
        ).await?;
    }
    validate_tombstone_owner(conn, source_identity, &delta.tombstones).await
}

async fn validate_tombstone_owner(
    conn: &mut DatabaseSession,
    source_identity: &str,
    tombstones: &ProjectionTombstones,
) -> Result<(), StorageError> {
    for source in &tombstones.sources {
        ensure_source(source, source_identity)?;
    }
    for id in &tombstones.items {
        ensure_existing_owner(conn, "projection_items", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.collections {
        ensure_existing_owner(conn, "projection_collections", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.units {
        ensure_existing_owner(conn, "projection_units", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.assets {
        ensure_existing_owner(conn, "projection_assets", "id", &id.0, source_identity).await?;
    }
    for relation in &tombstones.relations {
        ensure_source(&relation.source_id, source_identity)?;
    }
    for id in &tombstones.actions {
        ensure_existing_owner(conn, "projection_actions", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.hints {
        ensure_existing_owner(
            conn,
            "projection_hints",
            "resource_id",
            &id.0,
            source_identity,
        ).await?;
    }
    Ok(())
}

async fn ensure_existing_owner(
    conn: &mut DatabaseSession,
    table: &str,
    id_column: &str,
    id: &str,
    source_identity: &str,
) -> Result<(), StorageError> {
    let sql = format!("SELECT source_identity AS value FROM {table} WHERE {id_column} = ?");
    let owner = statement(sql)
        .bind(id)
        .get_result::<OwnerRow>(conn).await
        .optional()
        .map_err(database_error)?;
    if owner.is_some_and(|owner| owner.value != source_identity) {
        return Err(StorageError::InvalidInput("投影资源跨来源写入".to_string()));
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
pub(crate) async fn upsert_projection_source(
    conn: &mut DatabaseSession,
    source: &SourceProfile,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(source)?;
    statement(
        "INSERT INTO projection_sources (id, payload_json, updated_global_seq) VALUES (?, ?, ?) ON CONFLICT(id) DO UPDATE SET payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind(&source.id.0)
    .bind(&payload)
    .bind(to_i64(global_seq)?)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn upsert_item(
    conn: &mut DatabaseSession,
    item: &MediaItem,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(item)?;
    let affected = statement(
        "INSERT INTO projection_items (id, source_identity, media_kind, title, completeness, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, media_kind = excluded.media_kind, title = excluded.title, completeness = excluded.completeness, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq WHERE projection_items.source_identity = excluded.source_identity",
    )
    .bind(&item.id.0)
    .bind(&item.source_id.0)
    .bind(serialize(&item.media_kind)?)
    .bind(&item.title)
    .bind(serialize(&item.completeness)?)
    .bind(&payload)
    .bind(to_i64(global_seq)?)
        .execute(conn).await
        .map_err(database_error)?;
    if affected == 0 {
        return Err(StorageError::InvalidInput("投影资源跨来源写入".to_string()));
    }
    Ok(())
}

async fn upsert_collection(
    conn: &mut DatabaseSession,
    collection: &MediaCollection,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(collection)?;
    statement(
        "INSERT INTO projection_collections (id, source_identity, collection_kind, title, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, collection_kind = excluded.collection_kind, title = excluded.title, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind(&collection.id.0)
    .bind(&collection.source_id.0)
    .bind(&collection.kind)
    .bind(&collection.title)
    .bind(&payload)
    .bind(to_i64(global_seq)?)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn upsert_unit(
    conn: &mut DatabaseSession,
    unit: &MediaUnit,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(unit)?;
    let position = unit.position.map(i64::from);
    statement(
        "INSERT INTO projection_units (id, source_identity, item_id, position, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, item_id = excluded.item_id, position = excluded.position, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind(&unit.id.0)
    .bind(&unit.source_id.0)
    .bind(&unit.item_id.0)
    .bind(position)
    .bind(&payload)
    .bind(to_i64(global_seq)?)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn upsert_asset(
    conn: &mut DatabaseSession,
    asset: &MediaAsset,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(asset)?;
    statement(
        "INSERT INTO projection_assets (id, source_identity, unit_id, asset_kind, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, unit_id = excluded.unit_id, asset_kind = excluded.asset_kind, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind(&asset.id.0)
    .bind(&asset.source_id.0)
    .bind(asset.unit_id.as_ref().map(|value| value.0.as_str()))
    .bind(serialize(&asset.asset_kind)?)
    .bind(&payload)
    .bind(to_i64(global_seq)?)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn upsert_relation(
    conn: &mut DatabaseSession,
    relation: &MediaRelation,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(relation)?;
    let kind = serialize(&relation.relation_kind)?;
    statement(
        "INSERT INTO projection_relations (source_identity, from_id, to_id, relation_kind, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(source_identity, from_id, to_id, relation_kind) DO UPDATE SET payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind(&relation.source_id.0)
    .bind(&relation.from_id.0)
    .bind(&relation.to_id.0)
    .bind(&kind)
    .bind(&payload)
    .bind(to_i64(global_seq)?)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn upsert_action(
    conn: &mut DatabaseSession,
    action: &MediaAction,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(action)?;
    statement(
        "INSERT INTO projection_actions (id, source_identity, intent, payload_json, updated_global_seq) VALUES (?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET source_identity = excluded.source_identity, intent = excluded.intent, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind(&action.id.0)
    .bind(&action.source_id.0)
    .bind(serialize(&action.intent)?)
    .bind(&payload)
    .bind(to_i64(global_seq)?)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn upsert_hint(
    conn: &mut DatabaseSession,
    source_identity: &str,
    hint: &PresentationHint,
    global_seq: u64,
) -> Result<(), StorageError> {
    let payload = serialize(hint)?;
    statement(
        "INSERT INTO projection_hints (resource_id, source_identity, payload_json, updated_global_seq) VALUES (?, ?, ?, ?) ON CONFLICT(resource_id) DO UPDATE SET source_identity = excluded.source_identity, payload_json = excluded.payload_json, updated_global_seq = excluded.updated_global_seq",
    )
    .bind(&hint.resource_id.0)
    .bind(source_identity)
    .bind(&payload)
    .bind(to_i64(global_seq)?)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn apply_tombstones(
    conn: &mut DatabaseSession,
    source_identity: &str,
    tombstones: &ProjectionTombstones,
) -> Result<(), StorageError> {
    for id in &tombstones.sources {
        delete_by_id(conn, "projection_sources", "id", &id.0).await?;
    }
    for id in &tombstones.items {
        delete_by_owner(conn, "projection_items", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.collections {
        delete_by_owner(conn, "projection_collections", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.units {
        delete_by_owner(conn, "projection_units", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.assets {
        delete_by_owner(conn, "projection_assets", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.actions {
        delete_by_owner(conn, "projection_actions", "id", &id.0, source_identity).await?;
    }
    for id in &tombstones.hints {
        delete_by_owner(
            conn,
            "projection_hints",
            "resource_id",
            &id.0,
            source_identity,
        ).await?;
    }
    for relation in &tombstones.relations {
        statement(
            "DELETE FROM projection_relations WHERE source_identity = ? AND from_id = ? AND to_id = ? AND relation_kind = ?",
        )
        .bind(&relation.source_id.0)
        .bind(&relation.from_id.0)
        .bind(&relation.to_id.0)
        .bind(&relation.relation_kind)
        .execute(conn).await
        .map_err(database_error)?;
    }
    Ok(())
}

async fn delete_by_id(
    conn: &mut DatabaseSession,
    table: &str,
    column: &str,
    id: &str,
) -> Result<(), StorageError> {
    let sql = format!("DELETE FROM {table} WHERE {column} = ?");
    statement(sql)
        .bind(id)
        .execute(conn).await
        .map_err(database_error)?;
    Ok(())
}

async fn delete_by_owner(
    conn: &mut DatabaseSession,
    table: &str,
    id_column: &str,
    id: &str,
    source_identity: &str,
) -> Result<(), StorageError> {
    let sql = format!("DELETE FROM {table} WHERE {id_column} = ? AND source_identity = ?");
    statement(sql)
        .bind(id)
        .bind(source_identity)
        .execute(conn).await
        .map_err(database_error)?;
    Ok(())
}
