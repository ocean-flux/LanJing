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
use keyring::{Entry, Error as KeyringError};
use uuid::Uuid;

use crate::types::{ArtifactKind, OrphanRecovery, StorageError};

const MASTER_KEY_ACCOUNT: &str = "master-key-v1";
const SECRET_FILE_VERSION: u8 = 1;
const AES_GCM_NONCE_LEN: usize = 12;

/// 已写入磁盘、尚待 `SQLite` ref transaction 认领的 artifact metadata。
#[derive(Debug, Clone)]
pub(crate) struct PendingArtifact {
    pub(crate) hash: String,
    pub(crate) kind: ArtifactKind,
    pub(crate) codec: String,
    pub(crate) encryption: Option<String>,
    pub(crate) relative_path: String,
    pub(crate) stored_bytes: u64,
}

/// 独占 artifact 根目录与同一安装级 keyring credential 的同步实现。
#[derive(Clone)]
pub(crate) struct ArtifactStore {
    root: PathBuf,
    master_key_entry: Arc<Entry>,
}

impl ArtifactStore {
    /// 创建 artifact 根目录描述，并固定同一个 keyring credential 实例。
    ///
    /// 固定实例既符合生产 keyring 的安装级语义，也让 keyring 官方 mock 在同一 storage
    /// 生命周期内正确模拟持久凭据。
    pub(crate) fn new(root: PathBuf, keyring_service: &str) -> Result<Self, StorageError> {
        let master_key_entry =
            Entry::new(keyring_service, MASTER_KEY_ACCOUNT).map_err(|_| StorageError::Keyring)?;
        Ok(Self {
            root,
            master_key_entry: Arc::new(master_key_entry),
        })
    }

    /// 将逻辑内容写成 body 或 secret artifact。
    ///
    /// 文件写入顺序固定为 temp → write → flush → fsync → rename。返回的 metadata 尚未
    /// 被数据库认领，调用方必须紧接着在 Event/ref transaction 中使用它。
    pub(crate) fn write(
        &self,
        kind: ArtifactKind,
        logical_bytes: &[u8],
    ) -> Result<PendingArtifact, StorageError> {
        let hash = blake3::hash(logical_bytes).to_hex().to_string();
        let (stored, codec, encryption, extension) = match kind {
            ArtifactKind::Body => (
                zstd::stream::encode_all(Cursor::new(logical_bytes), 3).map_err(file_error)?,
                "zstd".to_string(),
                None,
                "zst",
            ),
            ArtifactKind::Secret => (
                self.encrypt_secret(logical_bytes)?,
                "none".to_string(),
                Some("aes-256-gcm".to_string()),
                "secret",
            ),
        };
        let relative = Self::relative_path(kind, &hash, extension)?;
        let target = self.root.join(&relative);
        let stored_bytes = Self::atomic_write(&target, &stored)?;
        Ok(PendingArtifact {
            hash,
            kind,
            codec,
            encryption,
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

    /// 读取并认证/解密 secret artifact。
    pub(crate) fn read_secret(
        &self,
        hash: &str,
        relative_path: &str,
    ) -> Result<Vec<u8>, StorageError> {
        let encrypted = self.read_file(relative_path, hash)?;
        if encrypted.len() <= 1 + AES_GCM_NONCE_LEN || encrypted[0] != SECRET_FILE_VERSION {
            return Err(StorageError::SecretUnavailable);
        }
        let key = self.master_key(false)?;
        let cipher =
            Aes256Gcm::new_from_slice(&key).map_err(|_| StorageError::SecretUnavailable)?;
        let nonce = Nonce::try_from(&encrypted[1..=AES_GCM_NONCE_LEN])
            .map_err(|_| StorageError::SecretUnavailable)?;
        let logical_bytes = cipher
            .decrypt(&nonce, &encrypted[1 + AES_GCM_NONCE_LEN..])
            .map_err(|_| StorageError::SecretUnavailable)?;
        verify_logical_hash(&logical_bytes, hash).map_err(|_| StorageError::SecretUnavailable)?;
        Ok(logical_bytes)
    }

    /// 验证安装级 secret 主密钥可用，而不读取或解密任何 Secret Artifact。
    ///
    /// # Errors
    ///
    /// keyring 缺少或拒绝该主密钥时返回 [`StorageError`]。
    pub(crate) fn ensure_secret_key_available(&self) -> Result<(), StorageError> {
        self.master_key(false).map(|_| ())
    }

    /// 验证 Secret Artifact 文件存在且具有受支持的密文 envelope，不解密其内容。
    ///
    /// # Errors
    ///
    /// artifact 文件缺失、不可读取或 envelope 损坏时返回 [`StorageError`]。
    pub(crate) fn ensure_secret_artifact_exists(
        &self,
        hash: &str,
        relative_path: &str,
    ) -> Result<(), StorageError> {
        let encrypted = self.read_file(relative_path, hash)?;
        if encrypted.len() <= 1 + AES_GCM_NONCE_LEN || encrypted[0] != SECRET_FILE_VERSION {
            return Err(StorageError::SecretUnavailable);
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

    fn encrypt_secret(&self, logical_bytes: &[u8]) -> Result<Vec<u8>, StorageError> {
        let key = self.master_key(true)?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| StorageError::Keyring)?;
        let nonce = Nonce::generate();
        let ciphertext = cipher
            .encrypt(&nonce, logical_bytes)
            .map_err(|_| StorageError::SecretUnavailable)?;
        let mut stored = Vec::with_capacity(1 + nonce.len() + ciphertext.len());
        stored.push(SECRET_FILE_VERSION);
        stored.extend_from_slice(&nonce);
        stored.extend_from_slice(&ciphertext);
        Ok(stored)
    }

    fn master_key(&self, create: bool) -> Result<Vec<u8>, StorageError> {
        match self.master_key_entry.get_password() {
            Ok(encoded) => decode_key(&encoded),
            Err(KeyringError::NoEntry) if create => {
                let key = Key::<Aes256Gcm>::generate();
                let encoded = encode_hex(&key);
                self.master_key_entry
                    .set_password(&encoded)
                    .map_err(|_| StorageError::Keyring)?;
                decode_key(&encoded)
            }
            Err(KeyringError::NoEntry) => Err(StorageError::MasterKeyUnavailable),
            Err(_) => Err(StorageError::Keyring),
        }
    }

    fn relative_path(
        kind: ArtifactKind,
        hash: &str,
        extension: &str,
    ) -> Result<PathBuf, StorageError> {
        if hash.len() != 64 || !hash.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(StorageError::InvalidInput(
                "artifact hash 不是 BLAKE3 hex".to_string(),
            ));
        }
        let category = match kind {
            ArtifactKind::Body => "body",
            ArtifactKind::Secret => "secret",
        };
        Ok(PathBuf::from(category)
            .join(&hash[..2])
            .join(&hash[2..4])
            .join(format!("{hash}.{extension}")))
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

    fn read_file(&self, relative_path: &str, hash: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.root.join(relative_path);
        let mut file = File::open(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StorageError::ArtifactUnavailable(hash.to_string())
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
        return Err(StorageError::MasterKeyUnavailable);
    }
    let mut key = Vec::with_capacity(32);
    for chunk in encoded.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(chunk).map_err(|_| StorageError::MasterKeyUnavailable)?;
        let byte = u8::from_str_radix(text, 16).map_err(|_| StorageError::MasterKeyUnavailable)?;
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

fn file_error(error: std::io::Error) -> StorageError {
    let message = error.to_string();
    let _ = error.into_inner();
    StorageError::FileSystem(message)
}
