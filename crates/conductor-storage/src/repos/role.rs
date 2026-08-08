use chrono::Utc;
use conductor_domain::{
    CreateSubRoleRequest, CreateTagRequest, SubRole, Tag, UpdateSubRoleRequest, UpdateTagRequest,
};
use sqlx::{Any, Pool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct RoleRepo {
    pool: Pool<Any>,
}

impl RoleRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn list_sub_roles(&self) -> Result<Vec<SubRole>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, slug, name, description, color FROM sub_roles ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SubRole {
                id: r.get("id"),
                slug: r.get("slug"),
                name: r.get("name"),
                description: r.get("description"),
                color: r.get("color"),
            })
            .collect())
    }

    pub async fn create_sub_role(
        &self,
        req: &CreateSubRoleRequest,
    ) -> Result<SubRole, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO sub_roles (id, slug, name, description, color, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(req.slug.to_lowercase())
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.color)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(SubRole {
            id: id.to_string(),
            slug: req.slug.to_lowercase(),
            name: req.name.clone(),
            description: req.description.clone(),
            color: req.color.clone(),
        })
    }

    pub async fn update_sub_role(
        &self,
        id: &str,
        req: &UpdateSubRoleRequest,
    ) -> Result<Option<SubRole>, sqlx::Error> {
        let existing = sqlx::query(
            "SELECT id, slug, name, description, color FROM sub_roles WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(r) = existing else {
            return Ok(None);
        };

        let name: String = req.name.clone().unwrap_or_else(|| r.get("name"));
        let description: Option<String> = if req.description.is_some() {
            req.description.clone()
        } else {
            r.get("description")
        };
        let color: Option<String> = if req.color.is_some() {
            req.color.clone()
        } else {
            r.get("color")
        };
        let slug: String = r.get("slug");

        sqlx::query(
            "UPDATE sub_roles SET name = ?, description = ?, color = ? WHERE id = ?",
        )
        .bind(&name)
        .bind(&description)
        .bind(&color)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(Some(SubRole {
            id: id.to_string(),
            slug,
            name,
            description,
            color,
        }))
    }

    pub async fn delete_sub_role(&self, id: &str) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM user_sub_roles WHERE sub_role_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        let res = sqlx::query("DELETE FROM sub_roles WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn list_tags(&self) -> Result<Vec<Tag>, sqlx::Error> {
        let rows =
            sqlx::query("SELECT id, slug, name, description, color FROM tags ORDER BY name ASC")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows
            .into_iter()
            .map(|r| Tag {
                id: r.get("id"),
                slug: r.get("slug"),
                name: r.get("name"),
                description: r.get("description"),
                color: r.get("color"),
            })
            .collect())
    }

    pub async fn create_tag(&self, req: &CreateTagRequest) -> Result<Tag, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO tags (id, slug, name, description, color, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(req.slug.to_lowercase())
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.color)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(Tag {
            id: id.to_string(),
            slug: req.slug.to_lowercase(),
            name: req.name.clone(),
            description: req.description.clone(),
            color: req.color.clone(),
        })
    }

    pub async fn update_tag(
        &self,
        id: &str,
        req: &UpdateTagRequest,
    ) -> Result<Option<Tag>, sqlx::Error> {
        let existing =
            sqlx::query("SELECT id, slug, name, description, color FROM tags WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        let Some(r) = existing else {
            return Ok(None);
        };

        let name: String = req.name.clone().unwrap_or_else(|| r.get("name"));
        let description: Option<String> = if req.description.is_some() {
            req.description.clone()
        } else {
            r.get("description")
        };
        let color: Option<String> = if req.color.is_some() {
            req.color.clone()
        } else {
            r.get("color")
        };
        let slug: String = r.get("slug");

        sqlx::query("UPDATE tags SET name = ?, description = ?, color = ? WHERE id = ?")
            .bind(&name)
            .bind(&description)
            .bind(&color)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(Some(Tag {
            id: id.to_string(),
            slug,
            name,
            description,
            color,
        }))
    }

    pub async fn delete_tag(&self, id: &str) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM tag_assignments WHERE tag_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM user_tags WHERE tag_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        let res = sqlx::query("DELETE FROM tags WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn tag_ids_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT tag_id FROM tag_assignments \
             WHERE entity_type = ? AND entity_id = ? ORDER BY created_at ASC",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|row| row.get("tag_id")).collect())
    }

    pub async fn set_entity_tags(
        &self,
        entity_type: &str,
        entity_id: &str,
        tag_ids: &[String],
    ) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query(
            "DELETE FROM tag_assignments WHERE entity_type = ? AND entity_id = ?",
        )
        .bind(entity_type)
        .bind(entity_id)
        .execute(&self.pool)
        .await?;

        let now = Utc::now().to_rfc3339();
        for tag_id in tag_ids {
            sqlx::query(
                "INSERT INTO tag_assignments (tag_id, entity_type, entity_id, created_at) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(tag_id)
            .bind(entity_type)
            .bind(entity_id)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        self.tag_ids_for_entity(entity_type, entity_id).await
    }
}
