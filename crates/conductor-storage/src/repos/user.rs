use chrono::Utc;
use conductor_domain::{
    CreateMemberRequest, MemberListQuery, PrimaryRole, User, UserStatus,
};
use sqlx::{Any, Pool, Row};
use uuid::Uuid;

use crate::mapping::map_user_row;

const USER_SELECT: &str = r#"
    SELECT id, email, display_name, password_hash, primary_role, status,
           must_change_password, last_seen_at, created_at
    FROM users
"#;

#[derive(Clone)]
pub struct UserRepo {
    pool: Pool<Any>,
}

impl UserRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    async fn attach_junctions(&self, mut user: User) -> Result<User, sqlx::Error> {
        user.sub_role_ids = self.sub_role_ids_for(user.id).await?;
        user.tag_ids = self.tag_ids_for(user.id).await?;
        Ok(user)
    }

    async fn attach_junctions_batch(&self, mut users: Vec<User>) -> Result<Vec<User>, sqlx::Error> {
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
        let rows = sqlx::query("SELECT sub_role_id FROM user_sub_roles WHERE user_id = ?")
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|r| r.get("sub_role_id")).collect())
    }

    pub async fn tag_ids_for(&self, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT tag_id FROM tag_assignments \
             WHERE entity_type = 'member' AND entity_id = ?",
        )
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|r| r.get("tag_id")).collect())
    }

    pub async fn set_sub_roles(
        &self,
        user_id: Uuid,
        sub_role_ids: &[String],
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM user_sub_roles WHERE user_id = ?")
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        for rid in sub_role_ids {
            sqlx::query("INSERT INTO user_sub_roles (user_id, sub_role_id) VALUES (?, ?)")
                .bind(user_id.to_string())
                .bind(rid)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn set_tags(&self, user_id: Uuid, tag_ids: &[String]) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM tag_assignments WHERE entity_type = 'member' AND entity_id = ?",
        )
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        for tid in tag_ids {
            sqlx::query(
                "INSERT INTO tag_assignments (tag_id, entity_type, entity_id, created_at) \
                 VALUES (?, 'member', ?, ?)",
            )
                .bind(tid)
                .bind(user_id.to_string())
                .bind(Utc::now().to_rfc3339())
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn find_by_email(
        &self,
        email: &str,
    ) -> Result<Option<(User, Option<String>)>, sqlx::Error> {
        let row = sqlx::query(&format!("{USER_SELECT} WHERE email = ?"))
            .bind(email.to_lowercase())
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

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query(&format!("{USER_SELECT} WHERE id = ?"))
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(self.attach_junctions(map_user_row(&r)?).await?)),
            None => Ok(None),
        }
    }

    pub async fn list(&self) -> Result<Vec<User>, sqlx::Error> {
        let rows = sqlx::query(&format!("{USER_SELECT} ORDER BY created_at ASC"))
            .fetch_all(&self.pool)
            .await?;
        let users: Result<Vec<_>, _> = rows.iter().map(map_user_row).collect();
        self.attach_junctions_batch(users?).await
    }

    pub async fn list_filtered(
        &self,
        query: &MemberListQuery,
    ) -> Result<(Vec<User>, u64), sqlx::Error> {
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

        let list_sql = format!(
            "{USER_SELECT}{where_sql} ORDER BY created_at DESC LIMIT ? OFFSET ?"
        );
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
    ) -> Result<User, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let email = req.email.trim().to_lowercase();

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
        .execute(&self.pool)
        .await?;

        self.set_sub_roles(id, &req.sub_role_ids).await?;
        self.set_tags(id, &req.tag_ids).await?;

        self.find_by_id(id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    pub async fn create_pending_sso(
        &self,
        email: &str,
        display_name: &str,
    ) -> Result<User, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let email = email.to_lowercase();
        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, display_name, password_hash, primary_role, status,
                must_change_password, created_at
            ) VALUES (?, ?, ?, NULL, 'user', 'pending', 0, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&email)
        .bind(display_name)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        self.find_by_id(id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    /// SSO login path: return existing user, create pending if new, activate invited.
    pub async fn handle_sso_login(
        &self,
        email: &str,
        display_name: &str,
    ) -> Result<User, sqlx::Error> {
        let email = email.to_lowercase();
        if let Some((mut user, _)) = self.find_by_email(&email).await? {
            if !display_name.is_empty() && user.display_name != display_name {
                sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
                    .bind(display_name)
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
        self.create_pending_sso(&email, display_name).await
    }

    pub async fn activate_invited_on_password_login(
        &self,
        user_id: Uuid,
    ) -> Result<User, sqlx::Error> {
        sqlx::query("UPDATE users SET status = 'active' WHERE id = ? AND status = 'invited'")
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        self.find_by_id(user_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    pub async fn approve(
        &self,
        user_id: Uuid,
        approver_id: Uuid,
        primary_role: Option<PrimaryRole>,
        sub_role_ids: Option<&[String]>,
        tag_ids: Option<&[String]>,
    ) -> Result<User, sqlx::Error> {
        let now = Utc::now();
        if let Some(role) = primary_role {
            sqlx::query(
                r#"
                UPDATE users SET status = 'active', primary_role = ?,
                    approved_at = ?, approved_by = ?, must_change_password = 0
                WHERE id = ?
                "#,
            )
            .bind(role.as_str())
            .bind(now.to_rfc3339())
            .bind(approver_id.to_string())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE users SET status = 'active',
                    approved_at = ?, approved_by = ?, must_change_password = 0
                WHERE id = ?
                "#,
            )
            .bind(now.to_rfc3339())
            .bind(approver_id.to_string())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        }
        if let Some(ids) = sub_role_ids {
            self.set_sub_roles(user_id, ids).await?;
        }
        if let Some(ids) = tag_ids {
            self.set_tags(user_id, ids).await?;
        }
        self.find_by_id(user_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    pub async fn set_status(&self, user_id: Uuid, status: UserStatus) -> Result<User, sqlx::Error> {
        sqlx::query("UPDATE users SET status = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;
        self.find_by_id(user_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    pub async fn update_member(
        &self,
        user_id: Uuid,
        display_name: Option<&str>,
        primary_role: Option<PrimaryRole>,
        sub_role_ids: Option<&[String]>,
        tag_ids: Option<&[String]>,
    ) -> Result<User, sqlx::Error> {
        if let Some(name) = display_name {
            sqlx::query("UPDATE users SET display_name = ? WHERE id = ?")
                .bind(name)
                .bind(user_id.to_string())
                .execute(&self.pool)
                .await?;
        }
        if let Some(role) = primary_role {
            sqlx::query("UPDATE users SET primary_role = ? WHERE id = ?")
                .bind(role.as_str())
                .bind(user_id.to_string())
                .execute(&self.pool)
                .await?;
        }
        if let Some(ids) = sub_role_ids {
            self.set_sub_roles(user_id, ids).await?;
        }
        if let Some(ids) = tag_ids {
            self.set_tags(user_id, ids).await?;
        }
        self.find_by_id(user_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    pub async fn set_password(
        &self,
        user_id: Uuid,
        password_hash: &str,
        must_change: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE users SET password_hash = ?, must_change_password = ? WHERE id = ?",
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
}
