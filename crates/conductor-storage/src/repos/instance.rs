use chrono::Utc;
use conductor_domain::{
    InstanceConfig, SetupRequest, SetupStatus, SsoConfig, SsoProvider, User, UserStatus,
    PrimaryRole,
};
use sqlx::{Any, Pool};
use sqlx::Row;
use uuid::Uuid;

use crate::mapping::parse_dt;

#[derive(Clone)]
pub struct InstanceRepo {
    pool: Pool<Any>,
}

impl InstanceRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn setup_status(&self) -> Result<SetupStatus, sqlx::Error> {
        let row = sqlx::query(
            "SELECT project_name, public_url, setup_completed FROM instance LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        let sso_enabled = sqlx::query_scalar::<_, i64>("SELECT enabled FROM sso_config WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(0)
            == 1;

        Ok(match row {
            Some(r) => {
                let completed: i64 = r.get("setup_completed");
                SetupStatus {
                    configured: completed == 1,
                    project_name: Some(r.get("project_name")),
                    public_url: r.get("public_url"),
                    sso_enabled,
                }
            }
            None => SetupStatus {
                configured: false,
                project_name: None,
                public_url: None,
                sso_enabled: false,
            },
        })
    }

    pub async fn is_setup_completed(&self) -> Result<bool, sqlx::Error> {
        Ok(self.setup_status().await?.configured)
    }

    pub async fn complete_setup(
        &self,
        req: &SetupRequest,
        admin_password_hash: &str,
        jwt_secret: &str,
        client_secret_enc: Option<&str>,
    ) -> Result<(InstanceConfig, User), sqlx::Error> {
        let now = Utc::now();
        let instance_id = Uuid::new_v4();
        let admin_id = Uuid::new_v4();

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO instance (
                id, project_name, display_name, bind_host, bind_port, public_url,
                setup_completed, jwt_secret, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?)
            "#,
        )
        .bind(instance_id.to_string())
        .bind(&req.project_name)
        .bind(&req.display_name)
        .bind(&req.bind_host)
        .bind(req.bind_port as i64)
        .bind(&req.public_url)
        .bind(jwt_secret)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        let sso = req.sso.as_ref();
        let enabled = sso.map(|s| s.enabled).unwrap_or(false);
        let provider = sso
            .map(|s| s.provider.as_str().to_string())
            .unwrap_or_else(|| "oidc".into());
        let scopes = serde_json::to_string(
            &sso
                .and_then(|s| s.scopes.clone())
                .unwrap_or_else(|| vec!["openid".into(), "profile".into(), "email".into()]),
        )
        .unwrap_or_else(|_| "[]".into());

        sqlx::query(
            r#"
            INSERT INTO sso_config (
                id, enabled, provider, issuer_url, client_id, client_secret_enc,
                redirect_uri, scopes, updated_at
            ) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(if enabled { 1 } else { 0 })
        .bind(provider)
        .bind(sso.and_then(|s| s.issuer_url.clone()))
        .bind(sso.and_then(|s| s.client_id.clone()))
        .bind(client_secret_enc)
        .bind(sso.and_then(|s| s.redirect_uri.clone()))
        .bind(scopes)
        .bind(now.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, display_name, password_hash, primary_role, status, created_at
            ) VALUES (?, ?, ?, ?, 'admin', 'active', ?)
            "#,
        )
        .bind(admin_id.to_string())
        .bind(req.admin_email.to_lowercase())
        .bind(&req.admin_display_name)
        .bind(admin_password_hash)
        .bind(now.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        for (slug, name, color) in [
            ("dev", "Developer", "#60A5FA"),
            ("ba", "Business Analyst", "#A78BFA"),
            ("tester", "Tester", "#4ADE80"),
        ] {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO sub_roles (id, slug, name, description, color, created_at)
                VALUES (?, ?, ?, NULL, ?, ?)
                "#,
            )
            .bind(id.to_string())
            .bind(slug)
            .bind(name)
            .bind(color)
            .bind(now.to_rfc3339())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok((
            InstanceConfig {
                id: instance_id,
                project_name: req.project_name.clone(),
                display_name: req.display_name.clone(),
                bind_host: req.bind_host.clone(),
                bind_port: req.bind_port,
                public_url: req.public_url.clone(),
                setup_completed: true,
                created_at: now,
                updated_at: now,
            },
            User {
                id: admin_id,
                email: req.admin_email.to_lowercase(),
                display_name: req.admin_display_name.clone(),
                primary_role: PrimaryRole::Admin,
                sub_role_ids: vec![],
                status: UserStatus::Active,
                last_seen_at: None,
                created_at: now,
            },
        ))
    }

    pub async fn jwt_secret(&self) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT jwt_secret FROM instance LIMIT 1")
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn get(&self) -> Result<Option<InstanceConfig>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, project_name, display_name, bind_host, bind_port, public_url,
                   setup_completed, created_at, updated_at
            FROM instance LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| InstanceConfig {
            id: Uuid::parse_str(r.get::<String, _>("id").as_str()).unwrap_or_else(|_| Uuid::nil()),
            project_name: r.get("project_name"),
            display_name: r.get("display_name"),
            bind_host: r.get("bind_host"),
            bind_port: r.get::<i64, _>("bind_port") as u16,
            public_url: r.get("public_url"),
            setup_completed: r.get::<i64, _>("setup_completed") == 1,
            created_at: parse_dt(r.get("created_at")),
            updated_at: parse_dt(r.get("updated_at")),
        }))
    }

    pub async fn sso_config(&self) -> Result<SsoConfig, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT enabled, provider, issuer_url, client_id, client_secret_enc,
                   redirect_uri, scopes
            FROM sso_config WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(match row {
            Some(r) => {
                let secret: Option<String> = r.get("client_secret_enc");
                let scopes: String = r.get("scopes");
                SsoConfig {
                    enabled: r.get::<i64, _>("enabled") == 1,
                    provider: SsoProvider::parse(r.get::<String, _>("provider").as_str()),
                    issuer_url: r.get("issuer_url"),
                    client_id: r.get("client_id"),
                    client_secret_set: Some(secret.as_ref().is_some_and(|s| !s.is_empty())),
                    redirect_uri: r.get("redirect_uri"),
                    scopes: serde_json::from_str(&scopes).unwrap_or_default(),
                }
            }
            None => SsoConfig {
                enabled: false,
                provider: SsoProvider::Oidc,
                issuer_url: None,
                client_id: None,
                client_secret_set: Some(false),
                redirect_uri: None,
                scopes: vec!["openid".into(), "profile".into(), "email".into()],
            },
        })
    }

    /// Internal SSO material for token exchange (includes client secret).
    pub async fn sso_runtime(&self) -> Result<Option<SsoRuntime>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT enabled, provider, issuer_url, client_id, client_secret_enc,
                   redirect_uri, scopes
            FROM sso_config WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(r) = row else {
            return Ok(None);
        };
        if r.get::<i64, _>("enabled") != 1 {
            return Ok(None);
        }

        let issuer_url: Option<String> = r.get("issuer_url");
        let client_id: Option<String> = r.get("client_id");
        let client_secret: Option<String> = r.get("client_secret_enc");
        let redirect_uri: Option<String> = r.get("redirect_uri");
        let scopes: String = r.get("scopes");

        let (Some(issuer_url), Some(client_id), Some(client_secret), Some(redirect_uri)) =
            (issuer_url, client_id, client_secret, redirect_uri)
        else {
            return Ok(None);
        };

        if issuer_url.is_empty()
            || client_id.is_empty()
            || client_secret.is_empty()
            || redirect_uri.is_empty()
        {
            return Ok(None);
        }

        Ok(Some(SsoRuntime {
            provider: SsoProvider::parse(r.get::<String, _>("provider").as_str()),
            issuer_url,
            client_id,
            client_secret,
            redirect_uri,
            scopes: serde_json::from_str(&scopes).unwrap_or_else(|_| {
                vec!["openid".into(), "profile".into(), "email".into()]
            }),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct SsoRuntime {
    pub provider: SsoProvider,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}
