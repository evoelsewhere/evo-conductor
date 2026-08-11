//! Project-selectable content-addressed object storage.
//!
//! Resource bytes never belong in the relational database. Drafts and
//! immutable releases share this store; the database keeps only object keys,
//! digests, sizes and portable file manifests.

use std::collections::BTreeSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::Arc;

use anyhow::{anyhow, Context};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use conductor_domain::{DraftFile, GitAuthMode, ResourceKind, StorageBackend, StorageSettings};
use conductor_storage::repos::LogoArtifact;
use conductor_storage::Db;
use object_store::aws::AmazonS3Builder;
use object_store::azure::MicrosoftAzureBuilder;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjectPath;
use object_store::ObjectStore;
use sha2::{Digest, Sha256};
use sqlx::Row;
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};

const DEFAULT_OBJECT_DIR: &str = "objects";
const GIT_STORAGE_DIR: &str = "git-storage";
const GIT_AUTHOR_NAME: &str = "Evo Conductor";
const GIT_AUTHOR_EMAIL: &str = "conductor@localhost";

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
    driver: StoreDriver,
    prefix: String,
}

#[derive(Clone)]
enum StoreDriver {
    Object(Arc<dyn ObjectStore>),
    Git(Arc<GitObjectStore>),
}

struct GitObjectStore {
    root: PathBuf,
    branch: String,
    remote_url: String,
    authorization_header: Option<String>,
    credential_update: Option<GitCredentialUpdate>,
    operation: Mutex<()>,
}

#[derive(Clone)]
struct GitCredentialUpdate {
    path: PathBuf,
    previous: Option<Vec<u8>>,
    desired: Option<Vec<u8>>,
}

struct GitCredentialRollback {
    path: PathBuf,
    previous: Option<Vec<u8>>,
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
        F: FnOnce(StorageSettings) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        // Pause normal reads and writes before building the candidate. A Git
        // credential-only update may reuse the same managed checkout path.
        let mut active = self.active.write().await;
        let candidate = build_store(&settings, &self.data_root).await?;
        health_check_store(&candidate).await?;

        let candidate_git = match &candidate.driver {
            StoreDriver::Git(store) => Some(store.clone()),
            StoreDriver::Object(_) => None,
        };
        let candidate_git_guard = if let Some(store) = &candidate_git {
            let guard = store.operation.lock().await;
            store.sync_from_remote_locked().await?;
            Some(guard)
        } else {
            None
        };
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
            let copied = if let Some(store) = &candidate_git {
                store
                    .write_file_locked(&candidate.prefix, &key, &bytes)
                    .await
                    .with_context(|| format!("copy object {key}"))?;
                store
                    .read_file_locked(&candidate.prefix, &key)
                    .await
                    .with_context(|| format!("verify copied object {key}"))?
            } else {
                put_to(&candidate, &key, bytes.clone())
                    .await
                    .with_context(|| format!("copy object {key}"))?;
                read_from(&candidate, &key)
                    .await
                    .with_context(|| format!("verify copied object {key}"))?
            };
            validate_object_digest(&key, &copied)?;
            stats.objects_copied = stats.objects_copied.saturating_add(1);
            stats.bytes_copied = stats
                .bytes_copied
                .saturating_add(bytes.len().try_into().unwrap_or(u64::MAX));
        }

        if let Some(store) = &candidate_git {
            store
                .commit_and_push_locked("chore(storage): migrate objects", false)
                .await?;
        }
        drop(candidate_git_guard);

        let credential_rollback = apply_git_credential_update(&candidate).await?;
        if let Err(error) = persist(candidate.settings.clone()).await {
            if let Some(rollback) = credential_rollback {
                rollback_git_credential(rollback).await?;
            }
            return Err(error);
        }
        *active = candidate;
        Ok(stats)
    }
}

async fn build_store(
    settings: &StorageSettings,
    data_root: &Path,
) -> anyhow::Result<ConfiguredStore> {
    let mut effective = settings.clone();
    let (driver, prefix) = match settings.backend {
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
                StoreDriver::Object(Arc::new(
                    LocalFileSystem::new_with_prefix(&root)
                        .with_context(|| format!("open local object root {}", root.display()))?,
                )),
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
                StoreDriver::Object(Arc::new(
                    builder.build().context("configure S3 object storage")?,
                )),
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
                StoreDriver::Object(Arc::new(
                    builder.build().context("configure Azure Blob storage")?,
                )),
                normalize_prefix(&settings.azure_blob.prefix)?,
            )
        }
        StorageBackend::Git => {
            let store = build_git_store(&mut effective, data_root).await?;
            (
                StoreDriver::Git(Arc::new(store)),
                normalize_prefix(&settings.git.prefix)?,
            )
        }
    };
    Ok(ConfiguredStore {
        settings: effective,
        driver,
        prefix,
    })
}

async fn build_git_store(
    settings: &mut StorageSettings,
    data_root: &Path,
) -> anyhow::Result<GitObjectStore> {
    let repository_url = validate_git_repository_url(&settings.git.repository_url)?;
    let branch = validate_git_branch(&settings.git.branch)?;
    let prefix = normalize_prefix(&settings.git.prefix)?;
    if prefix
        .split('/')
        .any(|component| component.eq_ignore_ascii_case(".git"))
    {
        return Err(anyhow!("Git object prefix must not enter .git"));
    }
    let username = clean_optional(settings.git.username.as_deref()).map(str::to_string);
    if username.as_deref().is_some_and(|value| {
        value.len() > 256
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte == b':')
    }) {
        return Err(anyhow!("Git username is invalid"));
    }

    let git_root = data_root.join(GIT_STORAGE_DIR);
    let credential_id = hex::encode(Sha256::digest(repository_url.as_bytes()));
    let credential_path = git_root
        .join("credentials")
        .join(format!("{credential_id}.token"));
    let previous_credential = read_secret_file(&credential_path).await?;
    let requested_credential = settings
        .git
        .credential
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(credential) = requested_credential {
        validate_git_credential(credential)?;
    }
    let desired_credential = if settings.git.clear_credential {
        None
    } else if let Some(credential) = requested_credential {
        Some(credential.as_bytes().to_vec())
    } else {
        previous_credential.clone()
    };
    let credential = desired_credential
        .as_deref()
        .map(|bytes| String::from_utf8(bytes.to_vec()))
        .transpose()
        .context("Git credential must be UTF-8")?;
    let authorization_header = match settings.git.auth_mode {
        GitAuthMode::Environment => None,
        GitAuthMode::HttpsToken => {
            if !repository_url.starts_with("https://") {
                return Err(anyhow!(
                    "Git HTTPS token authentication requires an https:// repository URL"
                ));
            }
            let credential = credential
                .as_deref()
                .ok_or_else(|| anyhow!("Git access token is required"))?;
            let username = username.as_deref().unwrap_or("git");
            Some(format!(
                "Authorization: Basic {}",
                STANDARD.encode(format!("{username}:{credential}"))
            ))
        }
    };

    settings.git.repository_url = repository_url.clone();
    settings.git.branch = branch.clone();
    settings.git.prefix = prefix.clone();
    settings.git.username = username.clone();
    settings.git.credential = None;
    settings.git.clear_credential = false;
    settings.git.credential_set = credential.is_some();

    let checkout_identity = format!("{repository_url}\0{branch}\0{prefix}");
    let checkout_id = hex::encode(Sha256::digest(checkout_identity.as_bytes()));
    let store = GitObjectStore {
        root: git_root.join("checkouts").join(checkout_id),
        branch,
        remote_url: repository_url,
        authorization_header,
        credential_update: (previous_credential != desired_credential).then_some(
            GitCredentialUpdate {
                path: credential_path,
                previous: previous_credential,
                desired: desired_credential,
            },
        ),
        operation: Mutex::new(()),
    };
    store.initialize().await?;
    Ok(store)
}

impl GitObjectStore {
    async fn initialize(&self) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(&self.root)
            .await
            .with_context(|| format!("create Git checkout {}", self.root.display()))?;
        if !self.root.join(".git").is_dir() {
            self.run_checked(&["init", "-b", &self.branch, "."])
                .await
                .context("initialize Git storage checkout")?;
            self.run_checked(&["remote", "add", "origin", &self.remote_url])
                .await
                .context("configure Git storage remote")?;
        } else {
            let observed = self
                .run_checked(&["remote", "get-url", "origin"])
                .await
                .context("read Git storage remote")?;
            if String::from_utf8_lossy(&observed.stdout).trim() != self.remote_url {
                return Err(anyhow!(
                    "managed Git checkout remote does not match project settings"
                ));
            }
        }
        self.run_checked(&["config", "user.name", GIT_AUTHOR_NAME])
            .await?;
        self.run_checked(&["config", "user.email", GIT_AUTHOR_EMAIL])
            .await?;

        let _guard = self.operation.lock().await;
        self.recover_managed_checkout_locked().await?;
        self.sync_from_remote_locked().await
    }

    async fn put(&self, prefix: &str, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let _guard = self.operation.lock().await;
        self.sync_from_remote_locked().await?;
        self.write_file_locked(prefix, key, bytes).await?;
        self.commit_and_push_locked(&format!("chore(storage): put {key}"), false)
            .await
    }

    async fn read(&self, prefix: &str, key: &str) -> anyhow::Result<Vec<u8>> {
        let _guard = self.operation.lock().await;
        match self.read_file_locked(prefix, key).await {
            Ok(bytes) => Ok(bytes),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
            {
                self.sync_from_remote_locked().await?;
                self.read_file_locked(prefix, key).await
            }
            Err(error) => Err(error),
        }
    }

    async fn health_check(&self, prefix: &str) -> anyhow::Result<()> {
        let _guard = self.operation.lock().await;
        self.sync_from_remote_locked().await?;
        let key = format!("health/{}.txt", uuid::Uuid::new_v4());
        let body = b"evo-conductor-storage-health";
        self.write_file_locked(prefix, &key, body).await?;
        let observed = self.read_file_locked(prefix, &key).await?;
        if observed != body {
            return Err(anyhow!("Git storage health check returned different bytes"));
        }
        let path = safe_git_file_path(&self.root, prefix, &key)?;
        tokio::fs::remove_file(path)
            .await
            .context("remove Git storage health object")?;
        self.commit_and_push_locked("chore(storage): verify Git access", true)
            .await
    }

    async fn write_file_locked(&self, prefix: &str, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let path = safe_git_file_path(&self.root, prefix, key)?;
        if let Ok(existing) = tokio::fs::read(&path).await {
            if existing == bytes {
                return Ok(());
            }
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("create Git object directory")?;
        }
        let temporary = path.with_extension(format!("conductor-{}.tmp", uuid::Uuid::new_v4()));
        tokio::fs::write(&temporary, bytes)
            .await
            .context("write temporary Git object")?;
        tokio::fs::rename(&temporary, &path)
            .await
            .context("activate Git object")?;
        Ok(())
    }

    async fn read_file_locked(&self, prefix: &str, key: &str) -> anyhow::Result<Vec<u8>> {
        let path = safe_git_file_path(&self.root, prefix, key)?;
        tokio::fs::read(path).await.context("read Git object")
    }

    async fn recover_managed_checkout_locked(&self) -> anyhow::Result<()> {
        let status = self.run_checked(&["status", "--porcelain"]).await?;
        if status.stdout.is_empty() {
            return Ok(());
        }
        if self.head_exists_locked().await? {
            self.run_checked(&["reset", "--hard", "HEAD"]).await?;
        }
        self.run_checked(&["clean", "-fd"]).await?;
        Ok(())
    }

    async fn sync_from_remote_locked(&self) -> anyhow::Result<()> {
        if !self.remote_branch_exists_locked().await? {
            return Ok(());
        }
        self.run_checked(&["fetch", "origin", &self.branch])
            .await
            .context("fetch Git storage branch")?;
        let remote_ref = format!("origin/{}", self.branch);
        if self.head_exists_locked().await? {
            self.run_checked(&["checkout", &self.branch]).await?;
            self.run_checked(&["rebase", &remote_ref])
                .await
                .context("fast-forward Git storage mirror")?;
        } else {
            self.run_checked(&["checkout", "-B", &self.branch, &remote_ref])
                .await
                .context("checkout Git storage branch")?;
        }
        Ok(())
    }

    async fn commit_and_push_locked(&self, message: &str, allow_empty: bool) -> anyhow::Result<()> {
        self.run_checked(&["add", "--all"]).await?;
        let diff = self.run_output(&["diff", "--cached", "--quiet"]).await?;
        let has_changes = match diff.status.code() {
            Some(0) => false,
            Some(1) => true,
            _ => return Err(self.command_error("inspect staged Git storage changes", &diff)),
        };
        if !has_changes && !allow_empty {
            return Ok(());
        }
        if allow_empty {
            self.run_checked(&["commit", "--allow-empty", "-m", message])
                .await?;
        } else {
            self.run_checked(&["commit", "-m", message]).await?;
        }

        let destination = format!("HEAD:refs/heads/{}", self.branch);
        for attempt in 0..3 {
            let pushed = self.run_output(&["push", "origin", &destination]).await?;
            if pushed.status.success() {
                return Ok(());
            }
            if attempt == 2 {
                return Err(self.command_error("push Git storage commit", &pushed));
            }
            self.run_checked(&["fetch", "origin", &self.branch]).await?;
            let remote_ref = format!("origin/{}", self.branch);
            self.run_checked(&["rebase", &remote_ref])
                .await
                .context("rebase concurrent Git storage commit")?;
        }
        unreachable!()
    }

    async fn remote_branch_exists_locked(&self) -> anyhow::Result<bool> {
        let reference = format!("refs/heads/{}", self.branch);
        let output = self
            .run_output(&["ls-remote", "--exit-code", "--heads", "origin", &reference])
            .await?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(2) => Ok(false),
            _ => Err(self.command_error("inspect Git storage branch", &output)),
        }
    }

    async fn head_exists_locked(&self) -> anyhow::Result<bool> {
        let output = self.run_output(&["rev-parse", "--verify", "HEAD"]).await?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(128) => Ok(false),
            _ => Err(self.command_error("inspect Git storage HEAD", &output)),
        }
    }

    async fn run_checked(&self, args: &[&str]) -> anyhow::Result<Output> {
        let output = self.run_output(args).await?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(self.command_error("run Git storage command", &output))
        }
    }

    async fn run_output(&self, args: &[&str]) -> anyhow::Result<Output> {
        let mut command = Command::new("git");
        command
            .current_dir(&self.root)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(header) = &self.authorization_header {
            command
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "http.extraHeader")
                .env("GIT_CONFIG_VALUE_0", header);
        }
        command.output().await.context("start Git command")
    }

    fn command_error(&self, action: &str, output: &Output) -> anyhow::Error {
        let stderr = String::from_utf8_lossy(&output.stderr)
            .replace(&self.remote_url, "<remote>")
            .trim()
            .to_string();
        if stderr.is_empty() {
            anyhow!("{action} failed with {}", output.status)
        } else {
            anyhow!("{action} failed: {stderr}")
        }
    }
}

async fn write_secret_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("create Git credential directory")?;
    }
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(anyhow!("Git credential path must not be a symlink"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("inspect Git credential path"),
    }
    let temporary = path.with_extension(format!("conductor-{}.tmp", uuid::Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes)
        .await
        .context("write Git credential")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .await
            .context("protect Git credential")?;
    }
    tokio::fs::rename(&temporary, path)
        .await
        .context("activate Git credential")?;
    Ok(())
}

async fn read_secret_file(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(anyhow!("Git credential path must not be a symlink"))
        }
        Ok(_) => tokio::fs::read(path)
            .await
            .map(Some)
            .context("read Git credential"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("inspect Git credential path"),
    }
}

async fn apply_git_credential_update(
    store: &ConfiguredStore,
) -> anyhow::Result<Option<GitCredentialRollback>> {
    let StoreDriver::Git(git) = &store.driver else {
        return Ok(None);
    };
    let Some(update) = &git.credential_update else {
        return Ok(None);
    };
    match &update.desired {
        Some(secret) => write_secret_file(&update.path, secret).await?,
        None => match tokio::fs::remove_file(&update.path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("remove Git credential"),
        },
    }
    Ok(Some(GitCredentialRollback {
        path: update.path.clone(),
        previous: update.previous.clone(),
    }))
}

async fn rollback_git_credential(rollback: GitCredentialRollback) -> anyhow::Result<()> {
    match rollback.previous {
        Some(secret) => write_secret_file(&rollback.path, &secret).await,
        None => match tokio::fs::remove_file(&rollback.path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("rollback Git credential"),
        },
    }
}

fn validate_git_repository_url(value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    require_setting("Git repository URL", value)?;
    if value.len() > 2_048 || value.chars().any(char::is_control) {
        return Err(anyhow!("Git repository URL is invalid"));
    }
    if value.starts_with("https://") || value.starts_with("ssh://") || value.starts_with("file://")
    {
        let parsed = url::Url::parse(value).context("parse Git repository URL")?;
        let embeds_credential = parsed.password().is_some()
            || (parsed.scheme() == "https" && !parsed.username().is_empty());
        if embeds_credential {
            return Err(anyhow!(
                "Git repository URL must not contain embedded credentials"
            ));
        }
        return Ok(value.to_string());
    }
    if Path::new(value).is_absolute()
        || (value.contains(':') && !value.contains("://") && !value.contains(char::is_whitespace))
    {
        return Ok(value.to_string());
    }
    Err(anyhow!(
        "Git repository URL must use https://, ssh://, file://, an absolute path, or SCP syntax"
    ))
}

fn validate_git_branch(value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= 128
        && !value.starts_with(['.', '/', '-'])
        && !value.ends_with(['.', '/'])
        && !value.ends_with(".lock")
        && !value.contains("..")
        && !value.contains("//")
        && !value.contains("@{")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'));
    if valid {
        Ok(value.to_string())
    } else {
        Err(anyhow!("Git branch must be a safe branch name"))
    }
}

fn validate_git_credential(value: &str) -> anyhow::Result<()> {
    if value.is_empty()
        || value.len() > 4_096
        || value
            .bytes()
            .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
    {
        Err(anyhow!("Git credential is invalid"))
    } else {
        Ok(())
    }
}

fn safe_git_file_path(root: &Path, prefix: &str, key: &str) -> anyhow::Result<PathBuf> {
    let relative = object_path(prefix, key)?.to_string();
    let mut current = root.to_path_buf();
    for component in Path::new(&relative).components() {
        let std::path::Component::Normal(component) = component else {
            return Err(anyhow!("Git object path is not safe"));
        };
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(anyhow!("Git object path traverses a symlink"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("inspect Git object path"),
        }
    }
    Ok(current)
}

async fn put_to(store: &ConfiguredStore, key: &str, bytes: Vec<u8>) -> anyhow::Result<()> {
    match &store.driver {
        StoreDriver::Object(driver) => {
            let path = object_path(&store.prefix, key)?;
            if let Ok(metadata) = driver.head(&path).await {
                if metadata.size == bytes.len() as u64 {
                    return Ok(());
                }
            }
            driver
                .put(&path, bytes.into())
                .await
                .context("put object")?;
            Ok(())
        }
        StoreDriver::Git(driver) => driver.put(&store.prefix, key, &bytes).await,
    }
}

async fn read_from(store: &ConfiguredStore, key: &str) -> anyhow::Result<Vec<u8>> {
    match &store.driver {
        StoreDriver::Object(driver) => {
            let path = object_path(&store.prefix, key)?;
            Ok(driver
                .get(&path)
                .await
                .context("get object")?
                .bytes()
                .await
                .context("read object body")?
                .to_vec())
        }
        StoreDriver::Git(driver) => driver.read(&store.prefix, key).await,
    }
}

async fn health_check_store(store: &ConfiguredStore) -> anyhow::Result<()> {
    match &store.driver {
        StoreDriver::Object(driver) => {
            let key = format!("health/{}.txt", uuid::Uuid::new_v4());
            let path = object_path(&store.prefix, &key)?;
            let body = b"evo-conductor-storage-health".to_vec();
            driver.put(&path, body.clone().into()).await?;
            let observed = driver.get(&path).await?.bytes().await?;
            if observed.as_ref() != body.as_slice() {
                return Err(anyhow!(
                    "object storage health check returned different bytes"
                ));
            }
            driver.delete(&path).await?;
            Ok(())
        }
        StoreDriver::Git(driver) => driver.health_check(&store.prefix).await,
    }
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
    use std::process::Command as StdCommand;

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

    #[test]
    fn git_settings_reject_embedded_credentials_and_unsafe_refs() {
        assert!(validate_git_repository_url(
            "https://user:secret@git.example.test/acme/resources.git"
        )
        .is_err());
        assert!(validate_git_repository_url("https://git.example.test/acme/resources.git").is_ok());
        assert!(
            validate_git_repository_url("ssh://git@git.example.test/acme/resources.git").is_ok()
        );
        assert!(validate_git_branch("release/resources").is_ok());
        assert!(validate_git_branch("../main").is_err());
        assert!(validate_git_branch("-main").is_err());
        assert!(validate_git_branch("main.lock").is_err());
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
            .reconfigure(target.clone(), vec![artifact.key.clone()], |_| async {
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

    #[tokio::test]
    async fn git_store_pushes_content_addressed_objects_to_the_configured_branch() {
        let root =
            std::env::temp_dir().join(format!("conductor-git-storage-{}", uuid::Uuid::new_v4()));
        let remote = root.join("remote.git");
        std::fs::create_dir_all(&root).unwrap();
        let initialized = StdCommand::new("git")
            .args(["init", "--bare", remote.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(initialized.status.success());

        let settings = StorageSettings {
            backend: StorageBackend::Git,
            git: conductor_domain::GitStorageSettings {
                repository_url: remote.to_string_lossy().into_owned(),
                branch: "resources".into(),
                prefix: "evo-conductor".into(),
                ..conductor_domain::GitStorageSettings::default()
            },
            ..StorageSettings::default()
        };
        let configured = build_store(&settings, &root.join("data")).await.unwrap();
        health_check_store(&configured).await.unwrap();
        let body = b"git-backed portable resource".to_vec();
        let digest = hex::encode(Sha256::digest(&body));
        let key = format!("sha256/{}/{digest}", &digest[..2]);
        put_to(&configured, &key, body.clone()).await.unwrap();
        assert_eq!(read_from(&configured, &key).await.unwrap(), body);

        let verify = root.join("verify");
        let cloned = StdCommand::new("git")
            .args([
                "clone",
                "--branch",
                "resources",
                remote.to_str().unwrap(),
                verify.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            cloned.status.success(),
            "{}",
            String::from_utf8_lossy(&cloned.stderr)
        );
        assert_eq!(
            std::fs::read(verify.join("evo-conductor").join(key)).unwrap(),
            b"git-backed portable resource"
        );
    }

    #[tokio::test]
    async fn failed_git_settings_persistence_rolls_back_the_write_only_credential() {
        let root =
            std::env::temp_dir().join(format!("conductor-git-credential-{}", uuid::Uuid::new_v4()));
        let remote = root.join("remote.git");
        std::fs::create_dir_all(&root).unwrap();
        assert!(StdCommand::new("git")
            .args(["init", "--bare", remote.to_str().unwrap()])
            .status()
            .unwrap()
            .success());
        let data_root = root.join("data");
        let local_settings = StorageSettings {
            local: conductor_domain::LocalStorageSettings {
                root: Some(root.join("objects").to_string_lossy().into_owned()),
            },
            ..StorageSettings::default()
        };
        let active = build_store(&local_settings, &data_root).await.unwrap();
        let store = ArtifactStore {
            active: Arc::new(RwLock::new(active)),
            data_root: data_root.clone(),
        };
        let target = StorageSettings {
            backend: StorageBackend::Git,
            git: conductor_domain::GitStorageSettings {
                repository_url: remote.to_string_lossy().into_owned(),
                branch: "main".into(),
                credential: Some("write-only-test-token".into()),
                ..conductor_domain::GitStorageSettings::default()
            },
            ..StorageSettings::default()
        };
        let result = store
            .reconfigure(target, Vec::new(), |_| async {
                Err(anyhow!("simulated database failure"))
            })
            .await;
        assert!(result.is_err());
        assert_eq!(store.settings().await.backend, StorageBackend::Local);
        let credential_id = hex::encode(Sha256::digest(remote.to_string_lossy().as_bytes()));
        assert!(!data_root
            .join(GIT_STORAGE_DIR)
            .join("credentials")
            .join(format!("{credential_id}.token"))
            .exists());
    }
}
