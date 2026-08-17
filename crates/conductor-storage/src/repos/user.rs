use chrono::Utc;
use conductor_domain::{CreateMemberRequest, MemberListQuery, PrimaryRole, User, UserStatus};
use sqlx::{Any, AnyConnection, Pool, Row};
use uuid::Uuid;

use crate::core::error::{
    InvalidPersistedPrincipal, PersistedPrincipalField, PersistedSecurityReason, StorageError,
    StorageResult,
};
use crate::core::mapping::map_user_row;

pub(crate) const USER_SELECT: &str = r#"
    SELECT id, email, display_name, password_hash, primary_role, status,
           must_change_password, session_version, sso_issuer, sso_subject,
           last_seen_at, created_at
    FROM users
"#;

pub(crate) async fn sub_role_ids_for_on(
    connection: &mut AnyConnection,
    user_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT sub_role_id FROM user_sub_roles WHERE user_id = ? ORDER BY sub_role_id ASC",
    )
    .bind(user_id.to_string())
    .fetch_all(connection)
    .await?;
    Ok(rows.iter().map(|row| row.get("sub_role_id")).collect())
}

pub(crate) async fn tag_ids_for_on(
    connection: &mut AnyConnection,
    user_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT tag_id FROM tag_assignments \
         WHERE entity_type = 'member' AND entity_id = ? ORDER BY tag_id ASC",
    )
    .bind(user_id.to_string())
    .fetch_all(connection)
    .await?;
    Ok(rows.iter().map(|row| row.get("tag_id")).collect())
}

pub(crate) async fn replace_sub_roles_on(
    connection: &mut AnyConnection,
    user_id: Uuid,
    sub_role_ids: &[String],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM user_sub_roles WHERE user_id = ?")
        .bind(user_id.to_string())
        .execute(&mut *connection)
        .await?;
    for sub_role_id in sub_role_ids {
        sqlx::query("INSERT INTO user_sub_roles (user_id, sub_role_id) VALUES (?, ?)")
            .bind(user_id.to_string())
            .bind(sub_role_id)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

pub(crate) async fn replace_tags_on(
    connection: &mut AnyConnection,
    user_id: Uuid,
    tag_ids: &[String],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM tag_assignments WHERE entity_type = 'member' AND entity_id = ?")
        .bind(user_id.to_string())
        .execute(&mut *connection)
        .await?;
    let now = Utc::now().to_rfc3339();
    for tag_id in tag_ids {
        sqlx::query(
            "INSERT INTO tag_assignments (tag_id, entity_type, entity_id, created_at) \
             VALUES (?, 'member', ?, ?)",
        )
        .bind(tag_id)
        .bind(user_id.to_string())
        .bind(&now)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

pub(crate) async fn find_by_id_on(
    connection: &mut AnyConnection,
    id: Uuid,
) -> StorageResult<Option<User>> {
    let row = sqlx::query(&format!("{USER_SELECT} WHERE id = ?"))
        .bind(id.to_string())
        .fetch_optional(&mut *connection)
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let mut user = map_user_row(&row)?;
    user.sub_role_ids = sub_role_ids_for_on(&mut *connection, id).await?;
    user.tag_ids = tag_ids_for_on(&mut *connection, id).await?;
    Ok(Some(user))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberDirectoryRecord {
    pub id: Uuid,
    pub display_name: String,
    pub primary_role: PrimaryRole,
}

#[derive(Debug)]
pub enum SsoLoginError {
    Database(sqlx::Error),
    InvalidPersistedPrincipal(InvalidPersistedPrincipal),
    IdentityConflict,
}

impl From<sqlx::Error> for SsoLoginError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl From<StorageError> for SsoLoginError {
    fn from(value: StorageError) -> Self {
        match value {
            StorageError::Database(error) => Self::Database(error),
            StorageError::InvalidPersistedPrincipal(error) => {
                Self::InvalidPersistedPrincipal(error)
            }
            // These variants cannot originate from a user security read. Keep
            // them operational instead of disguising them as bad credentials.
            StorageError::InvalidPersistedCredential(_)
            | StorageError::InvalidPersistedResource(_)
            | StorageError::Serialization(_) => Self::Database(sqlx::Error::Protocol(
                "unexpected storage error while reading principal".into(),
            )),
        }
    }
}

#[derive(Clone)]
pub struct UserRepo {
    pool: Pool<Any>,
}

impl UserRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    async fn attach_junctions(&self, mut user: User) -> StorageResult<User> {
        user.sub_role_ids = self.sub_role_ids_for(user.id).await?;
        user.tag_ids = self.tag_ids_for(user.id).await?;
        Ok(user)
    }

    async fn attach_junctions_batch(&self, mut users: Vec<User>) -> StorageResult<Vec<User>> {
        if users.is_empty() {
            return Ok(users);
        }
        let ids: Vec<String> = users.iter().map(|u| u.id.to_string()).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sub_sql = format!(
            "SELECT user_id, sub_role_id FROM user_sub_roles WHERE user_id IN ({placeholders})"
        );
        let mut sub_q = sqlx::query(&sub_sql);
        for id in &ids {
            sub_q = sub_q.bind(id);
        }
        let sub_rows = sub_q.fetch_all(&self.pool).await?;

        let tag_sql = format!(
            "SELECT entity_id AS user_id, tag_id FROM tag_assignments \
             WHERE entity_type = 'member' AND entity_id IN ({placeholders})"
        );
        let mut tag_q = sqlx::query(&tag_sql);
        for id in &ids {
            tag_q = tag_q.bind(id);
        }
        let tag_rows = tag_q.fetch_all(&self.pool).await?;

        use std::collections::HashMap;
        let mut subs: HashMap<String, Vec<String>> = HashMap::new();
        for r in sub_rows {
            let uid: String = r.get("user_id");
            let rid: String = r.get("sub_role_id");
            subs.entry(uid).or_default().push(rid);
        }
        let mut tags: HashMap<String, Vec<String>> = HashMap::new();
        for r in tag_rows {
            let uid: String = r.get("user_id");
            let tid: String = r.get("tag_id");
            tags.entry(uid).or_default().push(tid);
        }

        for user in &mut users {
            let key = user.id.to_string();
            user.sub_role_ids = subs.remove(&key).unwrap_or_default();
            user.tag_ids = tags.remove(&key).unwrap_or_default();
        }
        Ok(users)
    }

    pub async fn sub_role_ids_for(&self, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
        let mut connection = self.pool.acquire().await?;
        sub_role_ids_for_on(&mut connection, user_id).await
    }

    pub async fn tag_ids_for(&self, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
        let mut connection = self.pool.acquire().await?;
        tag_ids_for_on(&mut connection, user_id).await
    }

    pub async fn find_by_email(
        &self,
        email: &str,
    ) -> StorageResult<Option<(User, Option<String>)>> {
        let row = sqlx::query(&format!("{USER_SELECT} WHERE email = ?"))
            .bind(email.trim().to_lowercase())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => {
                let hash: Option<String> = r.get("password_hash");
                let user = self.attach_junctions(map_user_row(&r)?).await?;
                Ok(Some((user, hash)))
            }
            None => Ok(None),
        }
    }

    pub async fn find_by_id(&self, id: Uuid) -> StorageResult<Option<User>> {
        let row = sqlx::query(&format!("{USER_SELECT} WHERE id = ?"))
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(self.attach_junctions(map_user_row(&r)?).await?)),
            None => Ok(None),
        }
    }

    pub async fn session_version(&self, id: Uuid) -> StorageResult<Option<i64>> {
        let version = sqlx::query_scalar("SELECT session_version FROM users WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| match error {
                sqlx::Error::ColumnDecode { .. } => {
                    StorageError::InvalidPersistedPrincipal(InvalidPersistedPrincipal::new(
                        Some(id),
                        PersistedPrincipalField::SessionVersion,
                        PersistedSecurityReason::InvalidInteger,
                    ))
                }
                operational => StorageError::Database(operational),
            })?;
        match version {
            Some(version) if version < 0 => Err(InvalidPersistedPrincipal::new(
                Some(id),
                PersistedPrincipalField::SessionVersion,
                PersistedSecurityReason::InvalidInteger,
            )
            .into()),
            version => Ok(version),
        }
    }

    pub async fn list(&self) -> StorageResult<Vec<User>> {
        let rows = sqlx::query(&format!("{USER_SELECT} ORDER BY created_at ASC"))
            .fetch_all(&self.pool)
            .await?;
        let users: Result<Vec<_>, _> = rows.iter().map(map_user_row).collect();
        self.attach_junctions_batch(users?).await
    }

    pub async fn list_filtered(&self, query: &MemberListQuery) -> StorageResult<(Vec<User>, u64)> {
        let page = query.page.max(1);
        let limit = query.limit.clamp(1, 100);
        let offset = ((page - 1) as i64) * (limit as i64);

        let mut where_parts: Vec<&str> = Vec::new();
        let mut binds: Vec<String> = Vec::new();

        if query.active_only {
            where_parts.push("status = 'active'");
        }
        if let Some(status) = query.status {
            where_parts.push("status = ?");
            binds.push(status.as_str().to_string());
        }
        if let Some(role) = query.role {
            where_parts.push("primary_role = ?");
            binds.push(role.as_str().to_string());
        }
        if let Some(ref q) = query.q {
            let trimmed = q.trim();
            if !trimmed.is_empty() {
                where_parts.push("(LOWER(email) LIKE ? OR LOWER(display_name) LIKE ?)");
                let pattern = format!("%{}%", trimmed.to_lowercase());
                binds.push(pattern.clone());
                binds.push(pattern);
            }
        }
        if let Some(ref tag) = query.tag {
            where_parts.push(
                "id IN (SELECT entity_id FROM tag_assignments WHERE entity_type = 'member' AND (tag_id = ? OR tag_id IN (SELECT id FROM tags WHERE slug = ?)))",
            );
            binds.push(tag.clone());
            binds.push(tag.clone());
        }

        let where_sql = if where_parts.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_parts.join(" AND "))
        };

        let count_sql = format!("SELECT COUNT(*) AS c FROM users{where_sql}");
        let mut count_q = sqlx::query(&count_sql);
        for b in &binds {
            count_q = count_q.bind(b);
        }
        let count_row = count_q.fetch_one(&self.pool).await?;
        let total: i64 = count_row.get("c");

        let list_sql =
            format!("{USER_SELECT}{where_sql} ORDER BY created_at DESC LIMIT ? OFFSET ?");
        let mut list_q = sqlx::query(&list_sql);
        for b in &binds {
            list_q = list_q.bind(b);
        }
        list_q = list_q.bind(limit as i64).bind(offset);
        let rows = list_q.fetch_all(&self.pool).await?;
        let users: Result<Vec<_>, _> = rows.iter().map(map_user_row).collect();
        let users = self.attach_junctions_batch(users?).await?;
        Ok((users, total as u64))
    }

    /// Active-member directory projection for non-Admin callers.
    ///
    /// Search deliberately covers display name only. Reusing `list_filtered`
    /// here would let a Contributor infer an email or private tag by observing
    /// which record matched a guessed filter.
    pub async fn list_active_directory(
        &self,
        q: Option<&str>,
        role: Option<PrimaryRole>,
        page: u32,
        limit: u32,
    ) -> StorageResult<(Vec<MemberDirectoryRecord>, u64)> {
        let page = page.max(1);
        let limit = limit.clamp(1, 100);
        let offset = ((page - 1) as i64) * i64::from(limit);
        let mut where_parts = vec!["status = 'active'"];
        let mut binds = Vec::new();
        if let Some(role) = role {
            where_parts.push("primary_role = ?");
            binds.push(role.as_str().to_string());
        }
        if let Some(q) = q.map(str::trim).filter(|q| !q.is_empty()) {
            where_parts.push("LOWER(display_name) LIKE ?");
            binds.push(format!("%{}%", q.to_lowercase()));
        }
        let where_sql = format!(" WHERE {}", where_parts.join(" AND "));

        let count_sql = format!("SELECT COUNT(*) AS c FROM users{where_sql}");
        let mut count_query = sqlx::query(&count_sql);
        for bind in &binds {
            count_query = count_query.bind(bind);
        }
        let total: i64 = count_query.fetch_one(&self.pool).await?.get("c");

        let list_sql = format!(
            "SELECT id, display_name, primary_role FROM users{where_sql} \
             ORDER BY LOWER(display_name) ASC, id ASC LIMIT ? OFFSET ?"
        );
        let mut list_query = sqlx::query(&list_sql);
        for bind in &binds {
            list_query = list_query.bind(bind);
        }
        let rows = list_query
            .bind(i64::from(limit))
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let id_raw: String = row.get("id");
            let id = Uuid::parse_str(&id_raw).map_err(|_| {
                InvalidPersistedPrincipal::new(
                    None,
                    PersistedPrincipalField::Id,
                    PersistedSecurityReason::InvalidUuid,
                )
            })?;
            let role_raw: String = row.get("primary_role");
            let primary_role = PrimaryRole::parse(&role_raw).ok_or_else(|| {
                InvalidPersistedPrincipal::new(
                    Some(id),
                    PersistedPrincipalField::PrimaryRole,
                    PersistedSecurityReason::UnknownValue,
                )
            })?;
            entries.push(MemberDirectoryRecord {
                id,
                display_name: row.get("display_name"),
                primary_role,
            });
        }
        Ok((entries, total as u64))
    }

    pub async fn pending_count(&self) -> Result<u64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM users WHERE status = 'pending'")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("c") as u64)
    }

    pub async fn create_invited(
        &self,
        req: &CreateMemberRequest,
        password_hash: &str,
        invited_by: Uuid,
    ) -> StorageResult<User> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let email = req.email.trim().to_lowercase();

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, display_name, password_hash, primary_role, status,
                must_change_password, invited_by, created_at
            ) VALUES (?, ?, ?, ?, ?, 'invited', 1, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&email)
        .bind(req.display_name.trim())
        .bind(password_hash)
        .bind(req.primary_role.as_str())
        .bind(invited_by.to_string())
        .bind(now.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        replace_sub_roles_on(&mut tx, id, &req.sub_role_ids).await?;
        replace_tags_on(&mut tx, id, &req.tag_ids).await?;
        let user = find_by_id_on(&mut tx, id)
            .await?
            .ok_or_else(|| StorageError::Database(sqlx::Error::RowNotFound))?;
        tx.commit().await?;

        Ok(user)
    }

    pub async fn create_pending_sso(
        &self,
        issuer: &str,
        subject: &str,
        email: &str,
        display_name: &str,
    ) -> StorageResult<User> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let email = email.to_lowercase();
        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, display_name, password_hash, primary_role, status,
                must_change_password, sso_issuer, sso_subject, created_at
            ) VALUES (?, ?, ?, NULL, 'user', 'pending', 0, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&email)
        .bind(display_name.trim())
        .bind(issuer)
        .bind(subject)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        self.find_by_id(id)
            .await?
            .ok_or_else(|| StorageError::Database(sqlx::Error::RowNotFound))
    }

    /// SSO login path: return existing user, create pending if new, activate invited.
    pub async fn handle_sso_login(
        &self,
        issuer: &str,
        subject: &str,
        email: &str,
        display_name: &str,
    ) -> Result<User, SsoLoginError> {
        let email = email.trim().to_lowercase();

        let identity_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM users WHERE sso_issuer = ? AND sso_subject = ?")
                .bind(issuer)
                .bind(subject)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| match error {
                    sqlx::Error::ColumnDecode { .. } => {
                        SsoLoginError::InvalidPersistedPrincipal(InvalidPersistedPrincipal::new(
                            None,
                            PersistedPrincipalField::Id,
                            PersistedSecurityReason::InvalidUuid,
                        ))
                    }
                    operational => SsoLoginError::Database(operational),
                })?;

        let mut existing = if let Some(id) = identity_id {
            let id = Uuid::parse_str(&id).map_err(|_| {
                SsoLoginError::InvalidPersistedPrincipal(InvalidPersistedPrincipal::new(
                    None,
                    PersistedPrincipalField::Id,
                    PersistedSecurityReason::InvalidUuid,
                ))
            })?;
            self.find_by_id(id).await?.map(|user| (user, None))
        } else {
            self.find_by_email(&email).await?
        };

        if let Some((mut user, _)) = existing.take() {
            let binding = sqlx::query("SELECT sso_issuer, sso_subject FROM users WHERE id = ?")
                .bind(user.id.to_string())
                .fetch_one(&self.pool)
                .await?;
            let bound_issuer: Option<String> = binding.get("sso_issuer");
            let bound_subject: Option<String> = binding.get("sso_subject");
            match (bound_issuer, bound_subject) {
                (Some(bound_issuer), Some(bound_subject))
                    if bound_issuer != issuer || bound_subject != subject =>
                {
                    return Err(SsoLoginError::IdentityConflict);
                }
                (None, None) => {
                    sqlx::query("UPDATE users SET sso_issuer = ?, sso_subject = ? WHERE id = ?")
                        .bind(issuer)
                        .bind(subject)
                        .bind(user.id.to_string())
                        .execute(&self.pool)
                        .await?;
                }
                _ => return Err(SsoLoginError::IdentityConflict),
            }

            if !display_name.is_empty() && user.display_name != display_name {
                sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
                    .bind(display_name.trim())
                    .bind(user.id.to_string())
                    .execute(&self.pool)
                    .await?;
                user.display_name = display_name.to_string();
            }
            if user.status == UserStatus::Invited {
                sqlx::query(
                    "UPDATE users SET status = 'active', must_change_password = 0 WHERE id = ?",
                )
                .bind(user.id.to_string())
                .execute(&self.pool)
                .await?;
                user.status = UserStatus::Active;
                user.must_change_password = false;
            }
            return Ok(user);
        }
        Ok(self
            .create_pending_sso(issuer, subject, &email, display_name)
            .await?)
    }

    pub async fn activate_invited_on_password_login(&self, user_id: Uuid) -> StorageResult<User> {
        sqlx::query("UPDATE users SET status = 'active' WHERE id = ? AND status = 'invited'")
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        self.find_by_id(user_id)
            .await?
            .ok_or_else(|| StorageError::Database(sqlx::Error::RowNotFound))
    }

    pub async fn set_password(
        &self,
        user_id: Uuid,
        password_hash: &str,
        must_change: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE users SET password_hash = ?, must_change_password = ?, \
             session_version = session_version + 1 WHERE id = ?",
        )
        .bind(password_hash)
        .bind(if must_change { 1 } else { 0 })
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_must_change_password(&self, user_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET must_change_password = 0 WHERE id = ?")
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn all_users_exist(&self, ids: &[Uuid]) -> Result<bool, sqlx::Error> {
        for id in ids {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?")
                .bind(id.to_string())
                .fetch_one(&self.pool)
                .await?;
            if count == 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn all_users_active(&self, ids: &[Uuid]) -> Result<bool, sqlx::Error> {
        for id in ids {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ? AND status = 'active'")
                    .bind(id.to_string())
                    .fetch_one(&self.pool)
                    .await?;
            if count == 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
