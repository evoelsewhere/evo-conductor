use conductor_domain::ManagedResource;
use sqlx::{Any, Pool};
use uuid::Uuid;

use crate::core::mapping::map_resource;

#[derive(Clone)]
pub struct ResourceRepo {
    pool: Pool<Any>,
}

impl ResourceRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn list_visible_to(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ManagedResource>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, kind, slug, name, description, version, owner_user_id,
                   visibility, payload, created_at, updated_at
            FROM resources
            WHERE visibility = 'shared' OR owner_user_id = ?
            ORDER BY kind, name
            "#,
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| map_resource(&r).ok())
            .collect())
    }
}
