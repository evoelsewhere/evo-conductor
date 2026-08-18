use chrono::{DateTime, Utc};
use conductor_domain::{
    ManagedResource, PrimaryRole, ResourceKind, ResourceStatus, ResourceVisibility, User,
    UserStatus,
};
use sqlx::any::AnyRow;
use sqlx::Row;
use uuid::Uuid;

use crate::core::error::{
    InvalidPersistedPrincipal, InvalidPersistedResource, PersistedPrincipalField,
    PersistedResourceField, PersistedSecurityReason, StorageError, StorageResult,
};

pub fn parse_dt(value: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// Normalizes legacy resource metadata at the read boundary so API responses
/// expose only the canonical `bundle` field without rewriting immutable rows.
pub fn canonicalize_resource_payload(mut payload: serde_json::Value) -> serde_json::Value {
    let serde_json::Value::Object(ref mut object) = payload else {
        return payload;
    };
    if !object.contains_key("bundle") {
        if let Some(bundle) = object.get("bundle_v2").cloned() {
            object.insert("bundle".into(), bundle);
        }
    }
    object.remove("bundle_v2");
    payload
}

pub fn map_user_row(r: &AnyRow) -> StorageResult<User> {
    let id_str: String = r.try_get("id").map_err(|error| {
        principal_column_error(
            error,
            None,
            PersistedPrincipalField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let id = Uuid::parse_str(&id_str).map_err(|_| {
        InvalidPersistedPrincipal::new(
            None,
            PersistedPrincipalField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let invalid = |field, reason| InvalidPersistedPrincipal::new(Some(id), field, reason);

    let role_str: String = r.try_get("primary_role").map_err(|error| {
        principal_column_error(
            error,
            Some(id),
            PersistedPrincipalField::PrimaryRole,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let primary_role = PrimaryRole::parse(&role_str).ok_or_else(|| {
        invalid(
            PersistedPrincipalField::PrimaryRole,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let status_str: String = r.try_get("status").map_err(|error| {
        principal_column_error(
            error,
            Some(id),
            PersistedPrincipalField::Status,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let status = UserStatus::parse(&status_str).ok_or_else(|| {
        invalid(
            PersistedPrincipalField::Status,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let session_version: i64 = r.try_get("session_version").map_err(|error| {
        principal_column_error(
            error,
            Some(id),
            PersistedPrincipalField::SessionVersion,
            PersistedSecurityReason::InvalidInteger,
        )
    })?;
    if session_version < 0 {
        return Err(invalid(
            PersistedPrincipalField::SessionVersion,
            PersistedSecurityReason::InvalidInteger,
        )
        .into());
    }
    let must_change: i64 = r.try_get("must_change_password").map_err(|error| {
        principal_column_error(
            error,
            Some(id),
            PersistedPrincipalField::MustChangePassword,
            PersistedSecurityReason::InvalidBoolean,
        )
    })?;
    if !matches!(must_change, 0 | 1) {
        return Err(invalid(
            PersistedPrincipalField::MustChangePassword,
            PersistedSecurityReason::InvalidBoolean,
        )
        .into());
    }
    let last_seen: Option<String> = r.try_get("last_seen_at").map_err(|error| {
        principal_column_error(
            error,
            Some(id),
            PersistedPrincipalField::LastSeenAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let last_seen_at = last_seen
        .map(|value| parse_persisted_principal_dt(&value, id, PersistedPrincipalField::LastSeenAt))
        .transpose()?;
    let created_at_raw: String = r.try_get("created_at").map_err(|error| {
        principal_column_error(
            error,
            Some(id),
            PersistedPrincipalField::CreatedAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let created_at =
        parse_persisted_principal_dt(&created_at_raw, id, PersistedPrincipalField::CreatedAt)?;

    Ok(User {
        id,
        email: r.try_get("email")?,
        display_name: r.try_get("display_name")?,
        primary_role,
        sub_role_ids: vec![],
        tag_ids: vec![],
        status,
        must_change_password: must_change == 1,
        last_seen_at,
        created_at,
    })
}

fn principal_column_error(
    error: sqlx::Error,
    row_id: Option<Uuid>,
    field: PersistedPrincipalField,
    reason: PersistedSecurityReason,
) -> StorageError {
    match error {
        sqlx::Error::ColumnDecode { .. } => {
            InvalidPersistedPrincipal::new(row_id, field, reason).into()
        }
        operational => StorageError::Database(operational),
    }
}

fn parse_persisted_principal_dt(
    value: &str,
    row_id: Uuid,
    field: PersistedPrincipalField,
) -> Result<DateTime<Utc>, InvalidPersistedPrincipal> {
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc))
        .map_err(|_| {
            InvalidPersistedPrincipal::new(
                Some(row_id),
                field,
                PersistedSecurityReason::InvalidTimestamp,
            )
        })
}

pub fn map_resource(r: &AnyRow) -> StorageResult<ManagedResource> {
    let id_raw: String = r.try_get("id").map_err(|error| {
        resource_column_error(
            error,
            None,
            PersistedResourceField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let id = Uuid::parse_str(&id_raw).map_err(|_| {
        InvalidPersistedResource::new(
            None,
            PersistedResourceField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let invalid = |field, reason| InvalidPersistedResource::new(Some(id), field, reason);

    let project_raw: String = r.try_get("project_id").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::ProjectId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let project_id = Uuid::parse_str(&project_raw).map_err(|_| {
        invalid(
            PersistedResourceField::ProjectId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;

    let kind_raw: String = r.try_get("kind").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
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

    let owner_raw: Option<String> = r.try_get("owner_user_id").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::OwnerUserId,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    let owner_user_id = owner_raw
        .map(|owner| {
            Uuid::parse_str(&owner).map_err(|_| {
                invalid(
                    PersistedResourceField::OwnerUserId,
                    PersistedSecurityReason::InvalidUuid,
                )
            })
        })
        .transpose()?;

    let visibility_raw: String = r.try_get("visibility").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::Visibility,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let visibility = match visibility_raw.as_str() {
        "shared" => ResourceVisibility::Shared,
        "private" => ResourceVisibility::Private,
        _ => {
            return Err(invalid(
                PersistedResourceField::Visibility,
                PersistedSecurityReason::UnknownValue,
            )
            .into())
        }
    };

    let status_raw: String = r.try_get("status").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::Status,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let status = match status_raw.as_str() {
        "draft" => ResourceStatus::Draft,
        "beta" => ResourceStatus::Beta,
        "published" => ResourceStatus::Published,
        "archived" => ResourceStatus::Archived,
        _ => {
            return Err(invalid(
                PersistedResourceField::Status,
                PersistedSecurityReason::UnknownValue,
            )
            .into())
        }
    };

    let payload_raw: String = r.try_get("payload").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::Payload,
            PersistedSecurityReason::MalformedPayload,
        )
    })?;
    let payload = serde_json::from_str(&payload_raw).map_err(|_| {
        invalid(
            PersistedResourceField::Payload,
            PersistedSecurityReason::MalformedPayload,
        )
    })?;

    let draft_revision: i64 = r.try_get("draft_revision").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::DraftRevision,
            PersistedSecurityReason::InvalidInteger,
        )
    })?;
    let draft_revision = draft_revision.try_into().map_err(|_| {
        invalid(
            PersistedResourceField::DraftRevision,
            PersistedSecurityReason::InvalidInteger,
        )
    })?;

    let release_channel_raw: Option<String> = r.try_get("release_channel").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::ReleaseChannel,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    let release_channel = release_channel_raw
        .map(|channel| {
            conductor_domain::ReleaseChannel::parse(&channel).ok_or_else(|| {
                invalid(
                    PersistedResourceField::ReleaseChannel,
                    PersistedSecurityReason::UnknownValue,
                )
            })
        })
        .transpose()?;

    let published_at_raw: Option<String> = r.try_get("published_at").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::PublishedAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let published_at = published_at_raw
        .map(|value| parse_persisted_resource_dt(&value, id, PersistedResourceField::PublishedAt))
        .transpose()?;
    let created_at_raw: String = r.try_get("created_at").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::CreatedAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let created_at =
        parse_persisted_resource_dt(&created_at_raw, id, PersistedResourceField::CreatedAt)?;
    let updated_at_raw: String = r.try_get("updated_at").map_err(|error| {
        resource_column_error(
            error,
            Some(id),
            PersistedResourceField::UpdatedAt,
            PersistedSecurityReason::InvalidTimestamp,
        )
    })?;
    let updated_at =
        parse_persisted_resource_dt(&updated_at_raw, id, PersistedResourceField::UpdatedAt)?;

    Ok(ManagedResource {
        id,
        project_id,
        kind,
        slug: r.try_get("slug")?,
        name: r.try_get("name")?,
        description: r.try_get("description")?,
        version: r.try_get("version")?,
        highest_version: r.try_get("highest_semver")?,
        draft_revision,
        release_channel,
        owner_user_id,
        visibility,
        status,
        payload: canonicalize_resource_payload(payload),
        published_at,
        created_at,
        updated_at,
    })
}

fn resource_column_error(
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

fn parse_persisted_resource_dt(
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
