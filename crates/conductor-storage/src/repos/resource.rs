use std::collections::{BTreeMap, HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use conductor_domain::{
    CreateResourceRequest, EffectiveResourceVersion, ManagedResource, PrimaryRole, ReleaseChannel,
    ReleaseResourceRequest, ReleaseResourceResult, ResourceAccessPolicy, ResourceBundleV2,
    ResourceDailyUsage, ResourceFeedback, ResourceInstallationState, ResourceInventoryMonitoring,
    ResourceInventoryMonitoringSummary, ResourceInventoryObservedState, ResourceInventoryRequest,
    ResourceMemberUsage, ResourceMonitoring, ResourceMonitoringSummary, ResourceUsageEventRequest,
    ResourceVersion, ResourceVersionLifecycleAction, ResourceVersionNotice, ResourceVersionStatus,
    SemanticVersion, UpdateResourceRequest, UpsertResourceFeedbackRequest, VersionMode,
};
use sqlx::{Any, Pool, QueryBuilder, Row};
use uuid::Uuid;

use crate::core::mapping::{map_resource, parse_dt};

#[derive(Clone)]
pub struct ResourceRepo {
    pool: Pool<Any>,
}

#[derive(Debug, Clone)]
pub struct ReleaseContent {
    pub sha256: String,
    pub size: u64,
    pub artifact_key: Option<String>,
    pub updated_payload: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DraftContent {
    pub artifact_key: String,
    pub sha256: String,
    pub size: u64,
    pub metadata_payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct DraftArtifact {
    pub resource_id: Uuid,
    pub revision: u64,
    pub artifact_key: String,
    pub sha256: String,
    pub size: u64,
    pub metadata_payload: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum DraftWriteError {
    #[error("resource was not found")]
    NotFound,
    #[error("draft revision is stale")]
    Conflict,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ReleaseResourceError {
    #[error("resource was not found")]
    NotFound,
    #[error("draft or version head changed")]
    Conflict,
    #[error("semantic version is invalid or not greater than the current head")]
    InvalidVersion,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceVersionLifecycleError {
    #[error("resource version was not found")]
    NotFound,
    #[error("resource is archived")]
    ResourceArchived,
    #[error("active release versions cannot be deprecated")]
    ActiveRelease,
    #[error("resource version is already deprecated")]
    AlreadyDeprecated,
    #[error("only immutable release versions support lifecycle actions")]
    NotReleased,
    #[error("deprecated version confirmation is required")]
    DeprecatedConfirmationRequired,
    #[error("draft revision is stale")]
    DraftConflict,
    #[error("resource version has no restorable source")]
    InvalidSource,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl ResourceRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
    }

    pub async fn object_keys(&self) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            SELECT draft_artifact_key FROM resources
            WHERE draft_artifact_key IS NOT NULL AND draft_artifact_key <> ''
            UNION
            SELECT artifact_key FROM resource_versions
            WHERE artifact_key IS NOT NULL AND artifact_key <> ''
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_for_actor(
        &self,
        user_id: Uuid,
        role: PrimaryRole,
    ) -> Result<Vec<ManagedResource>, sqlx::Error> {
        if role == PrimaryRole::Admin {
            return self.list_all().await;
        }
        if role == PrimaryRole::User {
            return self.list_visible_to(user_id).await;
        }

        let rows = sqlx::query(
            r#"
            SELECT r.id, r.project_id, r.kind, r.slug, r.name, r.description, r.version,
                   r.highest_semver, r.draft_revision, r.release_channel,
                   r.owner_user_id, r.visibility, r.status, r.payload,
                   r.published_at, r.created_at, r.updated_at
            FROM resources r
            WHERE r.owner_user_id = ? OR (
                r.status = 'published' AND (
                    (SELECT primary_role FROM users WHERE id = ?) = 'admin'
                    OR (r.visibility = 'shared' AND NOT EXISTS (
                        SELECT 1 FROM resource_access_rules a WHERE a.resource_id = r.id
                    ))
                    OR EXISTS (
                        SELECT 1 FROM resource_access_rules a
                        WHERE a.resource_id = r.id AND (
                            a.subject_type = 'all'
                            OR (a.subject_type = 'member' AND a.subject_id = ?)
                            OR (a.subject_type = 'primary_role' AND a.subject_id = (
                                SELECT primary_role FROM users WHERE id = ?
                            ))
                            OR (a.subject_type = 'sub_role' AND a.subject_id IN (
                                SELECT sub_role_id FROM user_sub_roles WHERE user_id = ?
                            ))
                            OR (a.subject_type = 'tag' AND a.subject_id IN (
                                SELECT tag_id FROM user_tags WHERE user_id = ?
                            ))
                        )
                    )
                )
            )
            ORDER BY r.updated_at DESC, r.name
            "#,
        )
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(map_resources(rows))
    }

    pub async fn list_all(&self) -> Result<Vec<ManagedResource>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, kind, slug, name, description, version,
                   highest_semver, draft_revision, release_channel, owner_user_id,
                   visibility, status, payload, published_at, created_at, updated_at
            FROM resources
            ORDER BY updated_at DESC, name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(map_resources(rows))
    }

    pub async fn list_visible_to(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ManagedResource>, sqlx::Error> {
        let rows = sqlx::query(&format!(
            "{} ORDER BY r.kind, r.name",
            visible_resources_query("SELECT r.id, r.project_id, r.kind, r.slug, r.name, r.description, r.version, r.highest_semver, r.draft_revision, r.release_channel, r.owner_user_id, r.visibility, r.status, r.payload, r.published_at, r.created_at, r.updated_at")
        ))
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(map_resources(rows))
    }

    pub async fn visible_resource_ids(&self, user_id: Uuid) -> Result<HashSet<Uuid>, sqlx::Error> {
        let rows = sqlx::query(&visible_resources_query("SELECT r.id"))
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| Uuid::parse_str(row.get::<String, _>("id").as_str()).ok())
            .collect())
    }

    pub async fn find_by_id(
        &self,
        resource_id: Uuid,
    ) -> Result<Option<ManagedResource>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, kind, slug, name, description, version,
                   highest_semver, draft_revision, release_channel, owner_user_id,
                   visibility, status, payload, published_at, created_at, updated_at
            FROM resources WHERE id = ?
            "#,
        )
        .bind(resource_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| map_resource(&row)).transpose()
    }

    pub async fn version_belongs_to(
        &self,
        resource_id: Uuid,
        version_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM resource_versions WHERE id = ? AND resource_id = ?",
        )
        .bind(version_id.to_string())
        .bind(resource_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn inventory_plugin_matches(
        &self,
        project_id: Uuid,
        installation_id: Uuid,
        resource_id: Uuid,
        version_id: Uuid,
        plugin_installation_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM installation_resource_inventory
            WHERE project_id = ? AND installation_id = ? AND resource_id = ?
              AND applied_version_id = ? AND plugin_installation_id = ?
              AND observed_state IN ('applied', 'in_sync', 'trust_pending')
            "#,
        )
        .bind(project_id.to_string())
        .bind(installation_id.to_string())
        .bind(resource_id.to_string())
        .bind(version_id.to_string())
        .bind(plugin_installation_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn create(
        &self,
        project_id: Uuid,
        request: &CreateResourceRequest,
        owner_user_id: Uuid,
        draft: &DraftContent,
    ) -> Result<ManagedResource, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let payload =
            serde_json::to_string(&draft.metadata_payload).unwrap_or_else(|_| "{}".into());

        sqlx::query(
            r#"
            INSERT INTO resources (
                id, project_id, kind, slug, name, description, version, owner_user_id,
                visibility, status, payload, draft_revision, highest_semver,
                release_channel, published_at, draft_artifact_key,
                draft_content_sha256, draft_content_size, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, 0, NULL, NULL, NULL, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(request.kind.as_str())
        .bind(request.slug.trim())
        .bind(request.name.trim())
        .bind(clean_optional(request.description.as_deref()))
        .bind(request.version.trim())
        .bind(owner_user_id.to_string())
        .bind(request.visibility.as_str())
        .bind(&payload)
        .bind(&draft.artifact_key)
        .bind(&draft.sha256)
        .bind(saturating_i64(draft.size))
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(self.find_by_id(id).await?.expect("created resource"))
    }

    pub async fn update(
        &self,
        resource_id: Uuid,
        request: &UpdateResourceRequest,
    ) -> Result<Option<ManagedResource>, sqlx::Error> {
        let existing = match self.find_by_id(resource_id).await? {
            Some(resource) => resource,
            None => return Ok(None),
        };
        sqlx::query(
            r#"
            UPDATE resources
            SET name = ?, description = ?, visibility = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(
            request
                .name
                .as_deref()
                .map(str::trim)
                .unwrap_or(&existing.name),
        )
        .bind(
            request
                .description
                .as_deref()
                .map(|value| clean_optional(Some(value)))
                .unwrap_or(existing.description),
        )
        .bind(request.visibility.unwrap_or(existing.visibility).as_str())
        .bind(Utc::now().to_rfc3339())
        .bind(resource_id.to_string())
        .execute(&self.pool)
        .await?;
        self.find_by_id(resource_id).await
    }

    pub async fn archive(&self, resource_id: Uuid) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let project_id: Option<String> =
            sqlx::query_scalar("SELECT project_id FROM resources WHERE id = ?")
                .bind(resource_id.to_string())
                .fetch_optional(&mut *tx)
                .await?;
        let Some(project_id) = project_id else {
            return Ok(false);
        };
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE resources SET status = 'archived', updated_at = ? WHERE id = ? AND status <> 'archived'",
        )
        .bind(&now)
        .bind(resource_id.to_string())
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        insert_resource_change(
            &mut tx,
            &project_id,
            resource_id,
            "archive",
            None,
            None,
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn versions(&self, resource_id: Uuid) -> Result<Vec<ResourceVersion>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, resource_id, version, status, payload, changelog,
                   release_channel, content_sha256, content_size, artifact_key,
                   minimum_evoflux_version, created_by, created_at, published_at,
                   deprecated_at, deprecated_by, deprecation_reason,
                   (SELECT channel FROM resource_release_channels active
                    WHERE active.resource_id = resource_versions.resource_id
                      AND active.version_id = resource_versions.id
                    ORDER BY channel LIMIT 1) AS active_channel
            FROM resource_versions
            WHERE resource_id = ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(resource_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(map_version).collect())
    }

    pub async fn deprecate_version(
        &self,
        resource_id: Uuid,
        version_id: Uuid,
        actor_id: Uuid,
        reason: &str,
    ) -> Result<ResourceVersion, ResourceVersionLifecycleError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT rv.project_id, rv.status, r.status AS resource_status,
                   (SELECT COUNT(*) FROM resource_release_channels active
                       WHERE active.resource_id = rv.resource_id
                         AND active.version_id = rv.id
                   ) AS active_count
            FROM resource_versions rv
            JOIN resources r ON r.id = rv.resource_id
            WHERE rv.id = ? AND rv.resource_id = ?
            "#,
        )
        .bind(version_id.to_string())
        .bind(resource_id.to_string())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ResourceVersionLifecycleError::NotFound)?;
        if row.get::<String, _>("resource_status") == "archived" {
            return Err(ResourceVersionLifecycleError::ResourceArchived);
        }
        if row.get::<i64, _>("active_count") != 0 {
            return Err(ResourceVersionLifecycleError::ActiveRelease);
        }
        let status = ResourceVersionStatus::parse(row.get::<String, _>("status").as_str());
        if status == ResourceVersionStatus::Draft {
            return Err(ResourceVersionLifecycleError::NotReleased);
        }
        if status == ResourceVersionStatus::Deprecated {
            return Err(ResourceVersionLifecycleError::AlreadyDeprecated);
        }

        let project_id: String = row.get("project_id");
        let now = Utc::now().to_rfc3339();
        let updated = sqlx::query(
            r#"
            UPDATE resource_versions
            SET status = 'deprecated', deprecated_at = ?, deprecated_by = ?,
                deprecation_reason = ?
            WHERE id = ? AND resource_id = ? AND status <> 'deprecated'
              AND NOT EXISTS (
                  SELECT 1 FROM resource_release_channels active
                  WHERE active.resource_id = resource_versions.resource_id
                    AND active.version_id = resource_versions.id
              )
            "#,
        )
        .bind(&now)
        .bind(actor_id.to_string())
        .bind(reason)
        .bind(version_id.to_string())
        .bind(resource_id.to_string())
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(ResourceVersionLifecycleError::ActiveRelease);
        }
        insert_version_event(
            &mut tx,
            &project_id,
            resource_id,
            version_id,
            ResourceVersionLifecycleAction::Deprecate.as_str(),
            actor_id,
            Some(reason),
            false,
            &now,
        )
        .await?;
        insert_resource_change(
            &mut tx,
            &project_id,
            resource_id,
            "deprecate",
            Some(version_id),
            None,
            &now,
        )
        .await?;
        tx.commit().await?;
        self.version_by_id(resource_id, version_id)
            .await?
            .ok_or(ResourceVersionLifecycleError::NotFound)
    }

    pub async fn restore_version_to_draft(
        &self,
        resource_id: Uuid,
        version_id: Uuid,
        actor_id: Uuid,
        expected_revision: u64,
        confirm_deprecated: bool,
    ) -> Result<DraftArtifact, ResourceVersionLifecycleError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT rv.project_id, rv.status, rv.payload, rv.artifact_key,
                   rv.content_sha256, rv.content_size,
                   r.status AS resource_status, r.draft_revision
            FROM resource_versions rv
            JOIN resources r ON r.id = rv.resource_id
            WHERE rv.id = ? AND rv.resource_id = ?
            "#,
        )
        .bind(version_id.to_string())
        .bind(resource_id.to_string())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ResourceVersionLifecycleError::NotFound)?;
        if row.get::<String, _>("resource_status") == "archived" {
            return Err(ResourceVersionLifecycleError::ResourceArchived);
        }
        let status = ResourceVersionStatus::parse(row.get::<String, _>("status").as_str());
        if status == ResourceVersionStatus::Draft {
            return Err(ResourceVersionLifecycleError::NotReleased);
        }
        if status == ResourceVersionStatus::Deprecated && !confirm_deprecated {
            return Err(ResourceVersionLifecycleError::DeprecatedConfirmationRequired);
        }
        let current_revision = nonnegative_u64(row.get("draft_revision"));
        if current_revision != expected_revision {
            return Err(ResourceVersionLifecycleError::DraftConflict);
        }
        let payload_text: String = row.get("payload");
        let payload = serde_json::from_str::<serde_json::Value>(&payload_text)
            .map_err(|_| ResourceVersionLifecycleError::InvalidSource)?;
        let artifact_key: String = row
            .get::<Option<String>, _>("artifact_key")
            .filter(|value| !value.is_empty())
            .ok_or(ResourceVersionLifecycleError::InvalidSource)?;
        let sha256: String = row.get("content_sha256");
        let size = nonnegative_u64(row.get("content_size"));

        let next_revision = expected_revision.saturating_add(1);
        let now = Utc::now().to_rfc3339();
        let updated = sqlx::query(
            r#"
            UPDATE resources
            SET payload = ?, draft_revision = ?, draft_artifact_key = ?,
                draft_content_sha256 = ?, draft_content_size = ?, updated_at = ?
            WHERE id = ? AND draft_revision = ? AND status <> 'archived'
            "#,
        )
        .bind(&payload_text)
        .bind(saturating_i64(next_revision))
        .bind(&artifact_key)
        .bind(&sha256)
        .bind(saturating_i64(size))
        .bind(&now)
        .bind(resource_id.to_string())
        .bind(saturating_i64(expected_revision))
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(ResourceVersionLifecycleError::DraftConflict);
        }
        let project_id: String = row.get("project_id");
        insert_version_event(
            &mut tx,
            &project_id,
            resource_id,
            version_id,
            ResourceVersionLifecycleAction::RestoreToDraft.as_str(),
            actor_id,
            None,
            status == ResourceVersionStatus::Deprecated,
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(DraftArtifact {
            resource_id,
            revision: next_revision,
            artifact_key,
            sha256,
            size,
            metadata_payload: payload,
        })
    }

    async fn version_by_id(
        &self,
        resource_id: Uuid,
        version_id: Uuid,
    ) -> Result<Option<ResourceVersion>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, resource_id, version, status, payload, changelog,
                   release_channel, content_sha256, content_size, artifact_key,
                   minimum_evoflux_version, created_by, created_at, published_at,
                   deprecated_at, deprecated_by, deprecation_reason,
                   (SELECT channel FROM resource_release_channels active
                    WHERE active.resource_id = resource_versions.resource_id
                      AND active.version_id = resource_versions.id
                    ORDER BY channel LIMIT 1) AS active_channel
            FROM resource_versions
            WHERE id = ? AND resource_id = ?
            "#,
        )
        .bind(version_id.to_string())
        .bind(resource_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(map_version))
    }

    pub async fn access_policy(
        &self,
        resource_id: Uuid,
    ) -> Result<ResourceAccessPolicy, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT subject_type, subject_id FROM resource_access_rules WHERE resource_id = ?",
        )
        .bind(resource_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        let mut policy = ResourceAccessPolicy::default();
        for row in rows {
            let subject_type: String = row.get("subject_type");
            let subject_id: String = row.get("subject_id");
            match subject_type.as_str() {
                "all" => policy.all_members = true,
                "primary_role" => policy.primary_roles.push(subject_id),
                "sub_role" => policy.sub_role_ids.push(subject_id),
                "tag" => policy.tag_ids.push(subject_id),
                "member" => {
                    if let Ok(id) = Uuid::parse_str(&subject_id) {
                        policy.member_ids.push(id);
                    }
                }
                _ => {}
            }
        }
        Ok(policy)
    }

    pub async fn set_access_policy(
        &self,
        resource_id: Uuid,
        policy: &ResourceAccessPolicy,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM resource_access_rules WHERE resource_id = ?")
            .bind(resource_id.to_string())
            .execute(&mut *tx)
            .await?;
        let now = Utc::now().to_rfc3339();
        if policy.all_members {
            insert_access_rule(&mut tx, resource_id, "all", "*", &now).await?;
        }
        for role in &policy.primary_roles {
            insert_access_rule(&mut tx, resource_id, "primary_role", role, &now).await?;
        }
        for id in &policy.sub_role_ids {
            insert_access_rule(&mut tx, resource_id, "sub_role", id, &now).await?;
        }
        for id in &policy.tag_ids {
            insert_access_rule(&mut tx, resource_id, "tag", id, &now).await?;
        }
        for id in &policy.member_ids {
            insert_access_rule(&mut tx, resource_id, "member", &id.to_string(), &now).await?;
        }
        if let Some(project_id) =
            sqlx::query_scalar::<_, String>("SELECT project_id FROM resources WHERE id = ?")
                .bind(resource_id.to_string())
                .fetch_optional(&mut *tx)
                .await?
        {
            insert_resource_change(
                &mut tx,
                &project_id,
                resource_id,
                "access",
                None,
                None,
                &now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn draft_artifact(
        &self,
        resource_id: Uuid,
    ) -> Result<Option<DraftArtifact>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, draft_revision, draft_artifact_key, draft_content_sha256,
                   draft_content_size, payload
            FROM resources WHERE id = ?
            "#,
        )
        .bind(resource_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|row| {
            Some(DraftArtifact {
                resource_id: parse_uuid(row.get("id")),
                revision: nonnegative_u64(row.get("draft_revision")),
                artifact_key: row.get::<Option<String>, _>("draft_artifact_key")?,
                sha256: row.get("draft_content_sha256"),
                size: nonnegative_u64(row.get("draft_content_size")),
                metadata_payload: serde_json::from_str(row.get::<String, _>("payload").as_str())
                    .unwrap_or_else(|_| serde_json::json!({})),
            })
        }))
    }

    pub async fn replace_draft_artifact(
        &self,
        resource_id: Uuid,
        draft: &DraftContent,
        expected_revision: u64,
    ) -> Result<DraftArtifact, DraftWriteError> {
        let next_revision = expected_revision.saturating_add(1);
        let payload =
            serde_json::to_string(&draft.metadata_payload).unwrap_or_else(|_| "{}".into());
        let updated = sqlx::query(
            "UPDATE resources SET payload = ?, draft_revision = ?, draft_artifact_key = ?, \
             draft_content_sha256 = ?, draft_content_size = ?, updated_at = ? \
             WHERE id = ? AND draft_revision = ? AND status <> 'archived'",
        )
        .bind(&payload)
        .bind(saturating_i64(next_revision))
        .bind(&draft.artifact_key)
        .bind(&draft.sha256)
        .bind(saturating_i64(draft.size))
        .bind(Utc::now().to_rfc3339())
        .bind(resource_id.to_string())
        .bind(saturating_i64(expected_revision))
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() == 0 {
            let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM resources WHERE id = ?")
                .bind(resource_id.to_string())
                .fetch_one(&self.pool)
                .await?;
            return Err(if exists == 0 {
                DraftWriteError::NotFound
            } else {
                DraftWriteError::Conflict
            });
        }
        Ok(DraftArtifact {
            resource_id,
            revision: next_revision,
            artifact_key: draft.artifact_key.clone(),
            sha256: draft.sha256.clone(),
            size: draft.size,
            metadata_payload: draft.metadata_payload.clone(),
        })
    }

    pub async fn release(
        &self,
        resource_id: Uuid,
        request: &ReleaseResourceRequest,
        content: &ReleaseContent,
        actor_id: Uuid,
    ) -> Result<ReleaseResourceResult, ReleaseResourceError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT project_id, draft_revision, highest_semver, payload, updated_at
            FROM resources WHERE id = ? AND status <> 'archived'
            "#,
        )
        .bind(resource_id.to_string())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ReleaseResourceError::NotFound)?;
        let project_id = parse_uuid(row.get("project_id"));
        let draft_revision = nonnegative_u64(row.get("draft_revision"));
        let previous_updated_at: String = row.get("updated_at");
        if draft_revision != request.draft_revision {
            return Err(ReleaseResourceError::Conflict);
        }
        let highest_text: Option<String> = row.get("highest_semver");
        let highest = highest_text
            .as_deref()
            .map(SemanticVersion::from_str)
            .transpose()
            .map_err(|_| ReleaseResourceError::InvalidVersion)?;
        let allocated = match request.version_mode {
            VersionMode::Auto => highest
                .as_ref()
                .map(SemanticVersion::next_patch)
                .unwrap_or_else(SemanticVersion::initial),
            VersionMode::Manual => {
                let value = request
                    .manual_version
                    .as_deref()
                    .ok_or(ReleaseResourceError::InvalidVersion)?;
                let parsed = SemanticVersion::from_str(value)
                    .map_err(|_| ReleaseResourceError::InvalidVersion)?;
                if highest.as_ref().is_some_and(|head| parsed <= *head) {
                    return Err(ReleaseResourceError::InvalidVersion);
                }
                parsed
            }
        };
        let version = allocated.to_string();
        let version_id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let stored_payload: String = row.get("payload");
        let version_payload = content
            .updated_payload
            .as_deref()
            .unwrap_or(&stored_payload);

        sqlx::query(
            r#"
            INSERT INTO resource_versions (
                id, project_id, resource_id, version, status, payload, changelog,
                release_channel, content_sha256, content_size, artifact_key,
                artifact_schema_version, minimum_evoflux_version, created_by,
                created_at, published_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '1', ?, ?, ?, ?)
            "#,
        )
        .bind(version_id.to_string())
        .bind(project_id.to_string())
        .bind(resource_id.to_string())
        .bind(&version)
        .bind(request.channel.as_str())
        .bind(version_payload)
        .bind(clean_optional(request.changelog.as_deref()))
        .bind(request.channel.as_str())
        .bind(&content.sha256)
        .bind(saturating_i64(content.size))
        .bind(content.artifact_key.as_deref())
        .bind(clean_optional(request.minimum_evoflux_version.as_deref()))
        .bind(actor_id.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|error| match &error {
            sqlx::Error::Database(database) if database.is_unique_violation() => {
                ReleaseResourceError::Conflict
            }
            _ => ReleaseResourceError::Database(error),
        })?;

        sqlx::query(
            "DELETE FROM resource_release_channels WHERE project_id = ? AND resource_id = ? AND channel = ?",
        )
        .bind(project_id.to_string())
        .bind(resource_id.to_string())
        .bind(request.channel.as_str())
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO resource_release_channels (
                project_id, resource_id, channel, version_id, updated_by, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(project_id.to_string())
        .bind(resource_id.to_string())
        .bind(request.channel.as_str())
        .bind(version_id.to_string())
        .bind(actor_id.to_string())
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        let previous_beta_members = if request.channel == ReleaseChannel::Beta {
            sqlx::query_scalar::<_, String>(
                "SELECT user_id FROM resource_beta_members WHERE project_id = ? AND resource_id = ?",
            )
            .bind(project_id.to_string())
            .bind(resource_id.to_string())
            .fetch_all(&mut *tx)
            .await?
        } else {
            Vec::new()
        };
        if request.channel == ReleaseChannel::Beta {
            sqlx::query(
                "DELETE FROM resource_beta_members WHERE project_id = ? AND resource_id = ?",
            )
            .bind(project_id.to_string())
            .bind(resource_id.to_string())
            .execute(&mut *tx)
            .await?;
            for member_id in &request.beta_member_ids {
                sqlx::query(
                    r#"
                    INSERT INTO resource_beta_members (
                        project_id, resource_id, user_id, assigned_by, assigned_at
                    ) VALUES (?, ?, ?, ?, ?)
                    "#,
                )
                .bind(project_id.to_string())
                .bind(resource_id.to_string())
                .bind(member_id.to_string())
                .bind(actor_id.to_string())
                .bind(&now)
                .execute(&mut *tx)
                .await?;
            }
        }

        let has_published: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM resource_release_channels WHERE project_id = ? AND resource_id = ? AND channel = 'published'",
        )
        .bind(project_id.to_string())
        .bind(resource_id.to_string())
        .fetch_one(&mut *tx)
        .await?;
        let status = if has_published > 0 {
            "published"
        } else {
            request.channel.as_str()
        };
        let updated = sqlx::query(
            r#"
            UPDATE resources
            SET version = ?, highest_semver = ?, release_channel = ?, status = ?,
                payload = ?, draft_revision = ?, published_at = ?, updated_at = ?
            WHERE id = ? AND draft_revision = ? AND updated_at = ?
            "#,
        )
        .bind(&version)
        .bind(&version)
        .bind(request.channel.as_str())
        .bind(status)
        .bind(version_payload)
        .bind(saturating_i64(if content.updated_payload.is_some() {
            draft_revision.saturating_add(1)
        } else {
            draft_revision
        }))
        .bind(&now)
        .bind(&now)
        .bind(resource_id.to_string())
        .bind(saturating_i64(draft_revision))
        .bind(previous_updated_at)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(ReleaseResourceError::Conflict);
        }

        let mut next_sequence: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(sequence), 0) + 1 FROM resource_changes")
                .fetch_one(&mut *tx)
                .await?;
        let effective_users = if request.channel == ReleaseChannel::Beta {
            let mut users = previous_beta_members.into_iter().collect::<HashSet<_>>();
            users.extend(request.beta_member_ids.iter().map(Uuid::to_string));
            users.into_iter().map(Some).collect::<Vec<_>>()
        } else {
            vec![None]
        };
        for effective_user_id in effective_users {
            sqlx::query(
                r#"
                INSERT INTO resource_changes (
                    sequence, project_id, resource_id, effective_user_id,
                    change_kind, version_id, channel, created_at
                ) VALUES (?, ?, ?, ?, 'release', ?, ?, ?)
                "#,
            )
            .bind(next_sequence)
            .bind(project_id.to_string())
            .bind(resource_id.to_string())
            .bind(effective_user_id)
            .bind(version_id.to_string())
            .bind(request.channel.as_str())
            .bind(&now)
            .execute(&mut *tx)
            .await?;
            next_sequence = next_sequence.saturating_add(1);
        }
        tx.commit().await?;

        Ok(ReleaseResourceResult {
            resource_id,
            version_id,
            version: version.clone(),
            channel: request.channel,
            sha256: content.sha256.clone(),
            size: content.size,
            highest_version: version,
            next_version: allocated.next_patch().to_string(),
        })
    }

    pub async fn effective_version(
        &self,
        resource_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<EffectiveResourceVersion>, sqlx::Error> {
        if !self
            .visible_resource_ids(user_id)
            .await?
            .contains(&resource_id)
        {
            return Ok(None);
        }
        let beta: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM resource_beta_members WHERE resource_id = ? AND user_id = ?",
        )
        .bind(resource_id.to_string())
        .bind(user_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        let preferred = if beta > 0 { "beta" } else { "published" };
        let row = sqlx::query(
            r#"
            SELECT r.project_id, r.id AS resource_id, r.kind, r.slug, r.description,
                   rv.id AS version_id, rv.version, rv.changelog, rv.release_channel, rv.payload,
                   rv.content_sha256, rv.content_size, rv.artifact_key,
                   rv.minimum_evoflux_version
            FROM resources r
            JOIN resource_release_channels c ON c.resource_id = r.id AND c.channel = ?
            JOIN resource_versions rv ON rv.id = c.version_id
            WHERE r.id = ? AND r.status <> 'archived'
            "#,
        )
        .bind(preferred)
        .bind(resource_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        let row = if row.is_none() && preferred == "beta" {
            sqlx::query(
                r#"
                SELECT r.project_id, r.id AS resource_id, r.kind, r.slug, r.description,
                       rv.id AS version_id, rv.version, rv.changelog, rv.release_channel, rv.payload,
                       rv.content_sha256, rv.content_size, rv.artifact_key,
                       rv.minimum_evoflux_version
                FROM resources r
                JOIN resource_release_channels c ON c.resource_id = r.id AND c.channel = 'published'
                JOIN resource_versions rv ON rv.id = c.version_id
                WHERE r.id = ? AND r.status <> 'archived'
                "#,
            )
            .bind(resource_id.to_string())
            .fetch_optional(&self.pool)
            .await?
        } else {
            row
        };
        let mut version = row.map(map_effective_version);
        if let Some(version) = version.as_mut() {
            let allowed_channel = if preferred == "beta" {
                "beta"
            } else {
                "published"
            };
            let rows = sqlx::query(
                r#"
                SELECT id, version, status, release_channel, changelog,
                       published_at, deprecation_reason
                FROM resource_versions
                WHERE resource_id = ? AND status <> 'draft'
                  AND (release_channel = 'published' OR release_channel = ?)
                ORDER BY created_at ASC
                "#,
            )
            .bind(resource_id.to_string())
            .bind(allowed_channel)
            .fetch_all(&self.pool)
            .await?;
            version.version_history = rows.into_iter().filter_map(map_version_notice).collect();
        }
        Ok(version)
    }

    pub async fn change_sequences(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        after: i64,
        limit: u32,
    ) -> Result<Vec<(i64, Uuid)>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT sequence, resource_id FROM resource_changes
            WHERE project_id = ? AND sequence > ?
              AND (effective_user_id IS NULL OR effective_user_id = ?)
            ORDER BY sequence ASC
            LIMIT ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(after)
        .bind(user_id.to_string())
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let resource_id =
                    Uuid::parse_str(row.get::<String, _>("resource_id").as_str()).ok()?;
                Some((row.get("sequence"), resource_id))
            })
            .collect())
    }

    pub async fn upsert_inventory(
        &self,
        project_id: Uuid,
        request: &ResourceInventoryRequest,
    ) -> Result<u32, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let mut accepted = 0_u32;
        for item in &request.items {
            sqlx::query(
                "DELETE FROM installation_resource_inventory WHERE project_id = ? AND installation_id = ? AND resource_id = ?",
            )
            .bind(project_id.to_string())
            .bind(request.installation_id.to_string())
            .bind(item.resource_id.to_string())
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO installation_resource_inventory (
                    project_id, installation_id, resource_id, desired_version_id,
                    applied_version_id, release_channel, content_sha256,
                    plugin_installation_id, observed_state, error_category, observed_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(project_id.to_string())
            .bind(request.installation_id.to_string())
            .bind(item.resource_id.to_string())
            .bind(item.desired_version_id.map(|value| value.to_string()))
            .bind(item.applied_version_id.map(|value| value.to_string()))
            .bind(item.release_channel.map(ReleaseChannel::as_str))
            .bind(item.content_sha256.as_deref())
            .bind(item.plugin_installation_id.as_deref())
            .bind(item.observed_state.trim())
            .bind(item.error_category.as_deref())
            .bind(item.observed_at.to_rfc3339())
            .execute(&mut *tx)
            .await?;
            accepted = accepted.saturating_add(1);
        }
        tx.commit().await?;
        Ok(accepted)
    }

    pub async fn insert_usage_event(
        &self,
        user_id: Uuid,
        event: &ResourceUsageEventRequest,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO resource_usage_events (
                event_id, resource_id, resource_version, user_id, session_id,
                outcome, duration_ms, tokens_in, tokens_out, occurred_at, received_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(event.event_id.to_string())
        .bind(event.resource_id.to_string())
        .bind(event.resource_version.trim())
        .bind(user_id.to_string())
        .bind(clean_optional(event.session_id.as_deref()))
        .bind(event.outcome.as_str())
        .bind(saturating_i64(event.duration_ms))
        .bind(saturating_i64(event.tokens_in))
        .bind(saturating_i64(event.tokens_out))
        .bind(event.occurred_at.to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => Ok(true),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub async fn existing_versions(
        &self,
        requested: &HashSet<(Uuid, String)>,
    ) -> Result<HashSet<(Uuid, String)>, sqlx::Error> {
        if requested.is_empty() {
            return Ok(HashSet::new());
        }
        let mut query =
            QueryBuilder::<Any>::new("SELECT resource_id, version FROM resource_versions WHERE ");
        let mut first = true;
        for (resource_id, version) in requested {
            if !first {
                query.push(" OR ");
            }
            first = false;
            query
                .push("(resource_id = ")
                .push_bind(resource_id.to_string())
                .push(" AND version = ")
                .push_bind(version.clone())
                .push(")");
        }
        let rows = query.build().fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let resource_id =
                    Uuid::parse_str(row.get::<String, _>("resource_id").as_str()).ok()?;
                Some((resource_id, row.get("version")))
            })
            .collect())
    }

    pub async fn monitoring(
        &self,
        resource_id: Uuid,
        days: u32,
    ) -> Result<ResourceMonitoring, sqlx::Error> {
        let since = Utc::now() - Duration::days(i64::from(days));
        let rows = sqlx::query(
            r#"
            SELECT e.user_id, u.display_name, e.outcome, e.duration_ms,
                   e.tokens_in, e.tokens_out, e.occurred_at
            FROM resource_usage_events e
            JOIN users u ON u.id = e.user_id
            WHERE e.resource_id = ? AND e.occurred_at >= ?
            ORDER BY e.occurred_at
            "#,
        )
        .bind(resource_id.to_string())
        .bind(since.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        let mut summary = ResourceMonitoringSummary::default();
        let mut daily: BTreeMap<String, UsageAccumulator> = BTreeMap::new();
        let mut members: HashMap<Uuid, MemberAccumulator> = HashMap::new();
        for row in rows {
            let occurred_at = parse_dt(row.get("occurred_at"));
            let date = occurred_at.format("%Y-%m-%d").to_string();
            let outcome: String = row.get("outcome");
            let duration_ms = nonnegative_u64(row.get("duration_ms"));
            let tokens_in = nonnegative_u64(row.get("tokens_in"));
            let tokens_out = nonnegative_u64(row.get("tokens_out"));
            let user_id = Uuid::parse_str(row.get::<String, _>("user_id").as_str())
                .unwrap_or_else(|_| Uuid::nil());
            let member_name: String = row.get("display_name");

            summary.executions += 1;
            summary.tokens_in += tokens_in;
            summary.tokens_out += tokens_out;
            if outcome == "success" {
                summary.successes += 1;
            } else if outcome == "failure" {
                summary.failures += 1;
            }

            daily.entry(date).or_default().add(&outcome, duration_ms);
            members
                .entry(user_id)
                .or_insert_with(|| MemberAccumulator::new(member_name, occurred_at))
                .add(&outcome, duration_ms, occurred_at);
        }

        let total_duration: u64 = daily.values().map(|point| point.total_duration_ms).sum();
        summary.active_members = members.len().try_into().unwrap_or(u32::MAX);
        summary.success_rate = percentage(summary.successes, summary.executions);
        summary.average_duration_ms = average(total_duration, summary.executions);

        let feedback: (i64, Option<f64>) = sqlx::query_as(
            "SELECT COUNT(*), AVG(rating) FROM resource_feedback WHERE resource_id = ?",
        )
        .bind(resource_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        summary.feedback_count = feedback.0.try_into().unwrap_or(u32::MAX);
        summary.average_rating = feedback.1.map(|value| (value * 10.0).round() / 10.0);

        let daily = daily
            .into_iter()
            .map(|(date, point)| ResourceDailyUsage {
                date,
                executions: point.executions,
                successes: point.successes,
                failures: point.failures,
                average_duration_ms: average(point.total_duration_ms, point.executions),
            })
            .collect();
        let mut members: Vec<_> = members
            .into_iter()
            .map(|(user_id, member)| ResourceMemberUsage {
                user_id,
                member_name: member.member_name,
                executions: member.executions,
                success_rate: percentage(member.successes, member.executions),
                average_duration_ms: average(member.total_duration_ms, member.executions),
                last_used_at: member.last_used_at,
            })
            .collect();
        members.sort_by_key(|member| std::cmp::Reverse(member.executions));

        Ok(ResourceMonitoring {
            resource_id,
            days,
            summary,
            daily,
            members,
        })
    }

    pub async fn inventory_monitoring(
        &self,
        resource_id: Uuid,
    ) -> Result<ResourceInventoryMonitoring, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT i.installation_id, c.display_name AS installation_name, c.platform,
                   c.evoflux_version, c.user_id, c.last_seen_at,
                   u.display_name AS member_name, u.email, u.primary_role,
                   i.desired_version_id, desired.version AS desired_version,
                   i.applied_version_id, applied.version AS applied_version,
                   i.release_channel, i.plugin_installation_id, i.observed_state,
                   i.error_category, i.observed_at
            FROM installation_resource_inventory i
            JOIN client_installations c ON c.id = i.installation_id
            JOIN users u ON u.id = c.user_id
            LEFT JOIN resource_versions desired ON desired.id = i.desired_version_id
            LEFT JOIN resource_versions applied ON applied.id = i.applied_version_id
            WHERE i.resource_id = ? AND i.observed_state <> ?
            ORDER BY i.observed_at DESC, c.display_name
            "#,
        )
        .bind(resource_id.to_string())
        .bind(ResourceInventoryObservedState::Removed.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut summary = ResourceInventoryMonitoringSummary::default();
        let mut installed_installations = HashSet::new();
        let mut installed_members = HashSet::new();
        let mut installations = Vec::with_capacity(rows.len());
        for row in rows {
            summary.reported_installations += 1;
            let observed_state: String = row.get("observed_state");
            let installation_id = parse_uuid(row.get("installation_id"));
            let user_id = parse_uuid(row.get("user_id"));
            if ResourceInventoryObservedState::INSTALLED
                .iter()
                .any(|candidate| candidate.as_str() == observed_state)
            {
                installed_installations.insert(installation_id);
                installed_members.insert(user_id);
            } else if ResourceInventoryObservedState::PENDING
                .iter()
                .any(|candidate| candidate.as_str() == observed_state)
            {
                summary.pending_installations += 1;
            } else {
                summary.attention_installations += 1;
            }
            let primary_role = PrimaryRole::parse(row.get::<String, _>("primary_role").as_str())
                .unwrap_or(PrimaryRole::User);
            installations.push(ResourceInstallationState {
                installation_id,
                installation_name: row.get("installation_name"),
                platform: row.get("platform"),
                evoflux_version: row.get("evoflux_version"),
                user_id,
                member_name: row.get("member_name"),
                email: row.get("email"),
                primary_role,
                desired_version_id: optional_uuid(row.get("desired_version_id")),
                desired_version: row.get("desired_version"),
                applied_version_id: optional_uuid(row.get("applied_version_id")),
                applied_version: row.get("applied_version"),
                release_channel: row
                    .get::<Option<String>, _>("release_channel")
                    .as_deref()
                    .and_then(ReleaseChannel::parse),
                plugin_installation_id: row.get("plugin_installation_id"),
                observed_state,
                error_category: row.get("error_category"),
                observed_at: parse_dt(row.get("observed_at")),
                last_seen_at: parse_dt(row.get("last_seen_at")),
            });
        }
        summary.installed_installations = installed_installations.len() as u64;
        summary.installed_members = installed_members.len() as u64;
        Ok(ResourceInventoryMonitoring {
            resource_id,
            summary,
            installations,
        })
    }

    pub async fn feedback(&self, resource_id: Uuid) -> Result<Vec<ResourceFeedback>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT f.id, f.resource_id, f.resource_version, f.user_id,
                   u.display_name, f.rating, f.comment, f.created_at, f.updated_at
            FROM resource_feedback f
            JOIN users u ON u.id = f.user_id
            WHERE f.resource_id = ?
            ORDER BY f.updated_at DESC
            "#,
        )
        .bind(resource_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(map_feedback).collect())
    }

    pub async fn upsert_feedback(
        &self,
        resource: &ManagedResource,
        user_id: Uuid,
        request: &UpsertResourceFeedbackRequest,
    ) -> Result<ResourceFeedback, sqlx::Error> {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM resource_feedback WHERE resource_id = ? AND user_id = ?",
        )
        .bind(resource.id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        let now = Utc::now().to_rfc3339();
        let id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM resource_feedback WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.pool)
            .await?
            > 0
        {
            sqlx::query(
                r#"
                UPDATE resource_feedback
                SET resource_version = ?, rating = ?, comment = ?, updated_at = ?
                WHERE id = ?
                "#,
            )
            .bind(&resource.version)
            .bind(i64::from(request.rating))
            .bind(clean_optional(request.comment.as_deref()))
            .bind(&now)
            .bind(&id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO resource_feedback (
                    id, resource_id, resource_version, user_id, rating,
                    comment, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&id)
            .bind(resource.id.to_string())
            .bind(&resource.version)
            .bind(user_id.to_string())
            .bind(i64::from(request.rating))
            .bind(clean_optional(request.comment.as_deref()))
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        Ok(self
            .feedback(resource.id)
            .await?
            .into_iter()
            .find(|feedback| feedback.user_id == user_id)
            .expect("upserted feedback"))
    }
}

fn visible_resources_query(select: &str) -> String {
    format!(
        r#"
        {select}
        FROM resources r
        WHERE r.status IN ('beta', 'published') AND (
            r.owner_user_id = ?
            OR (SELECT primary_role FROM users WHERE id = ?) = 'admin'
            OR (r.visibility = 'shared' AND NOT EXISTS (
                SELECT 1 FROM resource_access_rules a WHERE a.resource_id = r.id
            ))
            OR EXISTS (
                SELECT 1 FROM resource_access_rules a
                WHERE a.resource_id = r.id AND (
                    a.subject_type = 'all'
                    OR (a.subject_type = 'member' AND a.subject_id = ?)
                    OR (a.subject_type = 'primary_role' AND a.subject_id = (
                        SELECT primary_role FROM users WHERE id = ?
                    ))
                    OR (a.subject_type = 'sub_role' AND a.subject_id IN (
                        SELECT sub_role_id FROM user_sub_roles WHERE user_id = ?
                    ))
                    OR (a.subject_type = 'tag' AND a.subject_id IN (
                        SELECT tag_id FROM user_tags WHERE user_id = ?
                    ))
                )
            )
        )
        "#,
    )
}

fn map_resources(rows: Vec<sqlx::any::AnyRow>) -> Vec<ManagedResource> {
    rows.into_iter()
        .filter_map(|row| map_resource(&row).ok())
        .collect()
}

fn map_effective_version(row: sqlx::any::AnyRow) -> EffectiveResourceVersion {
    let payload = serde_json::from_str(row.get::<String, _>("payload").as_str())
        .unwrap_or_else(|_| serde_json::json!({}));
    let bundle_v2 = bundle_v2_from_payload(&payload);
    EffectiveResourceVersion {
        project_id: parse_uuid(row.get("project_id")),
        resource_id: parse_uuid(row.get("resource_id")),
        version_id: parse_uuid(row.get("version_id")),
        kind: conductor_domain::ResourceKind::parse(row.get::<String, _>("kind").as_str())
            .unwrap_or(conductor_domain::ResourceKind::Agent),
        slug: row.get("slug"),
        version: row.get("version"),
        description: row.get("description"),
        changelog: row.get("changelog"),
        version_history: Vec::new(),
        release_channel: row
            .get::<Option<String>, _>("release_channel")
            .as_deref()
            .and_then(ReleaseChannel::parse)
            .unwrap_or(ReleaseChannel::Published),
        payload,
        sha256: row.get("content_sha256"),
        size: nonnegative_u64(row.get("content_size")),
        artifact_key: row.get("artifact_key"),
        bundle_v2,
        minimum_evoflux_version: row.get("minimum_evoflux_version"),
    }
}

fn map_version_notice(row: sqlx::any::AnyRow) -> Option<ResourceVersionNotice> {
    Some(ResourceVersionNotice {
        version_id: Uuid::parse_str(row.get::<String, _>("id").as_str()).ok()?,
        version: row.get("version"),
        status: ResourceVersionStatus::parse(row.get::<String, _>("status").as_str()),
        release_channel: row
            .get::<Option<String>, _>("release_channel")
            .as_deref()
            .and_then(ReleaseChannel::parse)?,
        changelog: row.get("changelog"),
        published_at: row.get::<Option<String>, _>("published_at").map(parse_dt),
        deprecation_reason: row.get("deprecation_reason"),
    })
}

fn map_version(row: sqlx::any::AnyRow) -> ResourceVersion {
    let payload = serde_json::from_str(row.get::<String, _>("payload").as_str())
        .unwrap_or_else(|_| serde_json::json!({}));
    let bundle_v2 = bundle_v2_from_payload(&payload);
    ResourceVersion {
        id: parse_uuid(row.get("id")),
        project_id: parse_uuid(row.get("project_id")),
        resource_id: parse_uuid(row.get("resource_id")),
        version: row.get("version"),
        status: ResourceVersionStatus::parse(row.get::<String, _>("status").as_str()),
        payload,
        changelog: row.get("changelog"),
        release_channel: row
            .get::<Option<String>, _>("release_channel")
            .as_deref()
            .and_then(ReleaseChannel::parse),
        active_channel: row
            .get::<Option<String>, _>("active_channel")
            .as_deref()
            .and_then(ReleaseChannel::parse),
        content_sha256: row.get("content_sha256"),
        content_size: nonnegative_u64(row.get("content_size")),
        artifact_key: row.get("artifact_key"),
        bundle_v2,
        minimum_evoflux_version: row.get("minimum_evoflux_version"),
        created_by: parse_uuid(row.get("created_by")),
        created_at: parse_dt(row.get("created_at")),
        published_at: row.get::<Option<String>, _>("published_at").map(parse_dt),
        deprecated_at: row.get::<Option<String>, _>("deprecated_at").map(parse_dt),
        deprecated_by: row
            .get::<Option<String>, _>("deprecated_by")
            .map(parse_uuid),
        deprecation_reason: row.get("deprecation_reason"),
    }
}

fn bundle_v2_from_payload(payload: &serde_json::Value) -> Option<ResourceBundleV2> {
    payload
        .get("bundle_v2")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .filter(|bundle: &ResourceBundleV2| {
            bundle.schema_version == ResourceBundleV2::SCHEMA_VERSION
        })
}

fn map_feedback(row: sqlx::any::AnyRow) -> ResourceFeedback {
    ResourceFeedback {
        id: parse_uuid(row.get("id")),
        resource_id: parse_uuid(row.get("resource_id")),
        resource_version: row.get("resource_version"),
        user_id: parse_uuid(row.get("user_id")),
        member_name: row.get("display_name"),
        rating: row.get::<i64, _>("rating").try_into().unwrap_or(0),
        comment: row.get("comment"),
        created_at: parse_dt(row.get("created_at")),
        updated_at: parse_dt(row.get("updated_at")),
    }
}

async fn insert_access_rule(
    tx: &mut sqlx::Transaction<'_, Any>,
    resource_id: Uuid,
    subject_type: &str,
    subject_id: &str,
    created_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO resource_access_rules (resource_id, subject_type, subject_id, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(resource_id.to_string())
    .bind(subject_type)
    .bind(subject_id)
    .bind(created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_resource_change(
    tx: &mut sqlx::Transaction<'_, Any>,
    project_id: &str,
    resource_id: Uuid,
    change_kind: &str,
    version_id: Option<Uuid>,
    channel: Option<&str>,
    created_at: &str,
) -> Result<(), sqlx::Error> {
    let next_sequence: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(sequence), 0) + 1 FROM resource_changes")
            .fetch_one(&mut **tx)
            .await?;
    sqlx::query(
        r#"
        INSERT INTO resource_changes (
            sequence, project_id, resource_id, effective_user_id,
            change_kind, version_id, channel, created_at
        ) VALUES (?, ?, ?, NULL, ?, ?, ?, ?)
        "#,
    )
    .bind(next_sequence)
    .bind(project_id)
    .bind(resource_id.to_string())
    .bind(change_kind)
    .bind(version_id.map(|value| value.to_string()))
    .bind(channel)
    .bind(created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_version_event(
    tx: &mut sqlx::Transaction<'_, Any>,
    project_id: &str,
    resource_id: Uuid,
    version_id: Uuid,
    action: &str,
    actor_id: Uuid,
    reason: Option<&str>,
    confirmed_deprecated: bool,
    created_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO resource_version_events (
            id, project_id, resource_id, version_id, action, actor_id,
            reason, confirmed_deprecated, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id)
    .bind(resource_id.to_string())
    .bind(version_id.to_string())
    .bind(action)
    .bind(actor_id.to_string())
    .bind(reason)
    .bind(i64::from(confirmed_deprecated))
    .bind(created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn clean_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn parse_uuid(value: String) -> Uuid {
    Uuid::parse_str(&value).unwrap_or_else(|_| Uuid::nil())
}

fn optional_uuid(value: Option<String>) -> Option<Uuid> {
    value.and_then(|value| Uuid::parse_str(&value).ok())
}

fn saturating_i64(value: u64) -> i64 {
    value.try_into().unwrap_or(i64::MAX)
}

fn nonnegative_u64(value: i64) -> u64 {
    value.max(0).try_into().unwrap_or(0)
}

fn average(total: u64, count: u64) -> u64 {
    total.checked_div(count).unwrap_or(0)
}

fn percentage(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        ((part as f64 / total as f64) * 1_000.0).round() / 10.0
    }
}

#[derive(Default)]
struct UsageAccumulator {
    executions: u64,
    successes: u64,
    failures: u64,
    total_duration_ms: u64,
}

impl UsageAccumulator {
    fn add(&mut self, outcome: &str, duration_ms: u64) {
        self.executions += 1;
        self.total_duration_ms += duration_ms;
        if outcome == "success" {
            self.successes += 1;
        } else if outcome == "failure" {
            self.failures += 1;
        }
    }
}

struct MemberAccumulator {
    member_name: String,
    executions: u64,
    successes: u64,
    total_duration_ms: u64,
    last_used_at: DateTime<Utc>,
}

impl MemberAccumulator {
    fn new(member_name: String, last_used_at: DateTime<Utc>) -> Self {
        Self {
            member_name,
            executions: 0,
            successes: 0,
            total_duration_ms: 0,
            last_used_at,
        }
    }

    fn add(&mut self, outcome: &str, duration_ms: u64, occurred_at: DateTime<Utc>) {
        self.executions += 1;
        self.total_duration_ms += duration_ms;
        if outcome == "success" {
            self.successes += 1;
        }
        if occurred_at > self.last_used_at {
            self.last_used_at = occurred_at;
        }
    }
}
