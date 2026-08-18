use chrono::{DateTime, Utc};
use conductor_domain::{ConnectionSecret, SecretScope};
use sqlx::Row;
use sqlx::{Any, Pool};
use uuid::Uuid;

use crate::core::error::{
    InvalidPersistedCredential, PersistedCredentialField, PersistedSecurityReason, StorageError,
    StorageResult,
};

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
    ) -> StorageResult<ConnectionSecret> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        validate_scopes(scopes, Some(id))?;
        let scopes_json = serde_json::to_string(scopes).map_err(StorageError::Serialization)?;

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

    pub async fn list_for_user(&self, owner_user_id: Uuid) -> StorageResult<Vec<ConnectionSecret>> {
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

        rows.into_iter().map(map_secret).collect()
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

    pub async fn find_by_hash(&self, token_hash: &str) -> StorageResult<Option<ConnectionSecret>> {
        let mut rows = sqlx::query(
            r#"
            SELECT id, name, prefix, owner_user_id, scopes, last_used_at,
                   expires_at, revoked_at, created_at
            FROM connection_secrets
            WHERE token_hash = ? AND revoked_at IS NULL
            LIMIT 2
            "#,
        )
        .bind(token_hash)
        .fetch_all(&self.pool)
        .await?;

        if rows.len() > 1 {
            return Err(InvalidPersistedCredential::new(
                None,
                PersistedCredentialField::TokenHash,
                PersistedSecurityReason::DuplicateValue,
            )
            .into());
        }
        rows.pop().map(map_secret).transpose()
    }

    /// Loads a credential for policy/realtime revalidation even after it was revoked.
    /// Lifecycle fields are decoded strictly before the caller evaluates them.
    pub async fn find_by_id(&self, id: Uuid) -> StorageResult<Option<ConnectionSecret>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, prefix, owner_user_id, scopes, last_used_at,
                   expires_at, revoked_at, created_at
            FROM connection_secrets
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_secret).transpose()
    }

    pub async fn mark_used(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE connection_secrets SET last_used_at = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn is_active(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM connection_secrets WHERE id = ? AND revoked_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }
}

fn map_secret(r: sqlx::any::AnyRow) -> StorageResult<ConnectionSecret> {
    let id_raw: String = r.try_get("id").map_err(|error| {
        credential_column_error(
            error,
            None,
            PersistedCredentialField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let id = Uuid::parse_str(&id_raw).map_err(|_| {
        InvalidPersistedCredential::new(
            None,
            PersistedCredentialField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let invalid = |field, reason| InvalidPersistedCredential::new(Some(id), field, reason);

    let owner_raw: String = r.try_get("owner_user_id").map_err(|error| {
        credential_column_error(
            error,
            Some(id),
            PersistedCredentialField::OwnerUserId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let owner_user_id = Uuid::parse_str(&owner_raw).map_err(|_| {
        invalid(
            PersistedCredentialField::OwnerUserId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let scopes_raw: String = r.try_get("scopes").map_err(|error| {
        credential_column_error(
            error,
            Some(id),
            PersistedCredentialField::Scopes,
            PersistedSecurityReason::MalformedPayload,
        )
    })?;
    let scopes: Vec<SecretScope> = serde_json::from_str(&scopes_raw).map_err(|_| {
        invalid(
            PersistedCredentialField::Scopes,
            PersistedSecurityReason::MalformedPayload,
        )
    })?;
    validate_scopes(&scopes, Some(id))?;

    let last_used_at_raw = r.try_get("last_used_at").map_err(|error| {
        credential_column_error(
            error,
            Some(id),
            PersistedCredentialField::LastUsedAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let last_used_at =
        parse_optional_credential_dt(last_used_at_raw, id, PersistedCredentialField::LastUsedAt)?;
    let expires_at_raw = r.try_get("expires_at").map_err(|error| {
        credential_column_error(
            error,
            Some(id),
            PersistedCredentialField::ExpiresAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let expires_at =
        parse_optional_credential_dt(expires_at_raw, id, PersistedCredentialField::ExpiresAt)?;
    let revoked_at_raw = r.try_get("revoked_at").map_err(|error| {
        credential_column_error(
            error,
            Some(id),
            PersistedCredentialField::RevokedAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let revoked_at =
        parse_optional_credential_dt(revoked_at_raw, id, PersistedCredentialField::RevokedAt)?;
    let created_at_raw: String = r.try_get("created_at").map_err(|error| {
        credential_column_error(
            error,
            Some(id),
            PersistedCredentialField::CreatedAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let created_at = parse_credential_dt(&created_at_raw, id, PersistedCredentialField::CreatedAt)?;

    Ok(ConnectionSecret {
        id,
        name: r.try_get("name")?,
        prefix: r.try_get("prefix")?,
        owner_user_id,
        scopes,
        last_used_at,
        expires_at,
        revoked_at,
        created_at,
    })
}

fn credential_column_error(
    error: sqlx::Error,
    credential_id: Option<Uuid>,
    field: PersistedCredentialField,
    reason: PersistedSecurityReason,
) -> StorageError {
    match error {
        sqlx::Error::ColumnDecode { .. } => {
            InvalidPersistedCredential::new(credential_id, field, reason).into()
        }
        operational => StorageError::Database(operational),
    }
}

fn validate_scopes(scopes: &[SecretScope], id: Option<Uuid>) -> StorageResult<()> {
    if scopes.is_empty() {
        return Err(InvalidPersistedCredential::new(
            id,
            PersistedCredentialField::Scopes,
            PersistedSecurityReason::EmptyCollection,
        )
        .into());
    }
    if scopes
        .iter()
        .enumerate()
        .any(|(index, scope)| scopes[..index].contains(scope))
    {
        return Err(InvalidPersistedCredential::new(
            id,
            PersistedCredentialField::Scopes,
            PersistedSecurityReason::DuplicateValue,
        )
        .into());
    }
    Ok(())
}

fn parse_optional_credential_dt(
    value: Option<String>,
    credential_id: Uuid,
    field: PersistedCredentialField,
) -> Result<Option<DateTime<Utc>>, InvalidPersistedCredential> {
    value
        .map(|value| parse_credential_dt(&value, credential_id, field))
        .transpose()
}

fn parse_credential_dt(
    value: &str,
    credential_id: Uuid,
    field: PersistedCredentialField,
) -> Result<DateTime<Utc>, InvalidPersistedCredential> {
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc))
        .map_err(|_| {
            InvalidPersistedCredential::new(
                Some(credential_id),
                field,
                PersistedSecurityReason::InvalidTimestamp,
            )
        })
}
