//! 随机 secret artifact identity 与 ownership/ref-count。
//!
//! plaintext 只在 artifact port 局部值中存在。SQLite 只保存随机
//! `SecretArtifactId`、随机 blob locator、key ID 与随机 nonce 密文 hash；owner row 是 ref-count
//! 的唯一证明。

use std::collections::HashSet;
use std::str::FromStr;

use crate::database::DatabaseSession;
use crate::database::OptionalResultExt;
use crate::database::statement;
use sea_orm::FromQueryResult;
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingSecretArtifact};
use crate::repository::event::{database_error, to_i64};
use crate::types::{SecretArtifactId, StorageError};

pub(crate) const SECRET_ARTIFACT_SCHEMA_VERSION: u32 = 1;
pub(crate) const VAULT_KEY_SCHEMA_VERSION: u32 = 1;

pub(crate) struct VaultKey {
    pub(crate) key_id: String,
    pub(crate) key: Vec<u8>,
}

#[derive(FromQueryResult)]
pub(crate) struct SecretArtifactRow {
    pub(crate) blob_locator: String,
    pub(crate) key_id: String,
    pub(crate) ciphertext_hash: String,
}

#[derive(FromQueryResult)]
struct VaultKeyRow {
    key_id: String,
    verifier: String,
}

#[derive(FromQueryResult)]
struct OwnerRow {
    secret_id: String,
}

#[derive(FromQueryResult)]
struct CountRow {
    value: i64,
}

/// 读取或首次创建随机 vault key，并验证 `SQLite` verifier。
pub(crate) async fn ensure_vault_key(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    created_at_ms: i64,
) -> Result<VaultKey, StorageError> {
    let existing = statement("SELECT key_id, verifier FROM vault_key_metadata WHERE id = 1")
        .get_result::<VaultKeyRow>(conn)
        .await
        .optional()
        .map_err(database_error)?;
    if let Some(existing) = existing {
        let key = artifacts.load_vault_key(&existing.key_id)?;
        if ArtifactStore::key_verifier(&key)? != existing.verifier {
            return Err(StorageError::KeyLost);
        }
        return Ok(VaultKey {
            key_id: existing.key_id,
            key,
        });
    }

    let key_id = Uuid::new_v4().hyphenated().to_string();
    let key = artifacts.create_vault_key(&key_id)?;
    let verifier = ArtifactStore::key_verifier(&key)?;
    let insert_key_id = key_id.clone();
    let insert = conn.immediate_transaction(|conn| Box::pin(async move {
        statement(
            "INSERT INTO vault_key_metadata (id, key_id, verifier, schema_version, created_at_ms) VALUES (1, ?, ?, ?, ?)",
        )
        .bind(&insert_key_id)
        .bind(&verifier)
        .bind(i64::from(VAULT_KEY_SCHEMA_VERSION))
        .bind(created_at_ms)
        .execute(conn).await
        .map_err(database_error)?;
        Ok(())
    })).await;
    if let Err(error) = insert {
        let _ = artifacts.delete_vault_key(&key_id);
        return Err(error);
    }
    Ok(VaultKey { key_id, key })
}

/// 在任何 `SQLite` ref mutation 前写好随机 secret 文件。
pub(crate) async fn write_secret(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    plaintext: &[u8],
    created_at_ms: i64,
) -> Result<PendingSecretArtifact, StorageError> {
    let vault_key = ensure_vault_key(conn, artifacts, created_at_ms).await?;
    artifacts.write_secret(&vault_key.key_id, &vault_key.key, plaintext)
}

/// 把 pending secret 与一个唯一 owner 原子认领。
pub(crate) async fn retain_pending_secret(
    conn: &mut DatabaseSession,
    pending: &PendingSecretArtifact,
    owner_kind: &str,
    owner_id: &str,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    validate_owner(owner_kind, owner_id)?;
    insert_pending_secret(conn, pending, 1, created_at_ms).await?;
    statement(
        "INSERT INTO secret_artifact_owners (owner_kind, owner_id, secret_id, created_at_ms) VALUES (?, ?, ?, ?)",
    )
    .bind(owner_kind)
    .bind(owner_id)
    .bind(pending.secret_id.to_string())
    .bind(created_at_ms)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

async fn insert_pending_secret(
    conn: &mut DatabaseSession,
    pending: &PendingSecretArtifact,
    ref_count: u64,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    statement(
        "INSERT INTO secret_artifact_projection (secret_id, blob_locator, key_id, ciphertext_hash, stored_bytes, ref_count, schema_version, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(pending.secret_id.to_string())
    .bind(&pending.blob_locator)
    .bind(&pending.key_id)
    .bind(&pending.ciphertext_hash)
    .bind(to_i64(pending.stored_bytes)?)
    .bind(to_i64(ref_count)?)
    .bind(i64::from(SECRET_ARTIFACT_SCHEMA_VERSION))
    .bind(created_at_ms)
    .execute(conn).await
    .map_err(database_error)?;
    Ok(())
}

/// 为已有 secret 增加唯一 owner；相同 owner/secret 的重试幂等。
pub(crate) async fn retain_existing_secret(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    secret_id: SecretArtifactId,
    owner_kind: &str,
    owner_id: &str,
    created_at_ms: i64,
) -> Result<(), StorageError> {
    validate_owner(owner_kind, owner_id)?;
    let row = secret_artifact_row(conn, secret_id)
        .await?
        .ok_or_else(|| StorageError::ArtifactUnavailable(secret_id.to_string()))?;
    artifacts.ensure_vault_secret_exists(secret_id, &row.blob_locator, &row.ciphertext_hash)?;
    let existing = owner_secret(conn, owner_kind, owner_id).await?;
    if let Some(existing) = existing {
        return if existing == secret_id {
            Ok(())
        } else {
            Err(StorageError::SecretOwnershipMismatch)
        };
    }
    statement(
        "INSERT INTO secret_artifact_owners (owner_kind, owner_id, secret_id, created_at_ms) VALUES (?, ?, ?, ?)",
    )
    .bind(owner_kind)
    .bind(owner_id)
    .bind(secret_id.to_string())
    .bind(created_at_ms)
    .execute(conn).await
    .map_err(database_error)?;
    let changed = statement(
        "UPDATE secret_artifact_projection SET ref_count = ref_count + 1 WHERE secret_id = ?",
    )
    .bind(secret_id.to_string())
    .execute(conn)
    .await
    .map_err(database_error)?;
    if changed != 1 {
        return Err(StorageError::ArtifactUnavailable(secret_id.to_string()));
    }
    Ok(())
}

/// 释放 owner；不存在视为幂等成功。
pub(crate) async fn release_secret_owner(
    conn: &mut DatabaseSession,
    owner_kind: &str,
    owner_id: &str,
) -> Result<(), StorageError> {
    let Some(secret_id) = owner_secret(conn, owner_kind, owner_id).await? else {
        return Ok(());
    };
    let deleted =
        statement("DELETE FROM secret_artifact_owners WHERE owner_kind = ? AND owner_id = ?")
            .bind(owner_kind)
            .bind(owner_id)
            .execute(conn)
            .await
            .map_err(database_error)?;
    if deleted != 1 {
        return Err(StorageError::Database(
            "secret owner 删除不一致".to_string(),
        ));
    }
    let changed = statement(
        "UPDATE secret_artifact_projection SET ref_count = ref_count - 1 WHERE secret_id = ? AND ref_count > 0",
    )
    .bind(secret_id.to_string())
    .execute(conn).await
    .map_err(database_error)?;
    if changed != 1 {
        return Err(StorageError::ArtifactUnavailable(secret_id.to_string()));
    }
    Ok(())
}

/// 读取并认证随机 secret artifact。
pub(crate) async fn read_secret(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    secret_id: SecretArtifactId,
) -> Result<Vec<u8>, StorageError> {
    let row = secret_artifact_row(conn, secret_id)
        .await?
        .ok_or_else(|| StorageError::ArtifactUnavailable(secret_id.to_string()))?;
    artifacts.read_secret_artifact(
        secret_id,
        &row.blob_locator,
        &row.key_id,
        &row.ciphertext_hash,
    )
}

/// 仅当调用方给出的 owner 行恰好认领该随机 ID 时才解密，防止跨记录 ID 替换形成
/// secret-ref oracle。
pub(crate) async fn read_owned_secret(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
    secret_id: SecretArtifactId,
    owner_kind: &str,
    owner_id: &str,
) -> Result<Vec<u8>, StorageError> {
    validate_owner(owner_kind, owner_id)?;
    if owner_secret(conn, owner_kind, owner_id).await? != Some(secret_id) {
        return Err(StorageError::SecretOwnershipMismatch);
    }
    read_secret(conn, artifacts, secret_id).await
}

pub(crate) async fn secret_artifact_row(
    conn: &mut DatabaseSession,
    secret_id: SecretArtifactId,
) -> Result<Option<SecretArtifactRow>, StorageError> {
    statement(
        "SELECT secret_id, blob_locator, key_id, ciphertext_hash, ref_count FROM secret_artifact_projection WHERE secret_id = ?",
    )
    .bind(secret_id.to_string())
    .get_result::<SecretArtifactRow>(conn).await
    .optional()
    .map_err(database_error)
}

async fn owner_secret(
    conn: &mut DatabaseSession,
    owner_kind: &str,
    owner_id: &str,
) -> Result<Option<SecretArtifactId>, StorageError> {
    let row = statement(
        "SELECT secret_id FROM secret_artifact_owners WHERE owner_kind = ? AND owner_id = ?",
    )
    .bind(owner_kind)
    .bind(owner_id)
    .get_result::<OwnerRow>(conn)
    .await
    .optional()
    .map_err(database_error)?;
    row.map(|row| SecretArtifactId::from_str(&row.secret_id))
        .transpose()
}

/// 删除 ref-count 为零的随机 secret metadata/file；重复运行幂等。
pub(crate) async fn purge_zero_ref_secrets(
    conn: &mut DatabaseSession,
    artifacts: &ArtifactStore,
) -> Result<usize, StorageError> {
    let rows = statement(
        "SELECT secret_id, blob_locator, key_id, ciphertext_hash, ref_count FROM secret_artifact_projection WHERE ref_count = 0 ORDER BY created_at_ms ASC",
    )
    .load::<SecretArtifactRow>(conn).await
    .map_err(database_error)?;
    conn.immediate_transaction(|conn| {
        Box::pin(async move {
            statement("DELETE FROM secret_artifact_projection WHERE ref_count = 0")
                .execute(conn)
                .await
                .map_err(database_error)?;
            Ok(())
        })
    })
    .await?;
    for row in &rows {
        artifacts.remove_file(&row.blob_locator)?;
    }
    Ok(rows.len())
}

/// 返回 orphan recovery 必须保留的随机 secret blob locators。
pub(crate) async fn referenced_secret_paths(
    conn: &mut DatabaseSession,
) -> Result<HashSet<String>, StorageError> {
    let rows = statement(
        "SELECT secret_id, blob_locator, key_id, ciphertext_hash, ref_count FROM secret_artifact_projection",
    )
    .load::<SecretArtifactRow>(conn).await
    .map_err(database_error)?;
    Ok(rows.into_iter().map(|row| row.blob_locator).collect())
}

/// 验证每个随机 secret 的 `ref_count` 等于 owner 行数。
pub(crate) async fn validate_secret_ref_counts(
    conn: &mut DatabaseSession,
) -> Result<(), StorageError> {
    let inconsistent = statement(
        "SELECT COUNT(*) AS value FROM secret_artifact_projection AS secret WHERE secret.ref_count != (SELECT COUNT(*) FROM secret_artifact_owners AS owner WHERE owner.secret_id = secret.secret_id)",
    )
    .get_result::<CountRow>(conn).await
    .map_err(database_error)?;
    if inconsistent.value == 0 {
        Ok(())
    } else {
        Err(StorageError::SecretOwnershipCorrupt)
    }
}

fn validate_owner(owner_kind: &str, owner_id: &str) -> Result<(), StorageError> {
    if owner_kind.is_empty() || owner_id.is_empty() {
        Err(StorageError::InvalidInput(
            "secret owner 不能为空".to_string(),
        ))
    } else {
        Ok(())
    }
}
