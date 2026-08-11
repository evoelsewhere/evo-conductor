use chrono::Utc;
use conductor_domain::{
    validate_analytics_view_metadata, AnalyticsView, AnalyticsViewDefinition,
    AnalyticsViewVisibility, CreateAnalyticsViewRequest, UpdateAnalyticsViewRequest,
};
use sqlx::{Any, Pool, Row};
use thiserror::Error;
use uuid::Uuid;

use crate::core::mapping::parse_dt;

#[derive(Debug, Error)]
pub enum AnalyticsViewWriteError {
    #[error("analytics view not found")]
    NotFound,
    #[error("analytics view can only be changed by its owner or a project admin")]
    Forbidden,
    #[error("analytics view revision conflict; current revision is {current_revision}")]
    RevisionConflict { current_revision: u64 },
    #[error("an analytics view with this name already exists for the owner")]
    NameConflict,
    #[error("invalid analytics view: {0}")]
    Validation(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct AnalyticsViewRepo {
    pool: Pool<Any>,
}

impl AnalyticsViewRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    /// Return views the actor can read in one project. Private views are
    /// owner-only, except that project admins may audit them.
    pub async fn list_accessible(
        &self,
        project_id: Uuid,
        actor_id: Uuid,
        include_all_private: bool,
    ) -> Result<Vec<AnalyticsView>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, owner_user_id, name, description, visibility,
                   definition, revision, created_at, updated_at
            FROM analytics_views
            WHERE project_id = ?
              AND (owner_user_id = ? OR visibility = 'shared' OR ? = 1)
            ORDER BY updated_at DESC, name ASC
            "#,
        )
        .bind(project_id.to_string())
        .bind(actor_id.to_string())
        .bind(i64::from(include_all_private))
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_view).collect()
    }

    pub async fn find_accessible(
        &self,
        project_id: Uuid,
        view_id: Uuid,
        actor_id: Uuid,
        include_all_private: bool,
    ) -> Result<Option<AnalyticsView>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, owner_user_id, name, description, visibility,
                   definition, revision, created_at, updated_at
            FROM analytics_views
            WHERE id = ? AND project_id = ?
              AND (owner_user_id = ? OR visibility = 'shared' OR ? = 1)
            "#,
        )
        .bind(view_id.to_string())
        .bind(project_id.to_string())
        .bind(actor_id.to_string())
        .bind(i64::from(include_all_private))
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_view).transpose()
    }

    pub async fn create(
        &self,
        project_id: Uuid,
        owner_user_id: Uuid,
        request: &CreateAnalyticsViewRequest,
    ) -> Result<AnalyticsView, AnalyticsViewWriteError> {
        validate_request(
            &request.name,
            request.description.as_deref(),
            &request.definition,
        )?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let definition = encode_definition(&request.definition)?;
        let result = sqlx::query(
            r#"
            INSERT INTO analytics_views (
                id, project_id, owner_user_id, name, name_key, description,
                visibility, definition, revision, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(owner_user_id.to_string())
        .bind(&request.name)
        .bind(name_key(&request.name))
        .bind(&request.description)
        .bind(request.visibility.as_str())
        .bind(definition)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await;
        if let Err(error) = result {
            return Err(map_write_database_error(error));
        }

        Ok(AnalyticsView {
            id,
            project_id,
            owner_user_id,
            name: request.name.clone(),
            description: request.description.clone(),
            visibility: request.visibility,
            definition: request.definition.clone(),
            revision: 1,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn update(
        &self,
        project_id: Uuid,
        view_id: Uuid,
        actor_id: Uuid,
        actor_can_manage_all: bool,
        request: &UpdateAnalyticsViewRequest,
    ) -> Result<AnalyticsView, AnalyticsViewWriteError> {
        validate_request(
            &request.name,
            request.description.as_deref(),
            &request.definition,
        )?;
        if request.revision == 0 {
            return Err(AnalyticsViewWriteError::Validation(
                "revision must be at least 1".into(),
            ));
        }

        let mut tx = self.pool.begin().await?;
        let current = sqlx::query(
            "SELECT owner_user_id, revision FROM analytics_views WHERE id = ? AND project_id = ?",
        )
        .bind(view_id.to_string())
        .bind(project_id.to_string())
        .fetch_optional(&mut *tx)
        .await?;
        let Some(current) = current else {
            return Err(AnalyticsViewWriteError::NotFound);
        };
        let owner_user_id: String = current.get("owner_user_id");
        let current_revision = current.get::<i64, _>("revision").max(1) as u64;
        if owner_user_id != actor_id.to_string() && !actor_can_manage_all {
            return Err(AnalyticsViewWriteError::Forbidden);
        }
        if current_revision != request.revision {
            return Err(AnalyticsViewWriteError::RevisionConflict { current_revision });
        }

        let definition = encode_definition(&request.definition)?;
        let now = Utc::now();
        let next_revision = current_revision.saturating_add(1);
        let result = sqlx::query(
            r#"
            UPDATE analytics_views
            SET name = ?, name_key = ?, description = ?, visibility = ?, definition = ?,
                revision = ?, updated_at = ?
            WHERE id = ? AND project_id = ? AND revision = ?
            "#,
        )
        .bind(&request.name)
        .bind(name_key(&request.name))
        .bind(&request.description)
        .bind(request.visibility.as_str())
        .bind(definition)
        .bind(next_revision as i64)
        .bind(now.to_rfc3339())
        .bind(view_id.to_string())
        .bind(project_id.to_string())
        .bind(current_revision as i64)
        .execute(&mut *tx)
        .await;
        let result = match result {
            Ok(result) => result,
            Err(error) => return Err(map_write_database_error(error)),
        };
        if result.rows_affected() == 0 {
            return Err(AnalyticsViewWriteError::RevisionConflict { current_revision });
        }

        let row = sqlx::query(
            r#"
            SELECT id, project_id, owner_user_id, name, description, visibility,
                   definition, revision, created_at, updated_at
            FROM analytics_views WHERE id = ? AND project_id = ?
            "#,
        )
        .bind(view_id.to_string())
        .bind(project_id.to_string())
        .fetch_one(&mut *tx)
        .await?;
        let view = map_view(row)?;
        tx.commit().await?;
        Ok(view)
    }

    pub async fn delete(
        &self,
        project_id: Uuid,
        view_id: Uuid,
        actor_id: Uuid,
        actor_can_manage_all: bool,
        expected_revision: u64,
    ) -> Result<(), AnalyticsViewWriteError> {
        if expected_revision == 0 {
            return Err(AnalyticsViewWriteError::Validation(
                "revision must be at least 1".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query(
            "SELECT owner_user_id, revision FROM analytics_views WHERE id = ? AND project_id = ?",
        )
        .bind(view_id.to_string())
        .bind(project_id.to_string())
        .fetch_optional(&mut *tx)
        .await?;
        let Some(current) = current else {
            return Err(AnalyticsViewWriteError::NotFound);
        };
        let owner_user_id: String = current.get("owner_user_id");
        let current_revision = current.get::<i64, _>("revision").max(1) as u64;
        if owner_user_id != actor_id.to_string() && !actor_can_manage_all {
            return Err(AnalyticsViewWriteError::Forbidden);
        }
        if current_revision != expected_revision {
            return Err(AnalyticsViewWriteError::RevisionConflict { current_revision });
        }

        let result = sqlx::query(
            "DELETE FROM analytics_views WHERE id = ? AND project_id = ? AND revision = ?",
        )
        .bind(view_id.to_string())
        .bind(project_id.to_string())
        .bind(expected_revision as i64)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AnalyticsViewWriteError::RevisionConflict { current_revision });
        }
        tx.commit().await?;
        Ok(())
    }
}

fn validate_request(
    name: &str,
    description: Option<&str>,
    definition: &AnalyticsViewDefinition,
) -> Result<(), AnalyticsViewWriteError> {
    validate_analytics_view_metadata(name, description)
        .map_err(AnalyticsViewWriteError::Validation)?;
    definition
        .validate()
        .map_err(AnalyticsViewWriteError::Validation)
}

fn name_key(value: &str) -> String {
    value.to_lowercase()
}

fn encode_definition(
    definition: &AnalyticsViewDefinition,
) -> Result<String, AnalyticsViewWriteError> {
    serde_json::to_string(definition)
        .map_err(|error| AnalyticsViewWriteError::Database(sqlx::Error::Encode(Box::new(error))))
}

fn map_write_database_error(error: sqlx::Error) -> AnalyticsViewWriteError {
    if matches!(&error, sqlx::Error::Database(database) if database.is_unique_violation()) {
        AnalyticsViewWriteError::NameConflict
    } else {
        AnalyticsViewWriteError::Database(error)
    }
}

fn map_view(row: sqlx::any::AnyRow) -> Result<AnalyticsView, sqlx::Error> {
    let definition =
        serde_json::from_str::<AnalyticsViewDefinition>(&row.get::<String, _>("definition"))
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    // Reject corrupted or legacy data rather than returning executable-looking
    // unvalidated JSON to the console.
    definition
        .validate()
        .map_err(|error| sqlx::Error::Decode(error.into()))?;

    Ok(AnalyticsView {
        id: parse_uuid(&row, "id")?,
        project_id: parse_uuid(&row, "project_id")?,
        owner_user_id: parse_uuid(&row, "owner_user_id")?,
        name: row.get("name"),
        description: row.get("description"),
        visibility: AnalyticsViewVisibility::parse(row.get::<String, _>("visibility").as_str()),
        definition,
        revision: row.get::<i64, _>("revision").max(1) as u64,
        created_at: parse_dt(row.get("created_at")),
        updated_at: parse_dt(row.get("updated_at")),
    })
}

fn parse_uuid(row: &sqlx::any::AnyRow, column: &str) -> Result<Uuid, sqlx::Error> {
    let value: String = row.get(column);
    Uuid::parse_str(&value).map_err(|error| sqlx::Error::Decode(Box::new(error)))
}
