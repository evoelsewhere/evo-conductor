use chrono::{DateTime, Utc};
use conductor_domain::{ConnectionSecret, SecretScope};
use sqlx::{Any, Pool};
use sqlx::Row;
use uuid::Uuid;

use crate::mapping::parse_dt;

#[derive(Clone)]
pub struct SecretRepo {
    pool: Pool<Any>,
}

impl SecretRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        owner_user_id: Uuid,
        name: &str,
        prefix: &str,
        token_hash: &str,
        scopes: &[SecretScope],
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ConnectionSecret, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let scopes_json = serde_json::to_string(scopes).unwrap_or_else(|_| "[]".into());

        sqlx::query(
            r#"
            INSERT INTO connection_secrets (
                id, name, prefix, token_hash, owner_user_id, scopes,
                expires_at, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(name)
        .bind(prefix)
        .bind(token_hash)
        .bind(owner_user_id.to_string())
        .bind(scopes_json)
        .bind(expires_at.map(|t| t.to_rfc3339()))
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(ConnectionSecret {
            id,
            name: name.to_string(),
            prefix: prefix.to_string(),
            owner_user_id,
            scopes: scopes.to_vec(),
            last_used_at: None,
            expires_at,
            revoked_at: None,
            created_at: now,
        })
    }

    pub async fn list_for_user(
        &self,
        owner_user_id: Uuid,
    ) -> Result<Vec<ConnectionSecret>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, prefix, owner_user_id, scopes, last_used_at,
                   expires_at, revoked_at, created_at
            FROM connection_secrets
            WHERE owner_user_id = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(owner_user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(map_secret).collect())
    }

    pub async fn revoke(&self, id: Uuid, owner_user_id: Uuid) -> Result<bool, sqlx::Error> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            UPDATE connection_secrets
            SET revoked_at = ?
            WHERE id = ? AND owner_user_id = ? AND revoked_at IS NULL
            "#,
        )
        .bind(now.to_rfc3339())
        .bind(id.to_string())
        .bind(owner_user_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<ConnectionSecret>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, name, prefix, owner_user_id, scopes, last_used_at,
                   expires_at, revoked_at, created_at
            FROM connection_secrets
            WHERE token_hash = ? AND revoked_at IS NULL
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(map_secret))
    }
}

fn map_secret(r: sqlx::any::AnyRow) -> ConnectionSecret {
    let scopes: String = r.get("scopes");
    ConnectionSecret {
        id: Uuid::parse_str(r.get::<String, _>("id").as_str()).unwrap_or_else(|_| Uuid::nil()),
        name: r.get("name"),
        prefix: r.get("prefix"),
        owner_user_id: Uuid::parse_str(r.get::<String, _>("owner_user_id").as_str())
            .unwrap_or_else(|_| Uuid::nil()),
        scopes: serde_json::from_str(&scopes).unwrap_or_default(),
        last_used_at: r.get::<Option<String>, _>("last_used_at").map(parse_dt),
        expires_at: r.get::<Option<String>, _>("expires_at").map(parse_dt),
        revoked_at: r.get::<Option<String>, _>("revoked_at").map(parse_dt),
        created_at: parse_dt(r.get("created_at")),
    }
}
