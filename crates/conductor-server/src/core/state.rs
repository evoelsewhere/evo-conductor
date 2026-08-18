use std::sync::Arc;
use std::time::{Duration, Instant};

use conductor_auth::JwtService;
use conductor_domain::core::constants::auth::DEFAULT_JWT_TTL_HOURS;

use crate::core::constants::oidc::PENDING_TTL_SECS;
use conductor_storage::Db;
use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::core::artifacts::ArtifactStore;
use crate::core::authorization::AuthorizationService;
use crate::core::config::RealtimeConfig;
use crate::core::host_metrics::{HostMetricsProvider, SystemHostMetricsProvider};
use crate::http::realtime::RealtimeHub;

#[derive(Clone)]
pub struct PendingOidc {
    pub code_verifier: String,
    pub nonce: String,
    pub created_at: Instant,
}

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub jwt: Arc<RwLock<Option<JwtService>>>,
    pub oidc_pending: Arc<DashMap<String, PendingOidc>>,
    pub realtime: RealtimeHub,
    pub artifacts: ArtifactStore,
    pub authorization: AuthorizationService,
    pub host_metrics: Arc<dyn HostMetricsProvider>,
}

impl AppState {
    pub async fn new(database_url: &str, realtime_config: RealtimeConfig) -> anyhow::Result<Self> {
        let (db, jwt, realtime_config) = Self::connect(database_url, realtime_config).await?;
        let storage_settings = db.instance().storage_settings().await?;
        let artifacts = ArtifactStore::from_settings(storage_settings).await?;
        Self::finish_initialization(db, jwt, realtime_config, artifacts).await
    }

    /// Construct state with an already configured artifact store.
    ///
    /// This is an integration seam for callers that own the object-store
    /// lifecycle, including isolated HTTP fixtures. Normal server startup
    /// should use [`Self::new`] so project storage settings remain authoritative.
    #[doc(hidden)]
    pub async fn new_with_artifact_store(
        database_url: &str,
        realtime_config: RealtimeConfig,
        artifacts: ArtifactStore,
    ) -> anyhow::Result<Self> {
        let (db, jwt, realtime_config) = Self::connect(database_url, realtime_config).await?;
        Self::finish_initialization(db, jwt, realtime_config, artifacts).await
    }

    async fn connect(
        database_url: &str,
        realtime_config: RealtimeConfig,
    ) -> anyhow::Result<(Db, Arc<RwLock<Option<JwtService>>>, RealtimeConfig)> {
        let db = Db::connect(database_url).await?;
        let jwt = Arc::new(RwLock::new(None));

        if let Some(secret) = db.instance().jwt_secret().await? {
            *jwt.write().await = Some(JwtService::new(secret, DEFAULT_JWT_TTL_HOURS));
        }

        // Limits saved via the network settings override the environment config.
        let overrides = db.instance().network_overrides().await?;
        let realtime_config = realtime_config.with_overrides(&overrides);
        Ok((db, jwt, realtime_config))
    }

    async fn finish_initialization(
        db: Db,
        jwt: Arc<RwLock<Option<JwtService>>>,
        realtime_config: RealtimeConfig,
        artifacts: ArtifactStore,
    ) -> anyhow::Result<Self> {
        let migrated_payloads = artifacts.externalize_legacy_payloads(&db).await?;
        if migrated_payloads > 0 {
            tracing::info!(
                migrated_payloads,
                "externalized legacy resource file payloads"
            );
        }

        Ok(Self {
            db,
            jwt,
            oidc_pending: Arc::new(DashMap::new()),
            realtime: RealtimeHub::new(realtime_config),
            artifacts,
            authorization: AuthorizationService::default(),
            host_metrics: Arc::new(SystemHostMetricsProvider::default()),
        })
    }

    /// Replace the Conductor-host sampler before building the router.
    /// Production uses `SystemHostMetricsProvider`; deterministic HTTP tests
    /// inject a fixed implementation through this seam.
    #[doc(hidden)]
    pub fn set_host_metrics_provider(&mut self, provider: Arc<dyn HostMetricsProvider>) {
        self.host_metrics = provider;
    }

    pub async fn set_jwt_secret(&self, secret: impl Into<String>) {
        *self.jwt.write().await = Some(JwtService::new(secret.into(), DEFAULT_JWT_TTL_HOURS));
    }

    pub async fn jwt(&self) -> Option<JwtService> {
        self.jwt.read().await.clone()
    }

    pub fn store_oidc_pending(&self, state: String, code_verifier: String, nonce: String) {
        self.purge_oidc_pending();
        self.oidc_pending.insert(
            state,
            PendingOidc {
                code_verifier,
                nonce,
                created_at: Instant::now(),
            },
        );
    }

    pub fn take_oidc_pending(&self, state: &str) -> Option<(String, String)> {
        self.purge_oidc_pending();
        self.oidc_pending
            .remove(state)
            .map(|(_, v)| (v.code_verifier, v.nonce))
    }

    fn purge_oidc_pending(&self) {
        let ttl = Duration::from_secs(PENDING_TTL_SECS);
        self.oidc_pending
            .retain(|_, v| v.created_at.elapsed() < ttl);
    }
}
