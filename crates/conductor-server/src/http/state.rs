use std::sync::Arc;
use std::time::{Duration, Instant};

use conductor_auth::JwtService;
use conductor_storage::Db;
use dashmap::DashMap;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct PendingOidc {
    pub code_verifier: String,
    pub created_at: Instant,
}

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub jwt: Arc<RwLock<Option<JwtService>>>,
    pub oidc_pending: Arc<DashMap<String, PendingOidc>>,
}

impl AppState {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let db = Db::connect(database_url).await?;
        let jwt = Arc::new(RwLock::new(None));

        if let Some(secret) = db.instance().jwt_secret().await? {
            *jwt.write().await = Some(JwtService::new(secret, 72));
        }

        Ok(Self {
            db,
            jwt,
            oidc_pending: Arc::new(DashMap::new()),
        })
    }

    pub async fn set_jwt_secret(&self, secret: impl Into<String>) {
        *self.jwt.write().await = Some(JwtService::new(secret.into(), 72));
    }

    pub async fn jwt(&self) -> Option<JwtService> {
        self.jwt.read().await.clone()
    }

    pub fn store_oidc_pending(&self, state: String, code_verifier: String) {
        self.purge_oidc_pending();
        self.oidc_pending.insert(
            state,
            PendingOidc {
                code_verifier,
                created_at: Instant::now(),
            },
        );
    }

    pub fn take_oidc_pending(&self, state: &str) -> Option<String> {
        self.purge_oidc_pending();
        self.oidc_pending
            .remove(state)
            .map(|(_, v)| v.code_verifier)
    }

    fn purge_oidc_pending(&self) {
        let ttl = Duration::from_secs(600);
        self.oidc_pending
            .retain(|_, v| v.created_at.elapsed() < ttl);
    }
}
