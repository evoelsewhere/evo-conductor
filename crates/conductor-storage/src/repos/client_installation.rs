use chrono::{TimeDelta, Utc};
use conductor_domain::{ClientInstallation, ClientPlatform, RegisterClientRequest};
use sqlx::{Any, Pool, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::core::mapping::parse_dt;

#[derive(Debug, Error)]
pub enum RegisterInstallationError {
    #[error("idempotency key or installation key is already owned by another request")]
    Conflict,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct ClientInstallationRepo {
    pool: Pool<Any>,
}

impl ClientInstallationRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn register(
        &self,
        instance_id: Uuid,
        user_id: Uuid,
        idempotency_key: Uuid,
        request_hash: &str,
        request: &RegisterClientRequest,
    ) -> Result<ClientInstallation, RegisterInstallationError> {
        let mut tx = self.pool.begin().await?;
        let instance = instance_id.to_string();
        let user = user_id.to_string();
        let idempotency = idempotency_key.to_string();
        let now = Utc::now();
        let replay_cutoff = (now - TimeDelta::hours(24)).to_rfc3339();

        sqlx::query(
            "DELETE FROM client_registration_idempotency \
             WHERE instance_id = ? AND created_at < ?",
        )
        .bind(&instance)
        .bind(replay_cutoff)
        .execute(&mut *tx)
        .await?;

        if let Some(row) = sqlx::query(
            "SELECT request_hash, installation_id FROM client_registration_idempotency \
             WHERE instance_id = ? AND user_id = ? AND idempotency_key = ?",
        )
        .bind(&instance)
        .bind(&user)
        .bind(&idempotency)
        .fetch_optional(&mut *tx)
        .await?
        {
            let stored_hash: String = row.get("request_hash");
            if stored_hash != request_hash {
                return Err(RegisterInstallationError::Conflict);
            }
            let installation_id: String = row.get("installation_id");
            let installation = fetch_by_id(&mut *tx, &installation_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?;
            tx.commit().await?;
            return Ok(installation);
        }

        let existing = sqlx::query(
            "SELECT id, user_id FROM client_installations \
             WHERE instance_id = ? AND installation_key = ?",
        )
        .bind(&instance)
        .bind(request.installation_key.to_string())
        .fetch_optional(&mut *tx)
        .await?;

        let installation_id = if let Some(row) = existing {
            let existing_owner: String = row.get("user_id");
            if existing_owner != user {
                return Err(RegisterInstallationError::Conflict);
            }
            let id: String = row.get("id");
            sqlx::query(
                "UPDATE client_installations SET display_name = ?, platform = ?, \
                 evoflux_version = ?, workspace_association = ?, last_seen_at = ?, updated_at = ? \
                 WHERE id = ? AND instance_id = ? AND user_id = ?",
            )
            .bind(request.display_name.trim())
            .bind(platform_name(request.platform))
            .bind(request.evoflux_version.trim())
            .bind(request.workspace_association.as_deref())
            .bind(now.to_rfc3339())
            .bind(now.to_rfc3339())
            .bind(&id)
            .bind(&instance)
            .bind(&user)
            .execute(&mut *tx)
            .await?;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO client_installations (
                    id, instance_id, user_id, installation_key, display_name, platform,
                    evoflux_version, workspace_association, connected_at, last_seen_at,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&id)
            .bind(&instance)
            .bind(&user)
            .bind(request.installation_key.to_string())
            .bind(request.display_name.trim())
            .bind(platform_name(request.platform))
            .bind(request.evoflux_version.trim())
            .bind(request.workspace_association.as_deref())
            .bind(now.to_rfc3339())
            .bind(now.to_rfc3339())
            .bind(now.to_rfc3339())
            .bind(now.to_rfc3339())
            .execute(&mut *tx)
            .await?;
            id
        };

        sqlx::query(
            "INSERT INTO client_registration_idempotency \
             (instance_id, user_id, idempotency_key, request_hash, installation_id, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&instance)
        .bind(&user)
        .bind(&idempotency)
        .bind(request_hash)
        .bind(&installation_id)
        .bind(now.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        let installation = fetch_by_id(&mut *tx, &installation_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(installation)
    }

    pub async fn heartbeat(
        &self,
        installation_id: Uuid,
        instance_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ClientInstallation>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE client_installations SET last_seen_at = ?, updated_at = ? \
             WHERE id = ? AND instance_id = ? AND user_id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(installation_id.to_string())
        .bind(instance_id.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.find_by_id(installation_id).await
    }

    pub async fn find_by_id(
        &self,
        installation_id: Uuid,
    ) -> Result<Option<ClientInstallation>, sqlx::Error> {
        let mut connection = self.pool.acquire().await?;
        fetch_by_id(&mut *connection, &installation_id.to_string()).await
    }

    pub async fn list_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ClientInstallation>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, instance_id, user_id, installation_key, display_name, platform, \
             evoflux_version, workspace_association, connected_at, last_seen_at, created_at, updated_at \
             FROM client_installations WHERE user_id = ? ORDER BY last_seen_at DESC",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(map_installation).collect()
    }
}

async fn fetch_by_id<'e, E>(
    executor: E,
    installation_id: &str,
) -> Result<Option<ClientInstallation>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = Any>,
{
    let row = sqlx::query(
        "SELECT id, instance_id, user_id, installation_key, display_name, platform, \
         evoflux_version, workspace_association, connected_at, last_seen_at, created_at, updated_at \
         FROM client_installations WHERE id = ?",
    )
    .bind(installation_id)
    .fetch_optional(executor)
    .await?;
    row.map(map_installation).transpose()
}

fn map_installation(row: sqlx::any::AnyRow) -> Result<ClientInstallation, sqlx::Error> {
    Ok(ClientInstallation {
        id: parse_uuid(&row, "id")?,
        instance_id: parse_uuid(&row, "instance_id")?,
        user_id: parse_uuid(&row, "user_id")?,
        installation_key: parse_uuid(&row, "installation_key")?,
        display_name: row.get("display_name"),
        platform: parse_platform(row.get::<String, _>("platform").as_str()),
        evoflux_version: row.get("evoflux_version"),
        workspace_association: row.get("workspace_association"),
        connected_at: parse_dt(row.get("connected_at")),
        last_seen_at: parse_dt(row.get("last_seen_at")),
        created_at: parse_dt(row.get("created_at")),
        updated_at: parse_dt(row.get("updated_at")),
    })
}

fn parse_uuid(row: &sqlx::any::AnyRow, column: &str) -> Result<Uuid, sqlx::Error> {
    let value: String = row.get(column);
    Uuid::parse_str(&value).map_err(|error| sqlx::Error::Decode(Box::new(error)))
}

fn platform_name(platform: ClientPlatform) -> &'static str {
    match platform {
        ClientPlatform::Macos => "macos",
        ClientPlatform::Linux => "linux",
        ClientPlatform::Windows => "windows",
    }
}

fn parse_platform(value: &str) -> ClientPlatform {
    match value {
        "macos" => ClientPlatform::Macos,
        "windows" => ClientPlatform::Windows,
        _ => ClientPlatform::Linux,
    }
}
