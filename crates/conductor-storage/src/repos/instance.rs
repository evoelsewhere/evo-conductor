use chrono::Utc;
use conductor_domain::{
    InstanceConfig, PrimaryRole, RealtimeSettings, SetupRequest, SetupStatus, SsoConfig,
    SsoProvider, User, UserStatus,
};
use sqlx::Row;
use sqlx::{Any, Pool};
use uuid::Uuid;

use crate::core::mapping::parse_dt;

#[derive(Clone)]
pub struct InstanceRepo {
    pool: Pool<Any>,
}

pub struct SsoConfigUpdate<'a> {
    pub enabled: bool,
    pub provider: SsoProvider,
    pub issuer_url: Option<&'a str>,
    pub client_id: Option<&'a str>,
    pub client_secret: Option<&'a str>,
    pub redirect_uri: Option<&'a str>,
    pub scopes: Option<&'a [String]>,
}

/// Realtime limits stored on the instance row. `None` means the server falls
/// back to the environment configuration it was started with.
#[derive(Debug, Clone, Default)]
pub struct NetworkOverrides {
    pub realtime_max_connections: Option<u32>,
    pub realtime_max_per_secret: Option<u32>,
    pub realtime_heartbeat_seconds: Option<u32>,
}

impl InstanceRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn project_id(&self) -> Result<Option<Uuid>, sqlx::Error> {
        let value = sqlx::query_scalar::<_, String>("SELECT id FROM instance LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(value.and_then(|value| Uuid::parse_str(&value).ok()))
    }

    pub async fn setup_status(&self) -> Result<SetupStatus, sqlx::Error> {
        let row = sqlx::query(
            "SELECT project_name, display_name, logo_url, public_url, setup_completed FROM instance LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        let sso_enabled =
            sqlx::query_scalar::<_, i64>("SELECT enabled FROM sso_config WHERE id = 1")
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
                    display_name: r.get("display_name"),
                    logo_url: r.get("logo_url"),
                    public_url: r.get("public_url"),
                    sso_enabled,
                }
            }
            None => SetupStatus {
                configured: false,
                project_name: None,
                display_name: None,
                logo_url: None,
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
            &sso.and_then(|s| s.scopes.clone())
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
                id, email, display_name, password_hash, primary_role, status,
                must_change_password, created_at
            ) VALUES (?, ?, ?, ?, 'admin', 'active', 0, ?)
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
                logo_url: None,
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
                tag_ids: vec![],
                status: UserStatus::Active,
                must_change_password: false,
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
            SELECT id, project_name, display_name, bind_host, bind_port, public_url, logo_url,
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
            logo_url: r.get("logo_url"),
            setup_completed: r.get::<i64, _>("setup_completed") == 1,
            created_at: parse_dt(r.get("created_at")),
            updated_at: parse_dt(r.get("updated_at")),
        }))
    }

    pub async fn collection_level(&self) -> Result<String, sqlx::Error> {
        Ok(
            sqlx::query_scalar("SELECT collection_level FROM instance LIMIT 1")
                .fetch_optional(&self.pool)
                .await?
                .unwrap_or_else(|| "L1".to_string()),
        )
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
            scopes: serde_json::from_str(&scopes)
                .unwrap_or_else(|_| vec!["openid".into(), "profile".into(), "email".into()]),
        }))
    }

    pub async fn update_instance(
        &self,
        project_name: Option<&str>,
        display_name: Option<&str>,
        public_url: Option<&str>,
        logo_url: Option<&str>,
    ) -> Result<Option<InstanceConfig>, sqlx::Error> {
        let Some(current) = self.get().await? else {
            return Ok(None);
        };
        let now = Utc::now();
        let name = project_name.unwrap_or(&current.project_name);
        let display = display_name
            .map(|s| {
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s.trim().to_string())
                }
            })
            .unwrap_or_else(|| current.display_name.clone());
        let url = public_url
            .map(|s| {
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s.trim().to_string())
                }
            })
            .unwrap_or_else(|| current.public_url.clone());
        let logo = logo_url
            .map(|s| {
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s.trim().to_string())
                }
            })
            .unwrap_or_else(|| current.logo_url.clone());

        sqlx::query(
            r#"
            UPDATE instance SET project_name = ?, display_name = ?, public_url = ?, logo_url = ?, updated_at = ?
            "#,
        )
        .bind(name)
        .bind(&display)
        .bind(&url)
        .bind(&logo)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        self.get().await
    }

    pub async fn update_sso(&self, update: SsoConfigUpdate<'_>) -> Result<SsoConfig, sqlx::Error> {
        let SsoConfigUpdate {
            enabled,
            provider,
            issuer_url,
            client_id,
            client_secret,
            redirect_uri,
            scopes,
        } = update;
        let now = Utc::now();
        let existing = self.sso_config().await?;
        let scopes_json = serde_json::to_string(scopes.unwrap_or(existing.scopes.as_slice()))
            .unwrap_or_else(|_| "[]".into());

        let has_row = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sso_config WHERE id = 1")
            .fetch_one(&self.pool)
            .await?
            > 0;

        if has_row {
            if let Some(secret) = client_secret.filter(|s| !s.is_empty()) {
                sqlx::query(
                    r#"
                    UPDATE sso_config SET enabled = ?, provider = ?, issuer_url = ?,
                        client_id = ?, client_secret_enc = ?, redirect_uri = ?,
                        scopes = ?, updated_at = ?
                    WHERE id = 1
                    "#,
                )
                .bind(if enabled { 1 } else { 0 })
                .bind(provider.as_str())
                .bind(issuer_url)
                .bind(client_id)
                .bind(secret)
                .bind(redirect_uri)
                .bind(&scopes_json)
                .bind(now.to_rfc3339())
                .execute(&self.pool)
                .await?;
            } else {
                sqlx::query(
                    r#"
                    UPDATE sso_config SET enabled = ?, provider = ?, issuer_url = ?,
                        client_id = ?, redirect_uri = ?, scopes = ?, updated_at = ?
                    WHERE id = 1
                    "#,
                )
                .bind(if enabled { 1 } else { 0 })
                .bind(provider.as_str())
                .bind(issuer_url)
                .bind(client_id)
                .bind(redirect_uri)
                .bind(&scopes_json)
                .bind(now.to_rfc3339())
                .execute(&self.pool)
                .await?;
            }
        } else {
            sqlx::query(
                r#"
                INSERT INTO sso_config (
                    id, enabled, provider, issuer_url, client_id, client_secret_enc,
                    redirect_uri, scopes, updated_at
                ) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(if enabled { 1 } else { 0 })
            .bind(provider.as_str())
            .bind(issuer_url)
            .bind(client_id)
            .bind(client_secret)
            .bind(redirect_uri)
            .bind(&scopes_json)
            .bind(now.to_rfc3339())
            .execute(&self.pool)
            .await?;
        }

        self.sso_config().await
    }

    pub async fn network_overrides(&self) -> Result<NetworkOverrides, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT realtime_max_connections, realtime_max_per_secret, realtime_heartbeat_seconds
            FROM instance LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .map(|r| NetworkOverrides {
                realtime_max_connections: r
                    .get::<Option<i64>, _>("realtime_max_connections")
                    .map(|v| v as u32),
                realtime_max_per_secret: r
                    .get::<Option<i64>, _>("realtime_max_per_secret")
                    .map(|v| v as u32),
                realtime_heartbeat_seconds: r
                    .get::<Option<i64>, _>("realtime_heartbeat_seconds")
                    .map(|v| v as u32),
            })
            .unwrap_or_default())
    }

    pub async fn update_network(
        &self,
        bind_host: &str,
        bind_port: u16,
        public_url: Option<&str>,
        realtime: &RealtimeSettings,
    ) -> Result<(), sqlx::Error> {
        let public_url = public_url.and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        sqlx::query(
            r#"
            UPDATE instance SET bind_host = ?, bind_port = ?, public_url = ?,
                realtime_max_connections = ?, realtime_max_per_secret = ?,
                realtime_heartbeat_seconds = ?, updated_at = ?
            "#,
        )
        .bind(bind_host)
        .bind(i64::from(bind_port))
        .bind(public_url)
        .bind(i64::from(realtime.max_connections))
        .bind(i64::from(realtime.max_connections_per_secret))
        .bind(i64::from(realtime.heartbeat_seconds))
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
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
