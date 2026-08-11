//! Project-selectable content-addressed object storage.
//!
//! Resource bytes never belong in the relational database. Drafts and
//! immutable releases share this store; the database keeps only object keys,
//! digests, sizes and portable file manifests.

use std::collections::BTreeSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use conductor_domain::{DraftFile, ResourceKind, StorageBackend, StorageSettings};
use conductor_storage::repos::LogoArtifact;
use conductor_storage::Db;
use object_store::aws::AmazonS3Builder;
use object_store::azure::MicrosoftAzureBuilder;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use sha2::{Digest, Sha256};
use sqlx::Row;
use tokio::sync::RwLock;

const DEFAULT_OBJECT_DIR: &str = "objects";

#[derive(Clone)]
pub struct ArtifactStore {
    active: Arc<RwLock<ConfiguredStore>>,
    data_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredArtifact {
    pub key: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct StorageMigrationStats {
    pub objects_copied: u64,
    pub bytes_copied: u64,
}

#[derive(Clone)]
struct ConfiguredStore {
    settings: StorageSettings,
    store: Arc<dyn ObjectStore>,
    prefix: String,
}

impl ArtifactStore {
    pub async fn from_settings(settings: StorageSettings) -> anyhow::Result<Self> {
        let data_root = std::env::var("CONDUCTOR_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data"));
        let active = build_store(&settings, &data_root).await?;
        Ok(Self {
            active: Arc::new(RwLock::new(active)),
            data_root,
        })
    }

    #[cfg(test)]
    pub async fn new(root: PathBuf) -> Self {
        Self::from_settings(StorageSettings {
            local: conductor_domain::LocalStorageSettings {
                root: Some(root.to_string_lossy().into_owned()),
            },
            ..StorageSettings::default()
        })
        .await
        .expect("test object storage")
    }

    pub async fn settings(&self) -> StorageSettings {
        self.active.read().await.settings.clone()
    }

    pub async fn put_bundle(&self, files: &[DraftFile]) -> anyhow::Result<StoredArtifact> {
        let bytes = deterministic_zip(files)?;
        self.put(&bytes).await
    }

    pub async fn put(&self, bytes: &[u8]) -> anyhow::Result<StoredArtifact> {
        let digest = hex::encode(Sha256::digest(bytes));
        let key = format!("sha256/{}/{}", &digest[..2], digest);
        let active = self.active.read().await;
        put_to(&active, &key, bytes.to_vec()).await?;
        Ok(StoredArtifact {
            key,
            sha256: digest,
            size: bytes.len().try_into().unwrap_or(u64::MAX),
        })
    }

    pub async fn read(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        validate_content_key(key)?;
        let active = self.active.read().await;
        read_from(&active, key).await
    }

    pub async fn health_check(&self) -> anyhow::Result<()> {
        let active = self.active.read().await;
        health_check_store(&active).await
    }

    /// One-time compatibility migration for pre-object-store databases. Every
    /// payload containing authored `files[].content` is externalized and then
    /// replaced with a manifest-only payload.
    pub async fn externalize_legacy_payloads(&self, db: &Db) -> anyhow::Result<u64> {
        let mut migrated = 0_u64;
        if let Some(logo_url) =
            sqlx::query_scalar::<_, Option<String>>("SELECT logo_url FROM instance LIMIT 1")
                .fetch_optional(db.pool())
                .await?
                .flatten()
        {
            if let Some((media_type, bytes)) = decode_data_logo(&logo_url) {
                let artifact = self.put(&bytes).await?;
                db.instance()
                    .update_logo_artifact(Some(&LogoArtifact {
                        key: artifact.key,
                        sha256: artifact.sha256,
                        size: artifact.size,
                        media_type,
                    }))
                    .await?;
                migrated = migrated.saturating_add(1);
            }
        }
        let resources = sqlx::query(
            r#"
            SELECT id, kind, slug, version, payload
            FROM resources
            "#,
        )
        .fetch_all(db.pool())
        .await?;
        for row in resources {
            let payload_text: String = row.get("payload");
            let files = legacy_files(&payload_text);
            if files.is_empty() {
                continue;
            }
            let kind = ResourceKind::parse(row.get::<String, _>("kind").as_str())
                .ok_or_else(|| anyhow!("legacy resource has unknown kind"))?;
            let slug: String = row.get("slug");
            let version: String = row.get("version");
            let artifact = self.put_bundle(&files).await?;
            let metadata = crate::core::resource_authoring::resource_storage_payload(
                kind,
                &slug,
                &version,
                &artifact.key,
                &artifact.sha256,
                artifact.size,
                crate::core::resource_authoring::resource_archive_media_type(kind),
                &files,
            );
            let updated = sqlx::query(
                r#"
                UPDATE resources
                SET payload = ?, draft_artifact_key = ?, draft_content_sha256 = ?,
                    draft_content_size = ?
                WHERE id = ? AND payload = ?
                "#,
            )
            .bind(serde_json::to_string(&metadata)?)
            .bind(&artifact.key)
            .bind(&artifact.sha256)
            .bind(i64::try_from(artifact.size).unwrap_or(i64::MAX))
            .bind(row.get::<String, _>("id"))
            .bind(&payload_text)
            .execute(db.pool())
            .await?;
            migrated = migrated.saturating_add(updated.rows_affected());
        }

        let versions = sqlx::query(
            r#"
            SELECT rv.id, rv.resource_id, rv.version, rv.payload,
                   r.kind, r.slug, r.version AS current_version, r.status AS resource_status
            FROM resource_versions rv
            JOIN resources r ON r.id = rv.resource_id
            "#,
        )
        .fetch_all(db.pool())
        .await?;
        for row in versions {
            let payload_text: String = row.get("payload");
            let files = legacy_files(&payload_text);
            if files.is_empty() {
                continue;
            }
            let kind = ResourceKind::parse(row.get::<String, _>("kind").as_str())
                .ok_or_else(|| anyhow!("legacy version has unknown kind"))?;
            let slug: String = row.get("slug");
            let version: String = row.get("version");
            let artifact = self.put_bundle(&files).await?;
            let metadata = crate::core::resource_authoring::resource_storage_payload(
                kind,
                &slug,
                &version,
                &artifact.key,
                &artifact.sha256,
                artifact.size,
                crate::core::resource_authoring::resource_archive_media_type(kind),
                &files,
            );
            let metadata_text = serde_json::to_string(&metadata)?;
            let updated = sqlx::query(
                r#"
                UPDATE resource_versions
                SET payload = ?, artifact_key = ?, content_sha256 = ?, content_size = ?,
                    artifact_schema_version = '2'
                WHERE id = ? AND payload = ?
                "#,
            )
            .bind(&metadata_text)
            .bind(&artifact.key)
            .bind(&artifact.sha256)
            .bind(i64::try_from(artifact.size).unwrap_or(i64::MAX))
            .bind(row.get::<String, _>("id"))
            .bind(&payload_text)
            .execute(db.pool())
            .await?;
            migrated = migrated.saturating_add(updated.rows_affected());

            let current_version: String = row.get("current_version");
            let resource_status: String = row.get("resource_status");
            if current_version == version
                && matches!(resource_status.as_str(), "beta" | "published")
            {
                sqlx::query("UPDATE resources SET payload = ? WHERE id = ?")
                    .bind(&metadata_text)
                    .bind(row.get::<String, _>("resource_id"))
                    .execute(db.pool())
                    .await?;
            }
        }
        Ok(migrated)
    }

    /// Copy every referenced object while all normal reads/writes are paused,
    /// persist the new project setting, then atomically switch the live store.
    pub async fn reconfigure<F, Fut>(
        &self,
        settings: StorageSettings,
        keys: Vec<String>,
        persist: F,
    ) -> anyhow::Result<StorageMigrationStats>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let candidate = build_store(&settings, &self.data_root).await?;
        health_check_store(&candidate).await?;

        let mut active = self.active.write().await;
        let mut stats = StorageMigrationStats {
            objects_copied: 0,
            bytes_copied: 0,
        };
        for key in keys.into_iter().collect::<BTreeSet<_>>() {
            validate_content_key(&key)?;
            let bytes = read_from(&active, &key)
                .await
                .with_context(|| format!("read existing object {key}"))?;
            validate_object_digest(&key, &bytes)?;
            put_to(&candidate, &key, bytes.clone())
                .await
                .with_context(|| format!("copy object {key}"))?;
            let copied = read_from(&candidate, &key)
                .await
                .with_context(|| format!("verify copied object {key}"))?;
            validate_object_digest(&key, &copied)?;
            stats.objects_copied = stats.objects_copied.saturating_add(1);
            stats.bytes_copied = stats
                .bytes_copied
                .saturating_add(bytes.len().try_into().unwrap_or(u64::MAX));
        }

        persist().await?;
        *active = candidate;
        Ok(stats)
    }
}

async fn build_store(
    settings: &StorageSettings,
    data_root: &Path,
) -> anyhow::Result<ConfiguredStore> {
    let (store, prefix): (Arc<dyn ObjectStore>, String) = match settings.backend {
        StorageBackend::Local => {
            let configured = settings
                .local
                .root
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| data_root.join(DEFAULT_OBJECT_DIR));
            let root = if configured.is_absolute() {
                configured
            } else {
                data_root.join(configured)
            };
            std::fs::create_dir_all(&root)
                .with_context(|| format!("create local object root {}", root.display()))?;
            (
                Arc::new(
                    LocalFileSystem::new_with_prefix(&root)
                        .with_context(|| format!("open local object root {}", root.display()))?,
                ),
                String::new(),
            )
        }
        StorageBackend::S3 => {
            require_setting("S3 bucket", &settings.s3.bucket)?;
            require_setting("S3 region", &settings.s3.region)?;
            let mut builder = AmazonS3Builder::from_env()
                .with_bucket_name(settings.s3.bucket.trim())
                .with_region(settings.s3.region.trim())
                .with_virtual_hosted_style_request(!settings.s3.path_style);
            if let Some(endpoint) = clean_optional(settings.s3.endpoint.as_deref()) {
                builder = builder.with_endpoint(endpoint);
            }
            (
                Arc::new(builder.build().context("configure S3 object storage")?),
                normalize_prefix(&settings.s3.prefix)?,
            )
        }
        StorageBackend::AzureBlob => {
            require_setting("Azure storage account", &settings.azure_blob.account)?;
            require_setting("Azure Blob container", &settings.azure_blob.container)?;
            let mut builder = MicrosoftAzureBuilder::from_env()
                .with_account(settings.azure_blob.account.trim())
                .with_container_name(settings.azure_blob.container.trim());
            if let Some(endpoint) = clean_optional(settings.azure_blob.endpoint.as_deref()) {
                builder = builder.with_endpoint(endpoint.to_string());
            }
            (
                Arc::new(builder.build().context("configure Azure Blob storage")?),
                normalize_prefix(&settings.azure_blob.prefix)?,
            )
        }
    };
    Ok(ConfiguredStore {
        settings: settings.clone(),
        store,
        prefix,
    })
}

async fn put_to(store: &ConfiguredStore, key: &str, bytes: Vec<u8>) -> anyhow::Result<()> {
    let path = object_path(&store.prefix, key)?;
    if let Ok(metadata) = store.store.head(&path).await {
        if metadata.size == bytes.len() as u64 {
            return Ok(());
        }
    }
    store
        .store
        .put(&path, bytes.into())
        .await
        .context("put object")?;
    Ok(())
}

async fn read_from(store: &ConfiguredStore, key: &str) -> anyhow::Result<Vec<u8>> {
    let path = object_path(&store.prefix, key)?;
    Ok(store
        .store
        .get(&path)
        .await
        .context("get object")?
        .bytes()
        .await
        .context("read object body")?
        .to_vec())
}

async fn health_check_store(store: &ConfiguredStore) -> anyhow::Result<()> {
    let key = format!("health/{}.txt", uuid::Uuid::new_v4());
    let path = object_path(&store.prefix, &key)?;
    let body = b"evo-conductor-storage-health".to_vec();
    store.store.put(&path, body.clone().into()).await?;
    let observed = store.store.get(&path).await?.bytes().await?;
    if observed.as_ref() != body.as_slice() {
        return Err(anyhow!(
            "object storage health check returned different bytes"
        ));
    }
    store.store.delete(&path).await?;
    Ok(())
}

fn object_path(prefix: &str, key: &str) -> anyhow::Result<ObjectPath> {
    let combined = if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}/{key}")
    };
    ObjectPath::parse(combined).map_err(|error| anyhow!(error))
}

fn normalize_prefix(value: &str) -> anyhow::Result<String> {
    let value = value.trim().trim_matches('/');
    if value.is_empty() {
        return Ok(String::new());
    }
    if value
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || part.contains('\\'))
    {
        return Err(anyhow!("object prefix must be a safe relative path"));
    }
    Ok(value.to_string())
}

fn validate_content_key(key: &str) -> anyhow::Result<&str> {
    key.strip_prefix("sha256/")
        .and_then(|value| value.split_once('/'))
        .filter(|(prefix, digest)| {
            prefix.len() == 2
                && digest.len() == 64
                && digest.starts_with(prefix)
                && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .map(|(_, digest)| digest)
        .ok_or_else(|| anyhow!("invalid content-addressed object key"))
}

fn validate_object_digest(key: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let expected = validate_content_key(key)?;
    let observed = hex::encode(Sha256::digest(bytes));
    if observed != expected {
        return Err(anyhow!("object digest mismatch for {key}"));
    }
    Ok(())
}

fn require_setting(label: &str, value: &str) -> anyhow::Result<()> {
    if value.trim().is_empty() {
        Err(anyhow!("{label} is required"))
    } else {
        Ok(())
    }
}

fn clean_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn legacy_files(payload: &str) -> Vec<DraftFile> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|payload| payload.get("files").cloned())
        .and_then(|files| serde_json::from_value::<Vec<DraftFile>>(files).ok())
        .unwrap_or_default()
}

fn decode_data_logo(value: &str) -> Option<(String, Vec<u8>)> {
    let (metadata, encoded) = value.strip_prefix("data:")?.split_once(',')?;
    let (media_type, encoding) = metadata.split_once(';')?;
    if encoding != "base64" || !matches!(media_type, "image/png" | "image/jpeg" | "image/webp") {
        return None;
    }
    let bytes = STANDARD.decode(encoded).ok()?;
    if bytes.is_empty() || bytes.len() > 512 * 1024 {
        return None;
    }
    Some((media_type.to_string(), bytes))
}

fn deterministic_zip(files: &[DraftFile]) -> std::io::Result<Vec<u8>> {
    let mut files = files.to_vec();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    if files.len() > u16::MAX as usize {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "too many files",
        ));
    }
    let mut output = Vec::new();
    let mut central = Vec::new();
    for file in &files {
        let name = file.path.as_bytes();
        let body = file.content.as_bytes();
        let name_len = u16::try_from(name.len())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path too long"))?;
        let body_len = u32::try_from(body.len())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "file too large"))?;
        let offset = u32::try_from(output.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "archive too large")
        })?;
        let crc = crc32(body);

        push_u32(&mut output, 0x0403_4b50);
        push_u16(&mut output, 20);
        push_u16(&mut output, 0x0800);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u32(&mut output, crc);
        push_u32(&mut output, body_len);
        push_u32(&mut output, body_len);
        push_u16(&mut output, name_len);
        push_u16(&mut output, 0);
        output.extend_from_slice(name);
        output.extend_from_slice(body);

        push_u32(&mut central, 0x0201_4b50);
        push_u16(&mut central, 20);
        push_u16(&mut central, 20);
        push_u16(&mut central, 0x0800);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, crc);
        push_u32(&mut central, body_len);
        push_u32(&mut central, body_len);
        push_u16(&mut central, name_len);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, 0);
        push_u32(&mut central, offset);
        central.extend_from_slice(name);
    }
    let central_offset = u32::try_from(output.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "archive too large"))?;
    let central_size = u32::try_from(central.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "archive too large"))?;
    output.extend_from_slice(&central);
    push_u32(&mut output, 0x0605_4b50);
    push_u16(&mut output, 0);
    push_u16(&mut output, 0);
    push_u16(&mut output, files.len() as u16);
    push_u16(&mut output, files.len() as u16);
    push_u32(&mut output, central_size);
    push_u32(&mut output, central_offset);
    push_u16(&mut output, 0);
    Ok(output)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_is_deterministic_and_has_zip_markers() {
        let files = vec![
            DraftFile {
                path: "plugin.json".into(),
                content: "{}".into(),
            },
            DraftFile {
                path: "skills/demo/SKILL.md".into(),
                content: "demo".into(),
            },
        ];
        let first = deterministic_zip(&files).unwrap();
        let second = deterministic_zip(&files.into_iter().rev().collect::<Vec<_>>()).unwrap();
        assert_eq!(first, second);
        assert_eq!(&first[..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert!(first
            .windows(4)
            .any(|window| window == [0x50, 0x4b, 0x05, 0x06]));
    }

    #[tokio::test]
    async fn local_store_migrates_and_verifies_content_addressed_objects() {
        let first =
            std::env::temp_dir().join(format!("conductor-objects-a-{}", uuid::Uuid::new_v4()));
        let second =
            std::env::temp_dir().join(format!("conductor-objects-b-{}", uuid::Uuid::new_v4()));
        let store = ArtifactStore::new(first).await;
        let artifact = store.put(b"portable resource bytes").await.unwrap();
        let target = StorageSettings {
            local: conductor_domain::LocalStorageSettings {
                root: Some(second.to_string_lossy().into_owned()),
            },
            ..StorageSettings::default()
        };
        let stats = store
            .reconfigure(target.clone(), vec![artifact.key.clone()], || async {
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(stats.objects_copied, 1);
        assert_eq!(stats.bytes_copied, 23);
        assert_eq!(store.settings().await, target);
        assert_eq!(
            store.read(&artifact.key).await.unwrap(),
            b"portable resource bytes"
        );
    }
}
