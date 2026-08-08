use chrono::Utc;
use conductor_domain::{CreateSubRoleRequest, SubRole};
use sqlx::{Any, Pool};
use sqlx::Row;
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
}
