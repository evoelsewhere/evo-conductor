use std::collections::{BTreeMap, HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use conductor_domain::{
    CreateResourceRequest, EffectiveResourceVersion, ManagedResource, PrimaryRole, ReleaseChannel,
    ReleaseResourceRequest, ReleaseResourceResult, ResourceAccessPolicy, ResourceBundle,
    ResourceDailyUsage, ResourceFeedback, ResourceInstallationState, ResourceInventoryMonitoring,
    ResourceInventoryMonitoringSummary, ResourceInventoryObservedState, ResourceInventoryRequest,
    ResourceKind, ResourceMemberUsage, ResourceMonitoring, ResourceMonitoringSummary,
    ResourceUsageEventRequest, ResourceVersion, ResourceVersionLifecycleAction,
    ResourceVersionNotice, ResourceVersionStatus, SemanticVersion, UpdateResourceRequest,
    UpsertResourceFeedbackRequest, VersionMode,
};
use sqlx::{Any, Pool, QueryBuilder, Row};
use uuid::Uuid;

use crate::core::error::{
    InvalidPersistedPrincipal, InvalidPersistedResource, PersistedPrincipalField,
    PersistedResourceField, PersistedSecurityReason, StorageError, StorageResult,
};
use crate::core::mapping::{canonicalize_resource_payload, map_resource, parse_dt};

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

#[derive(Debug, thiserror::Error)]
pub enum InventoryWriteError {
    #[error("inventory item is invalid: {0}")]
    Invalid(&'static str),
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
    ) -> StorageResult<Vec<ManagedResource>> {
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
            WHERE r.project_id = (
                SELECT id FROM instance ORDER BY created_at ASC LIMIT 1
            ) AND (r.owner_user_id = ? OR (
                r.status = 'published' AND (
                    (SELECT primary_role FROM users WHERE id = ?) = 'admin'
                    OR (r.visibility = 'shared' AND NOT EXISTS (
                        SELECT 1 FROM resource_access_rules a WHERE a.resource_id = r.id
                    ))
                    OR EXISTS (
                        SELECT 1 FROM resource_access_rules a
                        WHERE a.resource_id = r.id
                          AND a.project_id = r.project_id
                          AND a.effect = 'allow'
                          AND (
                            (a.subject_type = 'all' AND a.subject_id = '*')
                            OR (a.subject_type = 'member' AND a.subject_id = ?)
                            OR (a.subject_type = 'primary_role' AND a.subject_id = (
                                SELECT primary_role FROM users WHERE id = ?
                            ))
                            OR (a.subject_type = 'sub_role' AND a.subject_id IN (
                                SELECT assignment.sub_role_id
                                FROM user_sub_roles assignment
                                JOIN sub_roles role ON role.id = assignment.sub_role_id
                                WHERE assignment.user_id = ?
                            ))
                            OR (a.subject_type = 'tag' AND a.subject_id IN (
                                SELECT assignment.tag_id
                                FROM tag_assignments assignment
                                JOIN tags tag ON tag.id = assignment.tag_id
                                WHERE assignment.entity_type = 'member'
                                  AND assignment.entity_id = ?
                            ))
                        )
                    )
                )
            ))
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
        map_resources(rows)
    }

    pub async fn list_all(&self) -> StorageResult<Vec<ManagedResource>> {
        let project_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM instance ORDER BY created_at ASC LIMIT 1")
                .fetch_optional(&self.pool)
                .await?;
        if let Some(project_id) = project_id.as_deref() {
            let foreign_rows =
                sqlx::query("SELECT id, project_id FROM resources WHERE project_id <> ?")
                    .bind(project_id)
                    .fetch_all(&self.pool)
                    .await?;
            for row in foreign_rows {
                let raw_resource_id: String = row.try_get("id")?;
                let resource_id = Uuid::parse_str(&raw_resource_id).ok();
                let raw_project_id: String = row.try_get("project_id")?;
                if Uuid::parse_str(&raw_project_id).is_err() {
                    return Err(InvalidPersistedResource::new(
                        resource_id,
                        PersistedResourceField::ProjectId,
                        PersistedSecurityReason::InvalidUuid,
                    )
                    .into());
                }
            }
        }
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, kind, slug, name, description, version,
                   highest_semver, draft_revision, release_channel, owner_user_id,
                   visibility, status, payload, published_at, created_at, updated_at
            FROM resources
            WHERE project_id = (
                SELECT id FROM instance ORDER BY created_at ASC LIMIT 1
            )
            ORDER BY updated_at DESC, name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        map_resources(rows)
    }

    pub async fn list_visible_to(&self, user_id: Uuid) -> StorageResult<Vec<ManagedResource>> {
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
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        map_resources(rows)
    }

    pub async fn visible_resource_ids(&self, user_id: Uuid) -> StorageResult<HashSet<Uuid>> {
        let rows = sqlx::query(&visible_resources_query("SELECT r.id"))
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(map_resource_id).collect()
    }

    pub async fn find_by_id(&self, resource_id: Uuid) -> StorageResult<Option<ManagedResource>> {
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

    /// Load a complete resource while strictly decoding every field used by
    /// owner/kind/lifecycle authorization. Legacy display mapping remains
    /// permissive elsewhere, but it must never feed a policy decision.
    pub async fn find_by_id_for_authorization(
        &self,
        resource_id: Uuid,
    ) -> StorageResult<Option<ManagedResource>> {
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
            SELECT COUNT(*)
            FROM installation_resource_inventory inventory
            JOIN client_installations installation
              ON installation.id = inventory.installation_id
             AND installation.instance_id = inventory.project_id
            JOIN resources resource
              ON resource.id = inventory.resource_id
             AND resource.project_id = inventory.project_id
             AND resource.kind = 'plugin'
            JOIN resource_versions version
              ON version.id = inventory.applied_version_id
             AND version.project_id = inventory.project_id
             AND version.resource_id = inventory.resource_id
             AND version.status <> 'draft'
            WHERE inventory.project_id = ? AND inventory.installation_id = ?
              AND inventory.resource_id = ? AND inventory.applied_version_id = ?
              AND inventory.plugin_installation_id = ?
              AND inventory.observed_state IN ('applied', 'in_sync', 'trust_pending')
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
    ) -> StorageResult<ManagedResource> {
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
    ) -> StorageResult<Option<ManagedResource>> {
        let existing = match self.find_by_id(resource_id).await? {
            Some(resource) => resource,
            None => return Ok(None),
        };
        let mut tx = self.pool.begin().await?;
        let project_id = existing.project_id.to_string();
        let previous_audience =
            visible_user_ids_for_resource(&mut tx, &project_id, resource_id).await?;
        let now = Utc::now().to_rfc3339();
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
        .bind(&now)
        .bind(resource_id.to_string())
        .execute(&mut *tx)
        .await?;
        let mut affected_audience = previous_audience.into_iter().collect::<HashSet<_>>();
        affected_audience
            .extend(visible_user_ids_for_resource(&mut tx, &project_id, resource_id).await?);
        for user_id in affected_audience {
            insert_resource_change(
                &mut tx,
                &project_id,
                resource_id,
                ResourceChangeInsert {
                    effective_user_id: Some(&user_id),
                    change_kind: "update",
                    version_id: None,
                    channel: None,
                    created_at: &now,
                },
            )
            .await?;
        }
        tx.commit().await?;
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
        let previous_audience =
            visible_user_ids_for_resource(&mut tx, &project_id, resource_id).await?;
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
        for user_id in previous_audience {
            insert_resource_change(
                &mut tx,
                &project_id,
                resource_id,
                ResourceChangeInsert {
                    effective_user_id: Some(&user_id),
                    change_kind: "archive",
                    version_id: None,
                    channel: None,
                    created_at: &now,
                },
            )
            .await?;
        }
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
            ResourceChangeInsert {
                effective_user_id: None,
                change_kind: "deprecate",
                version_id: Some(version_id),
                channel: None,
                created_at: &now,
            },
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
        project_id: Uuid,
    ) -> StorageResult<ResourceAccessPolicy> {
        let rows = sqlx::query(
            "SELECT project_id, subject_type, subject_id, effect \
             FROM resource_access_rules WHERE resource_id = ?",
        )
        .bind(resource_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        let mut policy = ResourceAccessPolicy::default();
        for row in rows {
            let stored_project_id: String = row.try_get("project_id")?;
            let subject_type: String = row.try_get("subject_type")?;
            let subject_id: String = row.try_get("subject_id")?;
            let effect: String = row.try_get("effect")?;
            let invalid_policy = || {
                InvalidPersistedResource::new(
                    Some(resource_id),
                    PersistedResourceField::AccessPolicy,
                    PersistedSecurityReason::UnknownValue,
                )
            };
            if stored_project_id != project_id.to_string() || effect != "allow" {
                return Err(invalid_policy().into());
            }
            match subject_type.as_str() {
                "all" if subject_id == "*" => policy.all_members = true,
                "primary_role" => {
                    let role = PrimaryRole::parse(&subject_id).ok_or_else(invalid_policy)?;
                    policy.primary_roles.push(role.as_str().to_owned());
                }
                "sub_role" => {
                    Uuid::parse_str(&subject_id).map_err(|_| invalid_policy())?;
                    let exists: i64 =
                        sqlx::query_scalar("SELECT COUNT(*) FROM sub_roles WHERE id = ?")
                            .bind(&subject_id)
                            .fetch_one(&self.pool)
                            .await?;
                    if exists != 1 {
                        return Err(invalid_policy().into());
                    }
                    policy.sub_role_ids.push(subject_id);
                }
                "tag" => {
                    Uuid::parse_str(&subject_id).map_err(|_| invalid_policy())?;
                    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = ?")
                        .bind(&subject_id)
                        .fetch_one(&self.pool)
                        .await?;
                    if exists != 1 {
                        return Err(invalid_policy().into());
                    }
                    policy.tag_ids.push(subject_id);
                }
                "member" => {
                    let member_id = Uuid::parse_str(&subject_id).map_err(|_| invalid_policy())?;
                    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?")
                        .bind(&subject_id)
                        .fetch_one(&self.pool)
                        .await?;
                    if exists != 1 {
                        return Err(invalid_policy().into());
                    }
                    policy.member_ids.push(member_id);
                }
                _ => return Err(invalid_policy().into()),
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
        let project_id: String =
            sqlx::query_scalar("SELECT project_id FROM resources WHERE id = ?")
                .bind(resource_id.to_string())
                .fetch_one(&mut *tx)
                .await?;
        let previous_audience =
            visible_user_ids_for_resource(&mut tx, &project_id, resource_id).await?;
        sqlx::query("DELETE FROM resource_access_rules WHERE resource_id = ?")
            .bind(resource_id.to_string())
            .execute(&mut *tx)
            .await?;
        let now = Utc::now().to_rfc3339();
        if policy.all_members {
            insert_access_rule(&mut tx, &project_id, resource_id, "all", "*", &now).await?;
        }
        for role in &policy.primary_roles {
            insert_access_rule(
                &mut tx,
                &project_id,
                resource_id,
                "primary_role",
                role,
                &now,
            )
            .await?;
        }
        for id in &policy.sub_role_ids {
            insert_access_rule(&mut tx, &project_id, resource_id, "sub_role", id, &now).await?;
        }
        for id in &policy.tag_ids {
            insert_access_rule(&mut tx, &project_id, resource_id, "tag", id, &now).await?;
        }
        for id in &policy.member_ids {
            insert_access_rule(
                &mut tx,
                &project_id,
                resource_id,
                "member",
                &id.to_string(),
                &now,
            )
            .await?;
        }
        let mut affected_audience = previous_audience.into_iter().collect::<HashSet<_>>();
        affected_audience
            .extend(visible_user_ids_for_resource(&mut tx, &project_id, resource_id).await?);
        for user_id in affected_audience {
            insert_resource_change(
                &mut tx,
                &project_id,
                resource_id,
                ResourceChangeInsert {
                    effective_user_id: Some(&user_id),
                    change_kind: "access",
                    version_id: None,
                    channel: None,
                    created_at: &now,
                },
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
    ) -> StorageResult<Option<EffectiveResourceVersion>> {
        if !self
            .visible_resource_ids(user_id)
            .await?
            .contains(&resource_id)
        {
            return Ok(None);
        }
        let beta: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM resource_beta_members \
             WHERE resource_id = ? AND user_id = ? \
               AND project_id = (SELECT project_id FROM resources WHERE id = ?)",
        )
        .bind(resource_id.to_string())
        .bind(user_id.to_string())
        .bind(resource_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        let preferred = if beta > 0 { "beta" } else { "published" };
        let preferred_pointer: Option<String> = sqlx::query_scalar(
            "SELECT c.version_id FROM resource_release_channels c \
             JOIN resources r ON r.id = c.resource_id AND r.project_id = c.project_id \
             WHERE r.id = ? AND c.channel = ?",
        )
        .bind(resource_id.to_string())
        .bind(preferred)
        .fetch_optional(&self.pool)
        .await?;
        if preferred_pointer
            .as_deref()
            .is_some_and(|value| Uuid::parse_str(value).is_err())
        {
            return Err(InvalidPersistedResource::new(
                Some(resource_id),
                PersistedResourceField::VersionId,
                PersistedSecurityReason::InvalidUuid,
            )
            .into());
        }
        let row = sqlx::query(
            r#"
            SELECT r.project_id, r.id AS resource_id, r.kind, r.slug, r.description,
                   rv.id AS version_id, rv.version, rv.changelog, rv.release_channel, rv.payload,
                   rv.content_sha256, rv.content_size, rv.artifact_key,
                   rv.minimum_evoflux_version
            FROM resources r
            JOIN resource_release_channels c ON c.project_id = r.project_id
              AND c.resource_id = r.id AND c.channel = ?
            JOIN resource_versions rv ON rv.id = c.version_id
              AND rv.project_id = r.project_id AND rv.resource_id = r.id
              AND rv.release_channel = c.channel AND rv.status = c.channel
            WHERE r.id = ? AND r.status <> 'archived'
            "#,
        )
        .bind(preferred)
        .bind(resource_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        if preferred_pointer.is_some() && row.is_none() {
            return Err(release_pointer_integrity_error(
                &self.pool,
                resource_id,
                preferred_pointer
                    .as_deref()
                    .expect("pointer checked as present"),
                preferred,
            )
            .await?
            .into());
        }
        let row = if row.is_none() && preferred == "beta" {
            let published_pointer: Option<String> = sqlx::query_scalar(
                "SELECT c.version_id FROM resource_release_channels c \
                 JOIN resources r ON r.id = c.resource_id AND r.project_id = c.project_id \
                 WHERE r.id = ? AND c.channel = 'published'",
            )
            .bind(resource_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
            if published_pointer
                .as_deref()
                .is_some_and(|value| Uuid::parse_str(value).is_err())
            {
                return Err(InvalidPersistedResource::new(
                    Some(resource_id),
                    PersistedResourceField::VersionId,
                    PersistedSecurityReason::InvalidUuid,
                )
                .into());
            }
            let published_row = sqlx::query(
                r#"
                SELECT r.project_id, r.id AS resource_id, r.kind, r.slug, r.description,
                       rv.id AS version_id, rv.version, rv.changelog, rv.release_channel, rv.payload,
                       rv.content_sha256, rv.content_size, rv.artifact_key,
                       rv.minimum_evoflux_version
                FROM resources r
                JOIN resource_release_channels c ON c.project_id = r.project_id
                  AND c.resource_id = r.id AND c.channel = 'published'
                JOIN resource_versions rv ON rv.id = c.version_id
                  AND rv.project_id = r.project_id AND rv.resource_id = r.id
                  AND rv.release_channel = c.channel AND rv.status = c.channel
                WHERE r.id = ? AND r.status <> 'archived'
                "#,
            )
            .bind(resource_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
            if published_pointer.is_some() && published_row.is_none() {
                return Err(release_pointer_integrity_error(
                    &self.pool,
                    resource_id,
                    published_pointer
                        .as_deref()
                        .expect("pointer checked as present"),
                    "published",
                )
                .await?
                .into());
            }
            published_row
        } else {
            row
        };
        let mut version = row.map(map_effective_version).transpose()?;
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
                WHERE project_id = ? AND resource_id = ? AND status <> 'draft'
                  AND (release_channel = 'published' OR release_channel = ?)
                ORDER BY created_at ASC
                "#,
            )
            .bind(version.project_id.to_string())
            .bind(resource_id.to_string())
            .bind(allowed_channel)
            .fetch_all(&self.pool)
            .await?;
            version.version_history = rows
                .into_iter()
                .map(|row| map_version_notice(row, resource_id))
                .collect::<StorageResult<_>>()?;
        }
        Ok(version)
    }

    /// Resolve the complete desired checkout for one member in a single SQL
    /// statement. Beta members receive the beta ref when it exists and fall
    /// back to published, matching [`Self::effective_version`].
    pub async fn list_effective_versions(
        &self,
        user_id: Uuid,
    ) -> StorageResult<Vec<EffectiveResourceVersion>> {
        let visible = visible_resources_query("SELECT r.id");
        let invalid_link_query = format!(
            r#"
            SELECT c.resource_id, c.version_id, c.channel
            FROM resources r
            JOIN resource_release_channels c ON c.project_id = r.project_id
              AND c.resource_id = r.id
              AND c.channel = CASE
                WHEN EXISTS (
                    SELECT 1 FROM resource_beta_members b
                    WHERE b.project_id = r.project_id
                      AND b.resource_id = r.id AND b.user_id = ?
                ) AND EXISTS (
                    SELECT 1 FROM resource_release_channels beta
                    WHERE beta.project_id = r.project_id
                      AND beta.resource_id = r.id AND beta.channel = 'beta'
                ) THEN 'beta'
                ELSE 'published'
              END
            LEFT JOIN resource_versions rv ON rv.id = c.version_id
              AND rv.project_id = r.project_id AND rv.resource_id = r.id
              AND rv.release_channel = c.channel AND rv.status = c.channel
            WHERE r.id IN ({visible}) AND rv.id IS NULL
            LIMIT 1
            "#,
        );
        let invalid_link = sqlx::query(&invalid_link_query)
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = invalid_link {
            let raw_resource_id: String = row.try_get("resource_id")?;
            let resource_id = Uuid::parse_str(&raw_resource_id).map_err(|_| {
                InvalidPersistedResource::new(
                    None,
                    PersistedResourceField::Id,
                    PersistedSecurityReason::InvalidUuid,
                )
            })?;
            let version_id: String = row.try_get("version_id")?;
            if Uuid::parse_str(&version_id).is_err() {
                return Err(InvalidPersistedResource::new(
                    Some(resource_id),
                    PersistedResourceField::VersionId,
                    PersistedSecurityReason::InvalidUuid,
                )
                .into());
            }
            let channel: String = row.try_get("channel")?;
            return Err(release_pointer_integrity_error(
                &self.pool,
                resource_id,
                &version_id,
                &channel,
            )
            .await?
            .into());
        }
        let query = format!(
            r#"
            SELECT r.project_id, r.id AS resource_id, r.kind, r.slug, r.description,
                   rv.id AS version_id, rv.version, rv.changelog, rv.release_channel, rv.payload,
                   rv.content_sha256, rv.content_size, rv.artifact_key,
                   rv.minimum_evoflux_version
            FROM resources r
            JOIN resource_release_channels c ON c.project_id = r.project_id
              AND c.resource_id = r.id
              AND c.channel = CASE
                WHEN EXISTS (
                    SELECT 1 FROM resource_beta_members b
                    WHERE b.project_id = r.project_id
                      AND b.resource_id = r.id AND b.user_id = ?
                ) AND EXISTS (
                    SELECT 1 FROM resource_release_channels beta
                    WHERE beta.project_id = r.project_id
                      AND beta.resource_id = r.id AND beta.channel = 'beta'
                ) THEN 'beta'
                ELSE 'published'
              END
            JOIN resource_versions rv ON rv.id = c.version_id
              AND rv.project_id = r.project_id AND rv.resource_id = r.id
              AND rv.release_channel = c.channel AND rv.status = c.channel
            WHERE r.id IN ({visible})
            ORDER BY r.kind, r.slug, r.id
            "#,
        );
        let rows = sqlx::query(&query)
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(map_effective_version).collect()
    }

    pub async fn max_change_sequence(&self, project_id: Uuid, user_id: Uuid) -> StorageResult<u64> {
        let row = sqlx::query(
            r#"
            SELECT c.sequence, c.resource_id, c.effective_user_id,
                   r.id AS resolved_resource_id, r.project_id AS resource_project_id
            FROM resource_changes c
            LEFT JOIN resources r ON r.id = c.resource_id
            WHERE c.project_id = ?
              AND (c.effective_user_id IS NULL OR c.effective_user_id = ?)
            ORDER BY c.sequence DESC
            LIMIT 1
            "#,
        )
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(0);
        };
        let (sequence, _, _) = map_resource_change_sequence(row, project_id)?;
        u64::try_from(sequence).map_err(|_| {
            InvalidPersistedResource::new(
                None,
                PersistedResourceField::ChangeSequence,
                PersistedSecurityReason::InvalidInteger,
            )
            .into()
        })
    }

    pub async fn change_sequences(
        &self,
        project_id: Uuid,
        user_id: Uuid,
        after: i64,
        limit: u32,
    ) -> StorageResult<Vec<(i64, Uuid, Option<Uuid>)>> {
        let rows = sqlx::query(
            r#"
            SELECT c.sequence, c.resource_id, c.effective_user_id,
                   r.id AS resolved_resource_id, r.project_id AS resource_project_id
            FROM resource_changes c
            LEFT JOIN resources r ON r.id = c.resource_id
            WHERE c.project_id = ? AND c.sequence > ?
              AND (c.effective_user_id IS NULL OR c.effective_user_id = ?)
            ORDER BY c.sequence ASC
            LIMIT ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(after)
        .bind(user_id.to_string())
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| map_resource_change_sequence(row, project_id))
            .collect()
    }

    pub async fn upsert_inventory(
        &self,
        project_id: Uuid,
        request: &ResourceInventoryRequest,
    ) -> Result<u32, InventoryWriteError> {
        let mut tx = self.pool.begin().await?;
        let installation_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM client_installations WHERE id = ? AND instance_id = ?",
        )
        .bind(request.installation_id.to_string())
        .bind(project_id.to_string())
        .fetch_one(&mut *tx)
        .await?;
        if installation_count != 1 {
            return Err(InventoryWriteError::Invalid(
                "installation does not belong to the project",
            ));
        }
        let mut accepted = 0_u32;
        let mut resource_ids = HashSet::with_capacity(request.items.len());
        for item in &request.items {
            if !resource_ids.insert(item.resource_id) {
                return Err(InventoryWriteError::Invalid(
                    "resource_id appears more than once",
                ));
            }
            let resource_kind: Option<String> =
                sqlx::query_scalar("SELECT kind FROM resources WHERE id = ? AND project_id = ?")
                    .bind(item.resource_id.to_string())
                    .bind(project_id.to_string())
                    .fetch_optional(&mut *tx)
                    .await?;
            let resource_kind = resource_kind
                .as_deref()
                .and_then(ResourceKind::parse)
                .ok_or(InventoryWriteError::Invalid(
                    "resource does not belong to the project",
                ))?;
            let desired = inventory_version_facts(
                &mut tx,
                project_id,
                item.resource_id,
                item.desired_version_id,
            )
            .await?;
            let applied = inventory_version_facts(
                &mut tx,
                project_id,
                item.resource_id,
                item.applied_version_id,
            )
            .await?;
            if item.desired_version_id.is_some() && desired.is_none()
                || item.applied_version_id.is_some() && applied.is_none()
            {
                return Err(InventoryWriteError::Invalid(
                    "version does not belong to the resource",
                ));
            }
            if ResourceInventoryObservedState::INSTALLED.contains(&item.observed_state)
                && item.applied_version_id.is_none()
            {
                return Err(InventoryWriteError::Invalid(
                    "installed state requires applied_version_id",
                ));
            }
            if item.observed_state == ResourceInventoryObservedState::InSync
                && (item.desired_version_id.is_none()
                    || item.desired_version_id != item.applied_version_id)
            {
                return Err(InventoryWriteError::Invalid(
                    "in_sync requires matching desired and applied versions",
                ));
            }
            if let Some(channel) = item.release_channel {
                let expected_channel = desired
                    .as_ref()
                    .or(applied.as_ref())
                    .and_then(|facts| facts.release_channel.as_deref());
                if expected_channel != Some(channel.as_str()) {
                    return Err(InventoryWriteError::Invalid(
                        "release channel does not match the reported version",
                    ));
                }
            }
            if let (Some(content_sha256), Some(applied)) =
                (item.content_sha256.as_deref(), applied.as_ref())
            {
                if content_sha256 != applied.content_sha256 {
                    return Err(InventoryWriteError::Invalid(
                        "content digest does not match applied version",
                    ));
                }
            }
            if item.plugin_installation_id.is_some() && resource_kind != ResourceKind::Plugin {
                return Err(InventoryWriteError::Invalid(
                    "plugin installation id requires a Plugin resource",
                ));
            }
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
            .bind(item.observed_state.as_str())
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
    ) -> StorageResult<ResourceMonitoring> {
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
            let invalid =
                |field, reason| InvalidPersistedResource::new(Some(resource_id), field, reason);
            let occurred_at_raw: String = row.try_get("occurred_at")?;
            let occurred_at = parse_effective_resource_dt(
                &occurred_at_raw,
                resource_id,
                PersistedResourceField::UsageOccurredAt,
            )?;
            let date = occurred_at.format("%Y-%m-%d").to_string();
            let outcome: String = row.try_get("outcome")?;
            if !matches!(outcome.as_str(), "success" | "failure" | "cancelled") {
                return Err(invalid(
                    PersistedResourceField::UsageOutcome,
                    PersistedSecurityReason::UnknownValue,
                )
                .into());
            }
            let duration_ms =
                u64::try_from(row.try_get::<i64, _>("duration_ms")?).map_err(|_| {
                    invalid(
                        PersistedResourceField::UsageDuration,
                        PersistedSecurityReason::InvalidInteger,
                    )
                })?;
            let tokens_in = u64::try_from(row.try_get::<i64, _>("tokens_in")?).map_err(|_| {
                invalid(
                    PersistedResourceField::UsageTokens,
                    PersistedSecurityReason::InvalidInteger,
                )
            })?;
            let tokens_out = u64::try_from(row.try_get::<i64, _>("tokens_out")?).map_err(|_| {
                invalid(
                    PersistedResourceField::UsageTokens,
                    PersistedSecurityReason::InvalidInteger,
                )
            })?;
            let user_id_raw: String = row.try_get("user_id")?;
            let user_id = Uuid::parse_str(&user_id_raw).map_err(|_| {
                InvalidPersistedPrincipal::new(
                    None,
                    PersistedPrincipalField::Id,
                    PersistedSecurityReason::InvalidUuid,
                )
            })?;
            let member_name: String = row.try_get("display_name")?;

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
    ) -> StorageResult<ResourceInventoryMonitoring> {
        let rows = sqlx::query(
            r#"
            SELECT i.project_id, i.resource_id, resource.project_id AS resource_project_id,
                   i.installation_id, c.id AS resolved_installation_id,
                   c.instance_id AS installation_project_id,
                   c.display_name AS installation_name, c.platform,
                   c.evoflux_version, c.user_id, c.last_seen_at,
                   u.id AS resolved_user_id, u.display_name AS member_name,
                   u.email, u.primary_role,
                   i.desired_version_id, desired.id AS resolved_desired_version_id,
                   desired.project_id AS desired_project_id,
                   desired.resource_id AS desired_resource_id,
                   desired.status AS desired_status,
                   desired.version AS desired_version,
                   i.applied_version_id, applied.id AS resolved_applied_version_id,
                   applied.project_id AS applied_project_id,
                   applied.resource_id AS applied_resource_id,
                   applied.status AS applied_status,
                   applied.version AS applied_version,
                   i.release_channel, i.plugin_installation_id, i.observed_state,
                   i.error_category, i.observed_at
            FROM installation_resource_inventory i
            LEFT JOIN resources resource ON resource.id = i.resource_id
            LEFT JOIN client_installations c ON c.id = i.installation_id
            LEFT JOIN users u ON u.id = c.user_id
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
            let invalid = |field, reason| {
                StorageError::InvalidPersistedResource(InvalidPersistedResource::new(
                    Some(resource_id),
                    field,
                    reason,
                ))
            };
            let inventory_project_id_raw: String = row.try_get("project_id")?;
            let inventory_project_id =
                Uuid::parse_str(&inventory_project_id_raw).map_err(|_| {
                    invalid(
                        PersistedResourceField::ProjectId,
                        PersistedSecurityReason::InvalidUuid,
                    )
                })?;
            let resource_project_id: Option<String> = row.try_get("resource_project_id")?;
            let stored_resource_id: String = row.try_get("resource_id")?;
            if stored_resource_id != resource_id.to_string()
                || resource_project_id.as_deref() != Some(inventory_project_id_raw.as_str())
            {
                return Err(invalid(
                    PersistedResourceField::ProjectId,
                    PersistedSecurityReason::UnknownValue,
                ));
            }
            let installation_id_raw: String = row.try_get("installation_id")?;
            let installation_id = Uuid::parse_str(&installation_id_raw).map_err(|_| {
                invalid(
                    PersistedResourceField::InventoryInstallationId,
                    PersistedSecurityReason::InvalidUuid,
                )
            })?;
            let resolved_installation_id: Option<String> =
                row.try_get("resolved_installation_id")?;
            let installation_project_id: Option<String> = row.try_get("installation_project_id")?;
            if resolved_installation_id.as_deref() != Some(installation_id_raw.as_str())
                || installation_project_id.as_deref() != Some(inventory_project_id_raw.as_str())
            {
                return Err(invalid(
                    PersistedResourceField::InventoryInstallationId,
                    PersistedSecurityReason::UnknownValue,
                ));
            }
            let user_id_raw: String = row.try_get("user_id")?;
            let user_id = Uuid::parse_str(&user_id_raw).map_err(|_| {
                StorageError::InvalidPersistedPrincipal(InvalidPersistedPrincipal::new(
                    None,
                    PersistedPrincipalField::Id,
                    PersistedSecurityReason::InvalidUuid,
                ))
            })?;
            let resolved_user_id: Option<String> = row.try_get("resolved_user_id")?;
            if resolved_user_id.as_deref() != Some(user_id_raw.as_str()) {
                return Err(StorageError::InvalidPersistedPrincipal(
                    InvalidPersistedPrincipal::new(
                        Some(user_id),
                        PersistedPrincipalField::Id,
                        PersistedSecurityReason::UnknownValue,
                    ),
                ));
            }
            let (desired_version_id, desired_version) = map_inventory_version_reference(
                &row,
                inventory_project_id,
                resource_id,
                InventoryVersionColumns {
                    id: "desired_version_id",
                    resolved_id: "resolved_desired_version_id",
                    project_id: "desired_project_id",
                    resource_id: "desired_resource_id",
                    status: "desired_status",
                    version: "desired_version",
                },
                PersistedResourceField::InventoryDesiredVersionId,
            )?;
            let (applied_version_id, applied_version) = map_inventory_version_reference(
                &row,
                inventory_project_id,
                resource_id,
                InventoryVersionColumns {
                    id: "applied_version_id",
                    resolved_id: "resolved_applied_version_id",
                    project_id: "applied_project_id",
                    resource_id: "applied_resource_id",
                    status: "applied_status",
                    version: "applied_version",
                },
                PersistedResourceField::InventoryAppliedVersionId,
            )?;
            let observed_state_raw: String = row.try_get("observed_state")?;
            let observed_state = ResourceInventoryObservedState::parse(&observed_state_raw)
                .ok_or_else(|| {
                    invalid(
                        PersistedResourceField::InventoryObservedState,
                        PersistedSecurityReason::UnknownValue,
                    )
                })?;
            if ResourceInventoryObservedState::INSTALLED.contains(&observed_state)
                && applied_version_id.is_none()
            {
                return Err(invalid(
                    PersistedResourceField::InventoryAppliedVersionId,
                    PersistedSecurityReason::EmptyCollection,
                ));
            }
            if observed_state == ResourceInventoryObservedState::InSync
                && (desired_version_id.is_none() || desired_version_id != applied_version_id)
            {
                return Err(invalid(
                    PersistedResourceField::InventoryObservedState,
                    PersistedSecurityReason::UnknownValue,
                ));
            }
            if ResourceInventoryObservedState::INSTALLED.contains(&observed_state) {
                installed_installations.insert(installation_id);
                installed_members.insert(user_id);
            } else if ResourceInventoryObservedState::PENDING.contains(&observed_state) {
                summary.pending_installations += 1;
            } else {
                summary.attention_installations += 1;
            }
            let primary_role_raw: String = row.get("primary_role");
            let primary_role = PrimaryRole::parse(&primary_role_raw).ok_or_else(|| {
                StorageError::InvalidPersistedPrincipal(InvalidPersistedPrincipal::new(
                    Some(user_id),
                    PersistedPrincipalField::PrimaryRole,
                    PersistedSecurityReason::UnknownValue,
                ))
            })?;
            let release_channel_raw: Option<String> = row.try_get("release_channel")?;
            let release_channel = release_channel_raw
                .as_deref()
                .map(|value| {
                    ReleaseChannel::parse(value).ok_or_else(|| {
                        invalid(
                            PersistedResourceField::ReleaseChannel,
                            PersistedSecurityReason::UnknownValue,
                        )
                    })
                })
                .transpose()?;
            let observed_at_raw: String = row.try_get("observed_at")?;
            let observed_at = parse_effective_resource_dt(
                &observed_at_raw,
                resource_id,
                PersistedResourceField::InventoryObservedAt,
            )?;
            let last_seen_at_raw: String = row.try_get("last_seen_at")?;
            let last_seen_at = parse_effective_resource_dt(
                &last_seen_at_raw,
                resource_id,
                PersistedResourceField::InventoryLastSeenAt,
            )?;
            summary.reported_installations += 1;
            installations.push(ResourceInstallationState {
                installation_id,
                installation_name: row.try_get("installation_name")?,
                platform: row.try_get("platform")?,
                evoflux_version: row.try_get("evoflux_version")?,
                user_id,
                member_name: row.try_get("member_name")?,
                email: row.try_get("email")?,
                primary_role,
                desired_version_id,
                desired_version,
                applied_version_id,
                applied_version,
                release_channel,
                plugin_installation_id: row.try_get("plugin_installation_id")?,
                observed_state: observed_state.as_str().to_owned(),
                error_category: row.try_get("error_category")?,
                observed_at,
                last_seen_at,
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
        WHERE r.project_id = (
            SELECT id FROM instance ORDER BY created_at ASC LIMIT 1
        ) AND r.status IN ('beta', 'published')
          AND (
            r.status <> 'beta'
            OR EXISTS (
                SELECT 1 FROM resource_beta_members beta_audience
                WHERE beta_audience.project_id = r.project_id
                  AND beta_audience.resource_id = r.id
                  AND beta_audience.user_id = ?
            )
          ) AND (
            r.owner_user_id = ?
            OR (SELECT primary_role FROM users WHERE id = ?) = 'admin'
            OR (r.visibility = 'shared' AND NOT EXISTS (
                SELECT 1 FROM resource_access_rules a WHERE a.resource_id = r.id
            ))
            OR EXISTS (
                SELECT 1 FROM resource_access_rules a
                WHERE a.resource_id = r.id
                  AND a.project_id = r.project_id
                  AND a.effect = 'allow'
                  AND (
                    (a.subject_type = 'all' AND a.subject_id = '*')
                    OR (a.subject_type = 'member' AND a.subject_id = ?)
                    OR (a.subject_type = 'primary_role' AND a.subject_id = (
                        SELECT primary_role FROM users WHERE id = ?
                    ))
                    OR (a.subject_type = 'sub_role' AND a.subject_id IN (
                        SELECT assignment.sub_role_id
                        FROM user_sub_roles assignment
                        JOIN sub_roles role ON role.id = assignment.sub_role_id
                        WHERE assignment.user_id = ?
                    ))
                    OR (a.subject_type = 'tag' AND a.subject_id IN (
                        SELECT assignment.tag_id
                        FROM tag_assignments assignment
                        JOIN tags tag ON tag.id = assignment.tag_id
                        WHERE assignment.entity_type = 'member'
                          AND assignment.entity_id = ?
                    ))
                )
            )
        )
        "#,
    )
}

fn map_resource_id(row: sqlx::any::AnyRow) -> StorageResult<Uuid> {
    let raw: String = row.try_get("id").map_err(|error| {
        persisted_resource_column_error(
            error,
            None,
            PersistedResourceField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    Uuid::parse_str(&raw).map_err(|_| {
        InvalidPersistedResource::new(
            None,
            PersistedResourceField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
        .into()
    })
}

fn map_resource_change_sequence(
    row: sqlx::any::AnyRow,
    expected_project_id: Uuid,
) -> StorageResult<(i64, Uuid, Option<Uuid>)> {
    let raw_resource_id: String = row.try_get("resource_id")?;
    let resource_id = Uuid::parse_str(&raw_resource_id).map_err(|_| {
        InvalidPersistedResource::new(
            None,
            PersistedResourceField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let resolved_resource_id: Option<String> = row.try_get("resolved_resource_id")?;
    let resource_project_id: Option<String> = row.try_get("resource_project_id")?;
    if resolved_resource_id.as_deref() != Some(raw_resource_id.as_str())
        || resource_project_id.as_deref() != Some(expected_project_id.to_string().as_str())
    {
        return Err(InvalidPersistedResource::new(
            Some(resource_id),
            PersistedResourceField::ProjectId,
            PersistedSecurityReason::UnknownValue,
        )
        .into());
    }
    let sequence: i64 = row.try_get("sequence")?;
    if sequence < 0 {
        return Err(InvalidPersistedResource::new(
            Some(resource_id),
            PersistedResourceField::ChangeSequence,
            PersistedSecurityReason::InvalidInteger,
        )
        .into());
    }
    let effective_user_id = row
        .try_get::<Option<String>, _>("effective_user_id")?
        .map(|value| {
            Uuid::parse_str(&value).map_err(|_| {
                InvalidPersistedResource::new(
                    Some(resource_id),
                    PersistedResourceField::ChangeAudience,
                    PersistedSecurityReason::InvalidUuid,
                )
            })
        })
        .transpose()?;
    Ok((sequence, resource_id, effective_user_id))
}

async fn release_pointer_integrity_error(
    pool: &Pool<Any>,
    resource_id: Uuid,
    version_id: &str,
    expected_channel: &str,
) -> Result<InvalidPersistedResource, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT resource.project_id AS expected_project_id,
               version.project_id AS version_project_id,
               version.resource_id AS version_resource_id,
               version.status AS version_status,
               version.release_channel AS version_release_channel
        FROM resources resource
        LEFT JOIN resource_versions version ON version.id = ?
        WHERE resource.id = ?
        "#,
    )
    .bind(version_id)
    .bind(resource_id.to_string())
    .fetch_one(pool)
    .await?;
    let expected_project_id: String = row.try_get("expected_project_id")?;
    let version_project_id: Option<String> = row.try_get("version_project_id")?;
    let version_resource_id: Option<String> = row.try_get("version_resource_id")?;
    let version_status: Option<String> = row.try_get("version_status")?;
    let version_release_channel: Option<String> = row.try_get("version_release_channel")?;
    let expected_resource_id = resource_id.to_string();
    let field = if version_project_id.as_deref() != Some(expected_project_id.as_str())
        || version_resource_id.as_deref() != Some(expected_resource_id.as_str())
    {
        PersistedResourceField::VersionId
    } else if version_release_channel
        .as_deref()
        .and_then(ReleaseChannel::parse)
        .is_none()
        || version_release_channel.as_deref() != Some(expected_channel)
    {
        PersistedResourceField::ReleaseChannel
    } else if version_status.as_deref() != Some(expected_channel) {
        PersistedResourceField::VersionStatus
    } else {
        PersistedResourceField::VersionId
    };
    Ok(InvalidPersistedResource::new(
        Some(resource_id),
        field,
        PersistedSecurityReason::UnknownValue,
    ))
}

fn map_resources(rows: Vec<sqlx::any::AnyRow>) -> StorageResult<Vec<ManagedResource>> {
    rows.into_iter().map(|row| map_resource(&row)).collect()
}

fn map_effective_version(row: sqlx::any::AnyRow) -> StorageResult<EffectiveResourceVersion> {
    let resource_id_raw: String = row.try_get("resource_id").map_err(|error| {
        persisted_resource_column_error(
            error,
            None,
            PersistedResourceField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let resource_id = Uuid::parse_str(&resource_id_raw).map_err(|_| {
        InvalidPersistedResource::new(
            None,
            PersistedResourceField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let invalid = |field, reason| InvalidPersistedResource::new(Some(resource_id), field, reason);

    let project_id_raw: String = row.try_get("project_id").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::ProjectId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let project_id = Uuid::parse_str(&project_id_raw).map_err(|_| {
        invalid(
            PersistedResourceField::ProjectId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;

    let version_id_raw: String = row.try_get("version_id").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::VersionId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let version_id = Uuid::parse_str(&version_id_raw).map_err(|_| {
        invalid(
            PersistedResourceField::VersionId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;

    let kind_raw: String = row.try_get("kind").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::Kind,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let kind = ResourceKind::parse(&kind_raw).ok_or_else(|| {
        invalid(
            PersistedResourceField::Kind,
            PersistedSecurityReason::UnknownValue,
        )
    })?;

    let release_channel_raw: Option<String> = row.try_get("release_channel").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::ReleaseChannel,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let release_channel = release_channel_raw
        .as_deref()
        .and_then(ReleaseChannel::parse)
        .ok_or_else(|| {
            invalid(
                PersistedResourceField::ReleaseChannel,
                PersistedSecurityReason::UnknownValue,
            )
        })?;

    let payload_raw: String = row.try_get("payload").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::Payload,
            PersistedSecurityReason::MalformedPayload,
        )
    })?;
    let payload = serde_json::from_str(&payload_raw)
        .map(canonicalize_resource_payload)
        .map_err(|_| {
            invalid(
                PersistedResourceField::Payload,
                PersistedSecurityReason::MalformedPayload,
            )
        })?;
    let bundle = effective_bundle_from_payload(&payload, resource_id)?;

    let content_size: i64 = row.try_get("content_size").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::ContentSize,
            PersistedSecurityReason::InvalidInteger,
        )
    })?;
    let size = content_size.try_into().map_err(|_| {
        invalid(
            PersistedResourceField::ContentSize,
            PersistedSecurityReason::InvalidInteger,
        )
    })?;

    let slug: String = row.try_get("slug")?;
    let version: String = row.try_get("version")?;
    let sha256: String = row.try_get("content_sha256")?;
    if let Some(bundle) = bundle.as_ref() {
        let expected_bundle_kind = conductor_domain::ResourceBundleKind::from_resource_kind(kind);
        if expected_bundle_kind != Some(bundle.kind)
            || bundle.slug != slug
            || bundle.version != version
            || bundle.artifact_sha256 != sha256
            || bundle.artifact_size != size
        {
            return Err(invalid(
                PersistedResourceField::Payload,
                PersistedSecurityReason::MalformedPayload,
            )
            .into());
        }
    }

    Ok(EffectiveResourceVersion {
        project_id,
        resource_id,
        version_id,
        kind,
        slug,
        version,
        description: row.try_get("description")?,
        changelog: row.try_get("changelog")?,
        version_history: Vec::new(),
        release_channel,
        payload,
        sha256,
        size,
        artifact_key: row.try_get("artifact_key")?,
        bundle,
        minimum_evoflux_version: row.try_get("minimum_evoflux_version")?,
    })
}

fn map_version_notice(
    row: sqlx::any::AnyRow,
    resource_id: Uuid,
) -> StorageResult<ResourceVersionNotice> {
    let invalid = |field, reason| InvalidPersistedResource::new(Some(resource_id), field, reason);
    let version_id_raw: String = row.try_get("id").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::VersionId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let version_id = Uuid::parse_str(&version_id_raw).map_err(|_| {
        invalid(
            PersistedResourceField::VersionId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;

    let status_raw: String = row.try_get("status").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::VersionStatus,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let status = match status_raw.as_str() {
        "draft" => ResourceVersionStatus::Draft,
        "beta" => ResourceVersionStatus::Beta,
        "published" => ResourceVersionStatus::Published,
        "deprecated" => ResourceVersionStatus::Deprecated,
        _ => {
            return Err(invalid(
                PersistedResourceField::VersionStatus,
                PersistedSecurityReason::UnknownValue,
            )
            .into())
        }
    };

    let release_channel_raw: Option<String> = row.try_get("release_channel").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::ReleaseChannel,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let release_channel = release_channel_raw
        .as_deref()
        .and_then(ReleaseChannel::parse)
        .ok_or_else(|| {
            invalid(
                PersistedResourceField::ReleaseChannel,
                PersistedSecurityReason::UnknownValue,
            )
        })?;

    let published_at_raw: Option<String> = row.try_get("published_at").map_err(|error| {
        persisted_resource_column_error(
            error,
            Some(resource_id),
            PersistedResourceField::PublishedAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let published_at = published_at_raw
        .map(|value| {
            parse_effective_resource_dt(&value, resource_id, PersistedResourceField::PublishedAt)
        })
        .transpose()?;

    Ok(ResourceVersionNotice {
        version_id,
        version: row.try_get("version")?,
        status,
        release_channel,
        changelog: row.try_get("changelog")?,
        published_at,
        deprecation_reason: row.try_get("deprecation_reason")?,
    })
}

fn effective_bundle_from_payload(
    payload: &serde_json::Value,
    resource_id: Uuid,
) -> StorageResult<Option<ResourceBundle>> {
    let Some(value) = payload.get("bundle").cloned() else {
        return Ok(None);
    };
    let bundle: ResourceBundle = serde_json::from_value(value).map_err(|_| {
        InvalidPersistedResource::new(
            Some(resource_id),
            PersistedResourceField::Payload,
            PersistedSecurityReason::MalformedPayload,
        )
    })?;
    if bundle.schema_version != ResourceBundle::SCHEMA_VERSION {
        return Err(InvalidPersistedResource::new(
            Some(resource_id),
            PersistedResourceField::Payload,
            PersistedSecurityReason::MalformedPayload,
        )
        .into());
    }
    Ok(Some(bundle))
}

fn persisted_resource_column_error(
    error: sqlx::Error,
    resource_id: Option<Uuid>,
    field: PersistedResourceField,
    reason: PersistedSecurityReason,
) -> StorageError {
    match error {
        sqlx::Error::ColumnDecode { .. } => {
            InvalidPersistedResource::new(resource_id, field, reason).into()
        }
        operational => StorageError::Database(operational),
    }
}

fn parse_effective_resource_dt(
    value: &str,
    resource_id: Uuid,
    field: PersistedResourceField,
) -> Result<DateTime<Utc>, InvalidPersistedResource> {
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc))
        .map_err(|_| {
            InvalidPersistedResource::new(
                Some(resource_id),
                field,
                PersistedSecurityReason::InvalidTimestamp,
            )
        })
}

fn map_version(row: sqlx::any::AnyRow) -> ResourceVersion {
    let payload = canonicalize_resource_payload(
        serde_json::from_str(row.get::<String, _>("payload").as_str())
            .unwrap_or_else(|_| serde_json::json!({})),
    );
    let bundle = bundle_from_payload(&payload);
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
        bundle,
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

fn bundle_from_payload(payload: &serde_json::Value) -> Option<ResourceBundle> {
    payload
        .get("bundle")
        // Releases created before the canonical field rename remain readable.
        .or_else(|| payload.get("bundle_v2"))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .filter(|bundle: &ResourceBundle| bundle.schema_version == ResourceBundle::SCHEMA_VERSION)
}

#[cfg(test)]
mod bundle_payload_tests {
    use super::*;

    #[test]
    fn reads_legacy_bundle_key_without_reemitting_it() {
        let legacy = serde_json::json!({
            "bundle_v2": {
                "schema_version": 2,
                "kind": "agent",
                "slug": "reviewer",
                "version": "1.0.0",
                "artifact_sha256": "a".repeat(64),
                "artifact_size": 10,
                "artifact_media_type": "application/vnd.evoflux.resource+zip",
                "tree_sha256": "b".repeat(64),
                "files": []
            }
        });

        let bundle = bundle_from_payload(&legacy).expect("legacy bundle remains readable");
        assert_eq!(bundle.slug, "reviewer");
        let canonical = canonicalize_resource_payload(legacy);
        assert!(canonical.get("bundle").is_some());
        assert!(canonical.get("bundle_v2").is_none());
        assert!(serde_json::to_value(bundle)
            .unwrap()
            .get("bundle_v2")
            .is_none());
    }
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

/// Resolve the exact active-client audience for a resource while the caller's
/// transaction still observes the relevant pre- or post-mutation policy.
async fn visible_user_ids_for_resource(
    tx: &mut sqlx::Transaction<'_, Any>,
    project_id: &str,
    resource_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT u.id
        FROM users u
        JOIN resources r ON r.id = ? AND r.project_id = ?
        WHERE u.status = 'active'
          AND r.status IN ('beta', 'published')
          AND (
            r.status <> 'beta'
            OR EXISTS (
                SELECT 1 FROM resource_beta_members beta_audience
                WHERE beta_audience.project_id = r.project_id
                  AND beta_audience.resource_id = r.id
                  AND beta_audience.user_id = u.id
            )
          )
          AND (
            r.owner_user_id = u.id
            OR u.primary_role = 'admin'
            OR (r.visibility = 'shared' AND NOT EXISTS (
                SELECT 1 FROM resource_access_rules any_rule
                WHERE any_rule.resource_id = r.id
            ))
            OR EXISTS (
                SELECT 1 FROM resource_access_rules rule
                WHERE rule.resource_id = r.id
                  AND rule.project_id = r.project_id
                  AND rule.effect = 'allow'
                  AND (
                    (rule.subject_type = 'all' AND rule.subject_id = '*')
                    OR (rule.subject_type = 'member' AND rule.subject_id = u.id)
                    OR (rule.subject_type = 'primary_role' AND rule.subject_id = u.primary_role)
                    OR (rule.subject_type = 'sub_role' AND rule.subject_id IN (
                        SELECT assignment.sub_role_id
                        FROM user_sub_roles assignment
                        JOIN sub_roles role ON role.id = assignment.sub_role_id
                        WHERE assignment.user_id = u.id
                    ))
                    OR (rule.subject_type = 'tag' AND rule.subject_id IN (
                        SELECT assignment.tag_id
                        FROM tag_assignments assignment
                        JOIN tags tag ON tag.id = assignment.tag_id
                        WHERE assignment.entity_type = 'member'
                          AND assignment.entity_id = u.id
                    ))
                  )
            )
          )
        "#,
    )
    .bind(resource_id.to_string())
    .bind(project_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| row.try_get::<String, _>("id"))
        .collect()
}

struct InventoryVersionFacts {
    release_channel: Option<String>,
    content_sha256: String,
}

#[derive(Clone, Copy)]
struct InventoryVersionColumns {
    id: &'static str,
    resolved_id: &'static str,
    project_id: &'static str,
    resource_id: &'static str,
    status: &'static str,
    version: &'static str,
}

fn map_inventory_version_reference(
    row: &sqlx::any::AnyRow,
    expected_project_id: Uuid,
    expected_resource_id: Uuid,
    columns: InventoryVersionColumns,
    field: PersistedResourceField,
) -> StorageResult<(Option<Uuid>, Option<String>)> {
    let invalid = |reason| InvalidPersistedResource::new(Some(expected_resource_id), field, reason);
    let raw_id: Option<String> = row.try_get(columns.id)?;
    let resolved_id: Option<String> = row.try_get(columns.resolved_id)?;
    let project_id: Option<String> = row.try_get(columns.project_id)?;
    let resource_id: Option<String> = row.try_get(columns.resource_id)?;
    let status: Option<String> = row.try_get(columns.status)?;
    let version: Option<String> = row.try_get(columns.version)?;
    let Some(raw_id) = raw_id else {
        if resolved_id.is_some()
            || project_id.is_some()
            || resource_id.is_some()
            || status.is_some()
            || version.is_some()
        {
            return Err(invalid(PersistedSecurityReason::UnknownValue).into());
        }
        return Ok((None, None));
    };
    let version_id =
        Uuid::parse_str(&raw_id).map_err(|_| invalid(PersistedSecurityReason::InvalidUuid))?;
    let expected_project_id = expected_project_id.to_string();
    let expected_resource_id = expected_resource_id.to_string();
    if resolved_id.as_deref() != Some(raw_id.as_str())
        || project_id.as_deref() != Some(expected_project_id.as_str())
        || resource_id.as_deref() != Some(expected_resource_id.as_str())
        || !matches!(status.as_deref(), Some("beta" | "published" | "deprecated"))
        || version.as_deref().is_none_or(str::is_empty)
    {
        return Err(invalid(PersistedSecurityReason::UnknownValue).into());
    }
    Ok((Some(version_id), version))
}

async fn inventory_version_facts(
    tx: &mut sqlx::Transaction<'_, Any>,
    project_id: Uuid,
    resource_id: Uuid,
    version_id: Option<Uuid>,
) -> Result<Option<InventoryVersionFacts>, sqlx::Error> {
    let Some(version_id) = version_id else {
        return Ok(None);
    };
    let row = sqlx::query(
        r#"
        SELECT release_channel, content_sha256
        FROM resource_versions
        WHERE id = ? AND project_id = ? AND resource_id = ? AND status <> 'draft'
        "#,
    )
    .bind(version_id.to_string())
    .bind(project_id.to_string())
    .bind(resource_id.to_string())
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| {
        Ok(InventoryVersionFacts {
            release_channel: row.try_get("release_channel")?,
            content_sha256: row.try_get("content_sha256")?,
        })
    })
    .transpose()
}

async fn insert_access_rule(
    tx: &mut sqlx::Transaction<'_, Any>,
    project_id: &str,
    resource_id: Uuid,
    subject_type: &str,
    subject_id: &str,
    created_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO resource_access_rules (project_id, resource_id, subject_type, subject_id, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(project_id)
    .bind(resource_id.to_string())
    .bind(subject_type)
    .bind(subject_id)
    .bind(created_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

struct ResourceChangeInsert<'a> {
    effective_user_id: Option<&'a str>,
    change_kind: &'a str,
    version_id: Option<Uuid>,
    channel: Option<&'a str>,
    created_at: &'a str,
}

async fn insert_resource_change(
    tx: &mut sqlx::Transaction<'_, Any>,
    project_id: &str,
    resource_id: Uuid,
    change: ResourceChangeInsert<'_>,
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
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(next_sequence)
    .bind(project_id)
    .bind(resource_id.to_string())
    .bind(change.effective_user_id)
    .bind(change.change_kind)
    .bind(change.version_id.map(|value| value.to_string()))
    .bind(change.channel)
    .bind(change.created_at)
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
