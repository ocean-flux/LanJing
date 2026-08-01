//! 文件 artifact 与安装级 keyring 主密钥。
//!
//! 本模块只能由单 writer blocking lane 调用。它先把内容写成 durable 文件，再由调用方
//! 在同一写入流程中建立 `SQLite` metadata/ref；若后者失败，下一次启动的 orphan sweeper
//! 会删除没有 metadata 的文件。

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aes_gcm::aead::{Aead, Generate, Key, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use keyring_core::{Entry, Error as KeyringError};
use uuid::Uuid;

use crate::types::{ArtifactKind, OrphanRecovery, SecretArtifactId, StorageError};

const VAULT_KEY_ACCOUNT_PREFIX: &str = "vault-key-v1/";
const VAULT_SECRET_FILE_VERSION: u8 = 2;
const AES_GCM_NONCE_LEN: usize = 12;

/// 已写入磁盘、尚待 `SQLite` ref transaction 认领的 artifact metadata。
#[derive(Debug, Clone)]
pub(crate) struct PendingArtifact {
    pub(crate) hash: String,
    pub(crate) kind: ArtifactKind,
    pub(crate) codec: String,
    pub(crate) relative_path: String,
    pub(crate) stored_bytes: u64,
}

/// 已写入随机 locator、尚待 ownership transaction 认领的 secret metadata。
///
/// 此类型不实现 `Debug`，避免未来新增敏感 metadata 后被整值记录。
pub(crate) struct PendingSecretArtifact {
    pub(crate) secret_id: SecretArtifactId,
    pub(crate) blob_locator: String,
    pub(crate) key_id: String,
    pub(crate) ciphertext_hash: String,
    pub(crate) stored_bytes: u64,
}

/// 独占 artifact 根目录与同一安装级 keyring service 的同步实现。
#[derive(Clone)]
pub(crate) struct ArtifactStore {
    root: PathBuf,
    keyring_service: Arc<str>,
}

impl ArtifactStore {
    /// 创建 artifact 根目录描述，并固定 current vault keyring service。
    pub(crate) fn new(root: PathBuf, keyring_service: &str) -> Result<Self, StorageError> {
        crate::keyring_init::ensure_default_keyring_store()?;
        Ok(Self {
            root,
            keyring_service: Arc::from(keyring_service),
        })
    }

    /// 将非 secret 逻辑内容写成 zstd body artifact。
    ///
    /// 所有敏感路径必须使用随机 [`SecretArtifactId`] 与 [`Self::write_secret`]。
    pub(crate) fn write(
        &self,
        kind: ArtifactKind,
        logical_bytes: &[u8],
    ) -> Result<PendingArtifact, StorageError> {
        let hash = blake3::hash(logical_bytes).to_hex().to_string();
        let stored = zstd::stream::encode_all(Cursor::new(logical_bytes), 3).map_err(file_error)?;
        let relative = Self::relative_path(&hash, "zst")?;
        let target = self.root.join(&relative);
        let stored_bytes = Self::atomic_write(&target, &stored)?;
        Ok(PendingArtifact {
            hash,
            kind,
            codec: "zstd".to_string(),
            relative_path: relative.to_string_lossy().replace('\\', "/"),
            stored_bytes,
        })
    }

    /// 读取并解码 body artifact。
    pub(crate) fn read_body(
        &self,
        hash: &str,
        relative_path: &str,
    ) -> Result<Vec<u8>, StorageError> {
        let bytes = self.read_file(relative_path, hash)?;
        let logical_bytes = zstd::stream::decode_all(Cursor::new(bytes)).map_err(file_error)?;
        verify_logical_hash(&logical_bytes, hash)?;
        Ok(logical_bytes)
    }

    /// 创建随机 key ID 对应的 AES-256 key，并写入平台 secure store。
    pub(crate) fn create_vault_key(&self, key_id: &str) -> Result<Vec<u8>, StorageError> {
        validate_key_id(key_id)?;
        let key = Key::<Aes256Gcm>::generate();
        let encoded = encode_hex(&key);
        self.vault_key_entry(key_id)?
            .set_password(&encoded)
            .map_err(|error| map_keyring_operation_error(&error))?;
        decode_key(&encoded).map_err(|_| StorageError::KeyLost)
    }

    /// 读取 `SQLite` 固定 key ID 对应的 AES-256 key。
    pub(crate) fn load_vault_key(&self, key_id: &str) -> Result<Vec<u8>, StorageError> {
        validate_key_id(key_id)?;
        let encoded = self
            .vault_key_entry(key_id)?
            .get_password()
            .map_err(map_keyring_load_error)?;
        decode_key(&encoded).map_err(|_| StorageError::KeyLost)
    }

    /// 回滚尚未被 `SQLite` 认领的随机 key；不存在视为幂等成功。
    pub(crate) fn delete_vault_key(&self, key_id: &str) -> Result<(), StorageError> {
        validate_key_id(key_id)?;
        match self.vault_key_entry(key_id)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(map_keyring_operation_error(&error)),
        }
    }

    /// 计算随机高熵 key 的独立 verifier；该值不能用于推导任何文档或 credential plaintext。
    pub(crate) fn key_verifier(key: &[u8]) -> Result<String, StorageError> {
        if key.len() != 32 {
            return Err(StorageError::KeyLost);
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"lanjing-vault-key-verifier-v1");
        hasher.update(key);
        Ok(hasher.finalize().to_hex().to_string())
    }

    /// 用已验证 key 写入随机 identity、随机 locator 的 AES-256-GCM secret。
    ///
    /// ciphertext hash 只覆盖带随机 nonce 的密文 envelope，不再对 plaintext 形成可预计算 oracle。
    pub(crate) fn write_secret(
        &self,
        key_id: &str,
        key: &[u8],
        logical_bytes: &[u8],
    ) -> Result<PendingSecretArtifact, StorageError> {
        validate_key_id(key_id)?;
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| StorageError::KeyLost)?;
        let nonce = Nonce::generate();
        let ciphertext = cipher
            .encrypt(&nonce, logical_bytes)
            .map_err(|_| StorageError::ArtifactCorrupt)?;
        let mut stored = Vec::with_capacity(1 + nonce.len() + ciphertext.len());
        stored.push(VAULT_SECRET_FILE_VERSION);
        stored.extend_from_slice(&nonce);
        stored.extend_from_slice(&ciphertext);

        let secret_id = SecretArtifactId::new();
        let locator = Uuid::new_v4().simple().to_string();
        let relative = Self::secret_relative_path(&locator)?;
        let target = self.root.join(&relative);
        let stored_bytes = Self::atomic_write(&target, &stored)?;
        Ok(PendingSecretArtifact {
            secret_id,
            blob_locator: relative.to_string_lossy().replace('\\', "/"),
            key_id: key_id.to_string(),
            ciphertext_hash: blake3::hash(&stored).to_hex().to_string(),
            stored_bytes,
        })
    }

    /// 按随机 identity/locator 认证并解密 secret artifact。
    pub(crate) fn read_secret_artifact(
        &self,
        secret_id: SecretArtifactId,
        blob_locator: &str,
        key_id: &str,
        ciphertext_hash: &str,
    ) -> Result<Vec<u8>, StorageError> {
        ensure_blake3_hex(ciphertext_hash)?;
        let encrypted = self.read_secret_file(blob_locator, secret_id)?;
        if blake3::hash(&encrypted).to_hex().as_str() != ciphertext_hash {
            return Err(StorageError::ArtifactCorrupt);
        }
        if encrypted.len() <= 1 + AES_GCM_NONCE_LEN || encrypted[0] != VAULT_SECRET_FILE_VERSION {
            return Err(StorageError::ArtifactCorrupt);
        }
        let key = self.load_vault_key(key_id)?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| StorageError::KeyLost)?;
        let nonce = Nonce::try_from(&encrypted[1..=AES_GCM_NONCE_LEN])
            .map_err(|_| StorageError::ArtifactCorrupt)?;
        cipher
            .decrypt(&nonce, &encrypted[1 + AES_GCM_NONCE_LEN..])
            .map_err(|_| StorageError::ArtifactCorrupt)
    }

    /// 验证随机 secret 文件和 ciphertext hash，不解密 plaintext。
    pub(crate) fn ensure_vault_secret_exists(
        &self,
        secret_id: SecretArtifactId,
        blob_locator: &str,
        ciphertext_hash: &str,
    ) -> Result<(), StorageError> {
        ensure_blake3_hex(ciphertext_hash)?;
        let encrypted = self.read_secret_file(blob_locator, secret_id)?;
        if encrypted.len() <= 1 + AES_GCM_NONCE_LEN
            || encrypted[0] != VAULT_SECRET_FILE_VERSION
            || blake3::hash(&encrypted).to_hex().as_str() != ciphertext_hash
        {
            return Err(StorageError::ArtifactCorrupt);
        }
        Ok(())
    }

    /// 从根目录删除 temp 或没有 `SQLite` metadata 的 orphan 文件。
    pub(crate) fn recover_orphans(
        &self,
        referenced_relative_paths: &HashSet<String>,
    ) -> Result<OrphanRecovery, StorageError> {
        if !self.root.exists() {
            return Ok(OrphanRecovery::default());
        }
        let mut files = Vec::new();
        collect_files(&self.root, &mut files)?;
        let mut result = OrphanRecovery::default();
        for file in files {
            let relative = file
                .strip_prefix(&self.root)
                .map_err(|_| StorageError::FileSystem("artifact 路径不在根目录内".to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            let is_temp = file.extension().is_some_and(|extension| extension == "tmp");
            if is_temp || !referenced_relative_paths.contains(&relative) {
                fs::remove_file(&file).map_err(file_error)?;
                result.removed_files += 1;
            } else {
                result.referenced_files += 1;
            }
        }
        Ok(result)
    }

    /// 删除一个已在 `SQLite` 中去引用的 artifact 文件；不存在视为幂等成功。
    pub(crate) fn remove_file(&self, relative_path: &str) -> Result<(), StorageError> {
        let path = self.root.join(relative_path);
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(file_error(error)),
        }
    }

    fn vault_key_entry(&self, key_id: &str) -> Result<Entry, StorageError> {
        Entry::new(
            &self.keyring_service,
            &format!("{VAULT_KEY_ACCOUNT_PREFIX}{key_id}"),
        )
        .map_err(|error| map_keyring_setup_error(&error))
    }

    fn relative_path(hash: &str, extension: &str) -> Result<PathBuf, StorageError> {
        if hash.len() != 64 || !hash.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(StorageError::InvalidInput(
                "artifact hash 不是 BLAKE3 hex".to_string(),
            ));
        }
        Ok(PathBuf::from("body")
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(format!("{hash}.{extension}")))
    }

    fn secret_relative_path(locator: &str) -> Result<PathBuf, StorageError> {
        if locator.len() != 32 || !locator.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(StorageError::InvalidInput(
                "secret blob locator 无效".to_string(),
            ));
        }
        Ok(PathBuf::from("vault")
            .join(&locator[..2])
            .join(&locator[2..4])
            .join(format!("{locator}.vault")))
    }

    fn atomic_write(target: &Path, bytes: &[u8]) -> Result<u64, StorageError> {
        if target.exists() {
            return fs::metadata(target)
                .map(|metadata| metadata.len())
                .map_err(file_error);
        }
        let parent = target
            .parent()
            .ok_or_else(|| StorageError::FileSystem("artifact 目标路径没有父目录".to_string()))?;
        fs::create_dir_all(parent).map_err(file_error)?;
        let file_name = target
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| StorageError::FileSystem("artifact 文件名无效".to_string()))?;
        let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
        let write_result = (|| -> Result<(), StorageError> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(file_error)?;
            file.write_all(bytes).map_err(file_error)?;
            file.flush().map_err(file_error)?;
            file.sync_all().map_err(file_error)?;
            drop(file);
            fs::rename(&temporary, target).map_err(file_error)?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write_result?;
        u64::try_from(bytes.len())
            .map_err(|_| StorageError::FileSystem("artifact 大小超过 u64".to_string()))
    }

    fn read_file(&self, relative_path: &str, identity: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.root.join(relative_path);
        let mut file = File::open(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StorageError::ArtifactUnavailable(identity.to_string())
            } else {
                file_error(error)
            }
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(file_error)?;
        Ok(bytes)
    }

    fn read_secret_file(
        &self,
        relative_path: &str,
        secret_id: SecretArtifactId,
    ) -> Result<Vec<u8>, StorageError> {
        let path = self.root.join(relative_path);
        let mut file = File::open(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StorageError::ArtifactUnavailable(secret_id.to_string())
            } else {
                file_error(error)
            }
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(file_error)?;
        Ok(bytes)
    }
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), StorageError> {
    for entry in fs::read_dir(root).map_err(file_error)? {
        let entry = entry.map_err(file_error)?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(file_error)?;
        if file_type.is_dir() {
            collect_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

fn decode_key(encoded: &str) -> Result<Vec<u8>, StorageError> {
    if encoded.len() != 64 || !encoded.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(StorageError::KeyLost);
    }
    let mut key = Vec::with_capacity(32);
    for chunk in encoded.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(chunk).map_err(|_| StorageError::KeyLost)?;
        let byte = u8::from_str_radix(text, 16).map_err(|_| StorageError::KeyLost)?;
        key.push(byte);
    }
    Ok(key)
}

fn verify_logical_hash(bytes: &[u8], expected: &str) -> Result<(), StorageError> {
    if blake3::hash(bytes).to_hex().as_str() == expected {
        Ok(())
    } else {
        Err(StorageError::ArtifactUnavailable(expected.to_string()))
    }
}

fn validate_key_id(key_id: &str) -> Result<(), StorageError> {
    let parsed = Uuid::parse_str(key_id).map_err(|_| StorageError::KeyLost)?;
    if parsed.hyphenated().to_string() != key_id {
        return Err(StorageError::KeyLost);
    }
    Ok(())
}

fn ensure_blake3_hex(value: &str) -> Result<(), StorageError> {
    if value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        Ok(())
    } else {
        Err(StorageError::ArtifactCorrupt)
    }
}

fn map_keyring_setup_error(error: &KeyringError) -> StorageError {
    match error {
        KeyringError::NoStorageAccess(_) => StorageError::KeyringLocked,
        KeyringError::NoDefaultStore | KeyringError::NotSupportedByStore(_) => {
            StorageError::KeyringUnavailable
        }
        _ => StorageError::KeyringUnavailable,
    }
}

fn map_keyring_operation_error(error: &KeyringError) -> StorageError {
    match error {
        KeyringError::NoStorageAccess(_) => StorageError::KeyringLocked,
        KeyringError::NoDefaultStore | KeyringError::NotSupportedByStore(_) => {
            StorageError::KeyringUnavailable
        }
        KeyringError::NoEntry
        | KeyringError::BadEncoding(_)
        | KeyringError::BadDataFormat(_, _) => StorageError::KeyLost,
        _ => StorageError::KeyringUnavailable,
    }
}

fn map_keyring_load_error(error: KeyringError) -> StorageError {
    match error {
        KeyringError::NoEntry => StorageError::KeyLost,
        other => map_keyring_operation_error(&other),
    }
}

fn file_error(error: std::io::Error) -> StorageError {
    let message = error.to_string();
    let _ = error.into_inner();
    StorageError::FileSystem(message)
}

#[cfg(test)]
mod tests {
    use keyring_core::Error as KeyringError;

    use super::{ensure_blake3_hex, map_keyring_load_error, map_keyring_setup_error};
    use crate::types::StorageError;

    #[test]
    fn keyring_and_ciphertext_failures_keep_four_typed_states() {
        assert!(matches!(
            map_keyring_setup_error(&KeyringError::NoDefaultStore),
            StorageError::KeyringUnavailable
        ));
        assert!(matches!(
            map_keyring_setup_error(&KeyringError::NoStorageAccess(Box::new(
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "locked"),
            ))),
            StorageError::KeyringLocked
        ));
        assert!(matches!(
            map_keyring_load_error(KeyringError::NoEntry),
            StorageError::KeyLost
        ));
        assert!(matches!(
            ensure_blake3_hex("not-a-ciphertext-hash"),
            Err(StorageError::ArtifactCorrupt)
        ));
    }
}
