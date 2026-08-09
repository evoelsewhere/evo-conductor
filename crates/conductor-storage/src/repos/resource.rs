use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use conductor_domain::{
    CreateResourceRequest, CreateResourceVersionRequest, ManagedResource, PrimaryRole,
    ResourceAccessPolicy, ResourceDailyUsage, ResourceFeedback, ResourceMemberUsage,
    ResourceMonitoring, ResourceMonitoringSummary, ResourceUsageEventRequest, ResourceVersion,
    ResourceVersionStatus, UpdateResourceRequest, UpsertResourceFeedbackRequest,
};
use sqlx::{Any, Pool, QueryBuilder, Row};
use uuid::Uuid;

use crate::core::mapping::{map_resource, parse_dt};

#[derive(Clone)]
pub struct ResourceRepo {
    pool: Pool<Any>,
}

impl ResourceRepo {
    pub fn new(pool: Pool<Any>) -> Self {
        Self { pool }
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
            SELECT r.id, r.kind, r.slug, r.name, r.description, r.version,
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
            SELECT id, kind, slug, name, description, version, owner_user_id,
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
            visible_resources_query("SELECT r.id, r.kind, r.slug, r.name, r.description, r.version, r.owner_user_id, r.visibility, r.status, r.payload, r.published_at, r.created_at, r.updated_at")
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
            SELECT id, kind, slug, name, description, version, owner_user_id,
                   visibility, status, payload, published_at, created_at, updated_at
            FROM resources WHERE id = ?
            "#,
        )
        .bind(resource_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| map_resource(&row)).transpose()
    }

    pub async fn create(
        &self,
        request: &CreateResourceRequest,
        owner_user_id: Uuid,
    ) -> Result<ManagedResource, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let id = Uuid::new_v4();
        let version_id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let payload = serde_json::to_string(&request.payload).unwrap_or_else(|_| "{}".into());

        sqlx::query(
            r#"
            INSERT INTO resources (
                id, kind, slug, name, description, version, owner_user_id,
                visibility, status, payload, published_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, NULL, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(request.kind.as_str())
        .bind(request.slug.trim())
        .bind(request.name.trim())
        .bind(clean_optional(request.description.as_deref()))
        .bind(request.version.trim())
        .bind(owner_user_id.to_string())
        .bind(request.visibility.as_str())
        .bind(&payload)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO resource_versions (
                id, resource_id, version, status, payload, changelog,
                created_by, created_at, published_at
            ) VALUES (?, ?, ?, 'draft', ?, ?, ?, ?, NULL)
            "#,
        )
        .bind(version_id.to_string())
        .bind(id.to_string())
        .bind(request.version.trim())
        .bind(payload)
        .bind(clean_optional(request.changelog.as_deref()))
        .bind(owner_user_id.to_string())
        .bind(now)
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
        let result = sqlx::query(
            "UPDATE resources SET status = 'archived', updated_at = ? WHERE id = ? AND status <> 'archived'",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(resource_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn versions(&self, resource_id: Uuid) -> Result<Vec<ResourceVersion>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, resource_id, version, status, payload, changelog,
                   created_by, created_at, published_at
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

    pub async fn create_version(
        &self,
        resource_id: Uuid,
        request: &CreateResourceVersionRequest,
        created_by: Uuid,
    ) -> Result<ResourceVersion, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO resource_versions (
                id, resource_id, version, status, payload, changelog,
                created_by, created_at, published_at
            ) VALUES (?, ?, ?, 'draft', ?, ?, ?, ?, NULL)
            "#,
        )
        .bind(id.to_string())
        .bind(resource_id.to_string())
        .bind(request.version.trim())
        .bind(serde_json::to_string(&request.payload).unwrap_or_else(|_| "{}".into()))
        .bind(clean_optional(request.changelog.as_deref()))
        .bind(created_by.to_string())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(ResourceVersion {
            id,
            resource_id,
            version: request.version.trim().to_string(),
            status: ResourceVersionStatus::Draft,
            payload: request.payload.clone(),
            changelog: clean_optional(request.changelog.as_deref()),
            created_by,
            created_at: now,
            published_at: None,
        })
    }

    pub async fn publish_version(
        &self,
        resource_id: Uuid,
        version_id: Uuid,
    ) -> Result<Option<ManagedResource>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT rv.version, rv.payload, r.updated_at
            FROM resource_versions rv
            JOIN resources r ON r.id = rv.resource_id
            WHERE rv.id = ? AND rv.resource_id = ? AND rv.status = 'draft'
            "#,
        )
        .bind(version_id.to_string())
        .bind(resource_id.to_string())
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let version: String = row.get("version");
        let payload: String = row.get("payload");
        let previous_updated_at: String = row.get("updated_at");
        let now = Utc::now().to_rfc3339();

        let updated = sqlx::query(
            r#"
            UPDATE resources
            SET version = ?, payload = ?, status = 'published',
                published_at = ?, updated_at = ?
            WHERE id = ? AND updated_at = ?
            "#,
        )
        .bind(version)
        .bind(payload)
        .bind(&now)
        .bind(&now)
        .bind(resource_id.to_string())
        .bind(previous_updated_at)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        sqlx::query(
            "UPDATE resource_versions SET status = 'deprecated' WHERE resource_id = ? AND status = 'published'",
        )
        .bind(resource_id.to_string())
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE resource_versions SET status = 'published', published_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(version_id.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.find_by_id(resource_id).await
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
        tx.commit().await?;
        Ok(())
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
        WHERE r.status = 'published' AND (
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

fn map_version(row: sqlx::any::AnyRow) -> ResourceVersion {
    ResourceVersion {
        id: parse_uuid(row.get("id")),
        resource_id: parse_uuid(row.get("resource_id")),
        version: row.get("version"),
        status: ResourceVersionStatus::parse(row.get::<String, _>("status").as_str()),
        payload: serde_json::from_str(row.get::<String, _>("payload").as_str())
            .unwrap_or_else(|_| serde_json::json!({})),
        changelog: row.get("changelog"),
        created_by: parse_uuid(row.get("created_by")),
        created_at: parse_dt(row.get("created_at")),
        published_at: row.get::<Option<String>, _>("published_at").map(parse_dt),
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
