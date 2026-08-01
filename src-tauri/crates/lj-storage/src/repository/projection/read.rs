/// 在同一只读 transaction 取得 library global sequence 和按稳定 ID 排序的 entries。
pub(crate) async fn library_projection_sync(
    conn: &mut DatabaseSession,
) -> Result<LibraryProjection, StorageError> {
    conn.transaction(|conn| Box::pin(async move {
        Ok(LibraryProjection {
            global_seq: current_global_seq(conn).await?,
            entries: list_library_projection_entries_sync(conn).await?,
        })
    })).await
}

async fn list_library_projection_entries_sync(
    conn: &mut DatabaseSession,
) -> Result<Vec<LibraryProjectionEntry>, StorageError> {
    let rows = statement(
        "SELECT library_projection.resource_id, library_projection.favorite, library_projection.pinned, library_projection.last_opened_at, library_projection.progress_json, library_projection.updated_global_seq, COALESCE(event_streams.version, -1) AS revision FROM library_projection LEFT JOIN event_streams ON event_streams.stream_id = 'library/' || library_projection.resource_id ORDER BY library_projection.resource_id ASC",
    )
    .load::<LibraryProjectionRow>(conn).await
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
pub(crate) async fn list_library_entries_sync(
    conn: &mut DatabaseSession,
) -> Result<Vec<LibraryEntry>, StorageError> {
    let rows = statement(
        "SELECT resource_id, favorite, pinned, last_opened_at, progress_json FROM library_projection ORDER BY resource_id ASC",
    )
    .load::<LibraryRow>(conn).await
    .map_err(database_error)?;
    rows.into_iter().map(library_from_row).collect()
}

pub(crate) async fn get_library_entry_sync(
    conn: &mut DatabaseSession,
    resource_id: &str,
) -> Result<Option<LibraryEntry>, StorageError> {
    let row = statement(
        "SELECT resource_id, favorite, pinned, last_opened_at, progress_json FROM library_projection WHERE resource_id = ?",
    )
    .bind(resource_id)
    .get_result::<LibraryRow>(conn).await
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
pub(crate) async fn source_projection_sync(
    conn: &mut DatabaseSession,
    source_identity: &str,
) -> Result<SourceProjectionView, StorageError> {
    let profile =
        get_payload_by_id::<SourceProfile>(conn, "projection_sources", "id", source_identity).await?;
    let sources = profile.clone().into_iter().collect();
    Ok(SourceProjectionView {
        profile,
        delta: MediaGraphDelta {
            sources,
            items: payloads_by_source(conn, "projection_items", source_identity).await?,
            collections: payloads_by_source(conn, "projection_collections", source_identity).await?,
            units: payloads_by_source(conn, "projection_units", source_identity).await?,
            assets: payloads_by_source(conn, "projection_assets", source_identity).await?,
            relations: payloads_by_source(conn, "projection_relations", source_identity).await?,
            actions: payloads_by_source(conn, "projection_actions", source_identity).await?,
            hints: payloads_by_source(conn, "projection_hints", source_identity).await?,
        },
    })
}

pub(crate) async fn get_payload_by_id<T: serde::de::DeserializeOwned>(
    conn: &mut DatabaseSession,
    table: &str,
    column: &str,
    id: &str,
) -> Result<Option<T>, StorageError> {
    let sql = format!("SELECT payload_json FROM {table} WHERE {column} = ?");
    let row = statement(sql)
        .bind(id)
        .get_result::<JsonRow>(conn).await
        .optional()
        .map_err(database_error)?;
    row.map(|value| deserialize(value.payload_json.as_bytes()))
        .transpose()
}

/// 按主键批量读取媒体主体；跳过缺失 ID，结果按 `id` 升序。
///
/// 使用 `json_each` 单次 `IN` 查询，调用方应保证 ID 数量有界（产品面 ≤64）。
pub(crate) async fn get_items_by_ids(
    conn: &mut DatabaseSession,
    ids: &[String],
) -> Result<Vec<MediaItem>, StorageError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids_json = serde_json::to_string(ids).map_err(|_| StorageError::Serialization)?;
    let rows = statement(
        "SELECT payload_json FROM projection_items \
         WHERE id IN (SELECT value FROM json_each(?)) \
         ORDER BY id ASC",
    )
    .bind(ids_json)
    .load::<JsonRow>(conn).await
    .map_err(database_error)?;
    rows.into_iter()
        .map(|row| deserialize(row.payload_json.as_bytes()))
        .collect()
}

/// 按 item 索引有界列出 unit：`position IS NULL` 置后，再 `position ASC, id ASC`。
pub(crate) async fn list_units_for_item_bounded(
    conn: &mut DatabaseSession,
    item_id: &str,
    offset: u32,
    limit: u32,
) -> Result<Vec<MediaUnit>, StorageError> {
    let rows = statement(
        "SELECT payload_json FROM projection_units \
         WHERE item_id = ? \
         ORDER BY (position IS NULL) ASC, position ASC, id ASC \
         LIMIT ? OFFSET ?",
    )
    .bind(item_id)
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .load::<JsonRow>(conn).await
    .map_err(database_error)?;
    rows.into_iter()
        .map(|row| deserialize(row.payload_json.as_bytes()))
        .collect()
}

/// 按 unit 索引有界列出 asset：稳定 `id ASC`。
pub(crate) async fn list_assets_for_unit_bounded(
    conn: &mut DatabaseSession,
    unit_id: &str,
    offset: u32,
    limit: u32,
) -> Result<Vec<MediaAsset>, StorageError> {
    let rows = statement(
        "SELECT payload_json FROM projection_assets \
         WHERE unit_id = ? \
         ORDER BY id ASC \
         LIMIT ? OFFSET ?",
    )
    .bind(unit_id)
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .load::<JsonRow>(conn).await
    .map_err(database_error)?;
    rows.into_iter()
        .map(|row| deserialize(row.payload_json.as_bytes()))
        .collect()
}

async fn payloads_by_source<T: serde::de::DeserializeOwned>(
    conn: &mut DatabaseSession,
    table: &str,
    source_identity: &str,
) -> Result<Vec<T>, StorageError> {
    payloads_by_column(conn, table, "source_identity", source_identity).await
}

pub(crate) async fn payloads_by_column<T: serde::de::DeserializeOwned>(
    conn: &mut DatabaseSession,
    table: &str,
    column: &str,
    value: &str,
) -> Result<Vec<T>, StorageError> {
    let sql =
        format!("SELECT payload_json FROM {table} WHERE {column} = ? ORDER BY payload_json ASC");
    let rows = statement(sql)
        .bind(value)
        .load::<JsonRow>(conn).await
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| deserialize(row.payload_json.as_bytes()))
        .collect()
}

fn library_stream_id(resource_id: &MediaResourceId) -> String {
    format!("library/{}", resource_id.0)
}

#[derive(FromQueryResult)]
struct OwnerRow {
    value: String,
}
#[derive(FromQueryResult)]
struct JsonRow {
    payload_json: String,
}

#[derive(FromQueryResult)]
struct LibraryRow {
    resource_id: String,
    favorite: i32,
    pinned: i32,
    last_opened_at: Option<String>,
    progress_json: Option<String>,
}

#[derive(FromQueryResult)]
struct LibraryProjectionRow {
    resource_id: String,
    favorite: i32,
    pinned: i32,
    last_opened_at: Option<String>,
    progress_json: Option<String>,
    updated_global_seq: i64,
    revision: i64,
}
