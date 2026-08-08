use chrono::Utc;
use conductor_domain::{PrimaryRole, User, UserStatus};
use sqlx::{Any, Pool};
use uuid::Uuid;

use crate::mapping::map_user;

#[derive(Clone)]
pub struct UserRepo {
    pool: Pool<Any>,
}

impl UserRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn find_by_email(
        &self,
        email: &str,
    ) -> Result<Option<(User, Option<String>)>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, email, display_name, password_hash, primary_role, status,
                   last_seen_at, created_at
            FROM users WHERE email = ?
            "#,
        )
        .bind(email.to_lowercase())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let user = map_user(&r)?;
                let hash: Option<String> = sqlx::Row::get(&r, "password_hash");
                Ok(Some((user, hash)))
            }
            None => Ok(None),
        }
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, email, display_name, password_hash, primary_role, status,
                   last_seen_at, created_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_user(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn list(&self) -> Result<Vec<User>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, email, display_name, password_hash, primary_role, status,
                   last_seen_at, created_at
            FROM users ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_user).collect()
    }

    /// Find-or-create a member authenticated via SSO (no local password).
    pub async fn upsert_from_sso(
        &self,
        email: &str,
        display_name: &str,
    ) -> Result<User, sqlx::Error> {
        let email = email.to_lowercase();
        if let Some((mut user, _)) = self.find_by_email(&email).await? {
            if user.display_name != display_name && !display_name.is_empty() {
                sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
                    .bind(display_name)
                    .bind(user.id.to_string())
                    .execute(&self.pool)
                    .await?;
                user.display_name = display_name.to_string();
            }
            return Ok(user);
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, display_name, password_hash, primary_role, status, created_at
            ) VALUES (?, ?, ?, NULL, 'user', 'active', ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&email)
        .bind(display_name)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(User {
            id,
            email,
            display_name: display_name.to_string(),
            primary_role: PrimaryRole::User,
            sub_role_ids: vec![],
            status: UserStatus::Active,
            last_seen_at: None,
            created_at: now,
        })
    }
}
