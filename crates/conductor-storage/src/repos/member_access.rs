use std::collections::HashSet;

use chrono::Utc;
use conductor_domain::{scope_is_role_compatible, PrimaryRole, SecretScope, User, UserStatus};
use sqlx::{Any, AnyConnection, Pool, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::core::dialect::DatabaseKind;
use crate::core::error::{
    InvalidPersistedCredential, InvalidPersistedPrincipal, PersistedCredentialField,
    PersistedPrincipalField, PersistedSecurityReason, StorageError,
};
use crate::core::mapping::map_user_row;
use crate::repos::user::{
    replace_sub_roles_on, replace_tags_on, sub_role_ids_for_on, tag_ids_for_on, USER_SELECT,
};

const LAST_ADMIN_MESSAGE: &str = "the project must keep at least one active admin";

#[derive(Debug, Clone)]
pub struct UpdateAccessProfile {
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub display_name: Option<String>,
    pub primary_role: Option<PrimaryRole>,
    pub sub_role_ids: Option<Vec<String>>,
    pub tag_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct ApproveMemberAccess {
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub primary_role: Option<PrimaryRole>,
    pub sub_role_ids: Option<Vec<String>>,
    pub tag_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberStatusReason {
    AdministrativeDisable,
    AdministrativeEnable,
    AdministrativeApproval,
}

impl MemberStatusReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AdministrativeDisable => "administrative_disable",
            Self::AdministrativeEnable => "administrative_enable",
            Self::AdministrativeApproval => "administrative_approval",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ChangeMemberStatus {
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub target_status: UserStatus,
    pub reason: MemberStatusReason,
}

impl ChangeMemberStatus {
    pub fn disable(actor_id: Uuid, target_id: Uuid) -> Self {
        Self {
            actor_id,
            target_id,
            target_status: UserStatus::Disabled,
            reason: MemberStatusReason::AdministrativeDisable,
        }
    }

    pub fn enable(actor_id: Uuid, target_id: Uuid) -> Self {
        Self {
            actor_id,
            target_id,
            target_status: UserStatus::Active,
            reason: MemberStatusReason::AdministrativeEnable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberSecuritySnapshot {
    pub primary_role: PrimaryRole,
    pub status: UserStatus,
    pub sub_role_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub session_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialPolicyEffect {
    pub credential_id: Uuid,
    pub scopes: Vec<SecretScope>,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccessChange {
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub before: MemberSecuritySnapshot,
    pub after: MemberSecuritySnapshot,
    pub admin_elevation: bool,
    pub audience_changed: bool,
    pub status_reason: Option<MemberStatusReason>,
    pub revoked_credentials: Vec<CredentialPolicyEffect>,
}

#[derive(Debug, Clone)]
pub struct MemberAccessResult {
    pub user: User,
    pub change: MemberAccessChange,
}

#[derive(Debug, Error)]
pub enum MemberAccessError {
    #[error("database operation failed")]
    Database(#[source] sqlx::Error),
    #[error("project security state is not configured")]
    ProjectNotConfigured,
    #[error("requesting member was not found")]
    ActorNotFound,
    #[error("target member was not found")]
    TargetNotFound,
    #[error("requesting member is no longer authorized to manage members")]
    ActorNotAuthorized,
    #[error("you cannot change your own primary role")]
    SelfPrimaryRoleChange,
    #[error("cannot disable yourself")]
    SelfDisable,
    #[error("{LAST_ADMIN_MESSAGE}")]
    LastActiveAdmin,
    #[error("display_name cannot be empty")]
    EmptyDisplayName,
    #[error("sub_role_ids contains duplicates")]
    DuplicateSubRoleId,
    #[error("tag_ids contains duplicates")]
    DuplicateTagId,
    #[error("sub_role_ids contains an unknown role")]
    UnknownSubRoleId,
    #[error("tag_ids contains an unknown tag")]
    UnknownTagId,
    #[error("member status operation supports only active and disabled")]
    UnsupportedStatus,
    #[error("member is not pending approval")]
    NotPendingApproval,
    #[error(transparent)]
    InvalidPersistedPrincipal(InvalidPersistedPrincipal),
    #[error(transparent)]
    InvalidPersistedCredential(InvalidPersistedCredential),
}

impl From<sqlx::Error> for MemberAccessError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl From<StorageError> for MemberAccessError {
    fn from(value: StorageError) -> Self {
        match value {
            StorageError::Database(error) => Self::Database(error),
            StorageError::InvalidPersistedPrincipal(error) => {
                Self::InvalidPersistedPrincipal(error)
            }
            StorageError::InvalidPersistedCredential(error) => {
                Self::InvalidPersistedCredential(error)
            }
            StorageError::InvalidPersistedResource(_) | StorageError::Serialization(_) => {
                Self::Database(sqlx::Error::Protocol(
                    "unexpected storage error during member access update".into(),
                ))
            }
        }
    }
}

#[derive(Debug)]
struct LoadedMember {
    user: User,
    session_version: i64,
}

impl LoadedMember {
    fn snapshot(&self) -> MemberSecuritySnapshot {
        MemberSecuritySnapshot {
            primary_role: self.user.primary_role,
            status: self.user.status,
            sub_role_ids: self.user.sub_role_ids.clone(),
            tag_ids: self.user.tag_ids.clone(),
            session_version: self.session_version,
        }
    }
}

#[derive(Clone)]
pub struct MemberAccessRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl MemberAccessRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    pub async fn update_access_profile(
        &self,
        command: UpdateAccessProfile,
    ) -> Result<MemberAccessResult, MemberAccessError> {
        validate_profile_shape(&command)?;
        let mut tx = self.begin_security_transaction().await?;
        let result = self.update_access_profile_on(&mut tx, &command).await;
        finish_transaction(tx, result).await
    }

    pub async fn approve_member(
        &self,
        command: ApproveMemberAccess,
    ) -> Result<MemberAccessResult, MemberAccessError> {
        validate_assignment_shape(command.sub_role_ids.as_deref(), command.tag_ids.as_deref())?;
        let mut tx = self.begin_security_transaction().await?;
        let result = self.approve_member_on(&mut tx, &command).await;
        finish_transaction(tx, result).await
    }

    pub async fn set_member_status(
        &self,
        command: ChangeMemberStatus,
    ) -> Result<MemberAccessResult, MemberAccessError> {
        if !matches!(
            command.target_status,
            UserStatus::Active | UserStatus::Disabled
        ) || !matches!(
            (command.target_status, command.reason),
            (UserStatus::Active, MemberStatusReason::AdministrativeEnable)
                | (
                    UserStatus::Disabled,
                    MemberStatusReason::AdministrativeDisable
                )
        ) {
            return Err(MemberAccessError::UnsupportedStatus);
        }

        let mut tx = self.begin_security_transaction().await?;
        let result = self.set_member_status_on(&mut tx, command).await;
        finish_transaction(tx, result).await
    }

    async fn begin_security_transaction(
        &self,
    ) -> Result<Transaction<'static, Any>, MemberAccessError> {
        match self.kind {
            DatabaseKind::Sqlite => Ok(self.pool.begin_with("BEGIN IMMEDIATE").await?),
            DatabaseKind::Postgres | DatabaseKind::Mysql => Ok(self.pool.begin().await?),
        }
    }

    async fn update_access_profile_on(
        &self,
        tx: &mut Transaction<'_, Any>,
        command: &UpdateAccessProfile,
    ) -> Result<MemberAccessResult, MemberAccessError> {
        lock_project_security_state(&mut *tx, self.kind).await?;

        let actor = load_member_on(&mut *tx, command.actor_id)
            .await?
            .ok_or(MemberAccessError::ActorNotFound)?;
        let target = load_member_on(&mut *tx, command.target_id)
            .await?
            .ok_or(MemberAccessError::TargetNotFound)?;
        let before = target.snapshot();
        let next_role = command.primary_role.unwrap_or(target.user.primary_role);

        if command.actor_id == command.target_id && next_role != target.user.primary_role {
            return Err(MemberAccessError::SelfPrimaryRoleChange);
        }

        if target.user.primary_role == PrimaryRole::Admin
            && target.user.status == UserStatus::Active
            && next_role != PrimaryRole::Admin
            && active_admin_count_on(&mut *tx).await? <= 1
        {
            return Err(MemberAccessError::LastActiveAdmin);
        }

        ensure_active_admin(&actor)?;
        validate_assignment_rows(
            &mut *tx,
            command.sub_role_ids.as_deref(),
            command.tag_ids.as_deref(),
        )
        .await?;

        let admin_elevation =
            target.user.primary_role != PrimaryRole::Admin && next_role == PrimaryRole::Admin;
        let next_display_name = command
            .display_name
            .as_deref()
            .unwrap_or(&target.user.display_name);
        let session_increment = i64::from(admin_elevation);
        let update = sqlx::query(
            "UPDATE users SET display_name = ?, primary_role = ?, \
             session_version = session_version + ? WHERE id = ?",
        )
        .bind(next_display_name)
        .bind(next_role.as_str())
        .bind(session_increment)
        .bind(command.target_id.to_string())
        .execute(&mut **tx)
        .await?;
        if update.rows_affected() != 1 {
            return Err(MemberAccessError::TargetNotFound);
        }

        if let Some(sub_role_ids) = command.sub_role_ids.as_deref() {
            replace_sub_roles_on(&mut *tx, command.target_id, sub_role_ids).await?;
        }
        if let Some(tag_ids) = command.tag_ids.as_deref() {
            replace_tags_on(&mut *tx, command.target_id, tag_ids).await?;
        }

        let revoked_credentials =
            reconcile_credential_policy_on(&mut *tx, command.target_id, next_role).await?;
        let after_member = load_member_on(&mut *tx, command.target_id)
            .await?
            .ok_or(MemberAccessError::TargetNotFound)?;
        let after = after_member.snapshot();
        let audience_changed = before.primary_role != after.primary_role
            || before.sub_role_ids != after.sub_role_ids
            || before.tag_ids != after.tag_ids;

        Ok(MemberAccessResult {
            user: after_member.user,
            change: MemberAccessChange {
                actor_id: command.actor_id,
                target_id: command.target_id,
                before,
                after,
                admin_elevation,
                audience_changed,
                status_reason: None,
                revoked_credentials,
            },
        })
    }

    async fn set_member_status_on(
        &self,
        tx: &mut Transaction<'_, Any>,
        command: ChangeMemberStatus,
    ) -> Result<MemberAccessResult, MemberAccessError> {
        lock_project_security_state(&mut *tx, self.kind).await?;

        let actor = load_member_on(&mut *tx, command.actor_id)
            .await?
            .ok_or(MemberAccessError::ActorNotFound)?;
        let target = load_member_on(&mut *tx, command.target_id)
            .await?
            .ok_or(MemberAccessError::TargetNotFound)?;
        let before = target.snapshot();

        if command.actor_id == command.target_id
            && command.target_status == UserStatus::Disabled
            && target.user.status != UserStatus::Disabled
        {
            return Err(MemberAccessError::SelfDisable);
        }

        if target.user.primary_role == PrimaryRole::Admin
            && target.user.status == UserStatus::Active
            && command.target_status != UserStatus::Active
            && active_admin_count_on(&mut *tx).await? <= 1
        {
            return Err(MemberAccessError::LastActiveAdmin);
        }

        ensure_active_admin(&actor)?;
        let status_changed = target.user.status != command.target_status;
        if status_changed {
            let update = sqlx::query(
                "UPDATE users SET status = ?, session_version = session_version + 1 WHERE id = ?",
            )
            .bind(command.target_status.as_str())
            .bind(command.target_id.to_string())
            .execute(&mut **tx)
            .await?;
            if update.rows_affected() != 1 {
                return Err(MemberAccessError::TargetNotFound);
            }
        }

        let after_member = load_member_on(&mut *tx, command.target_id)
            .await?
            .ok_or(MemberAccessError::TargetNotFound)?;
        let after = after_member.snapshot();

        Ok(MemberAccessResult {
            user: after_member.user,
            change: MemberAccessChange {
                actor_id: command.actor_id,
                target_id: command.target_id,
                before,
                after,
                admin_elevation: false,
                audience_changed: status_changed,
                status_reason: Some(command.reason),
                // REQ-005 must extend this transaction to revoke durable
                // credentials on disable. REQ-004 deliberately does not do so.
                revoked_credentials: Vec::new(),
            },
        })
    }

    async fn approve_member_on(
        &self,
        tx: &mut Transaction<'_, Any>,
        command: &ApproveMemberAccess,
    ) -> Result<MemberAccessResult, MemberAccessError> {
        lock_project_security_state(&mut *tx, self.kind).await?;
        let actor = load_member_on(&mut *tx, command.actor_id)
            .await?
            .ok_or(MemberAccessError::ActorNotFound)?;
        let target = load_member_on(&mut *tx, command.target_id)
            .await?
            .ok_or(MemberAccessError::TargetNotFound)?;
        ensure_active_admin(&actor)?;
        if !matches!(
            target.user.status,
            UserStatus::Pending | UserStatus::Invited
        ) {
            return Err(MemberAccessError::NotPendingApproval);
        }
        validate_assignment_rows(
            &mut *tx,
            command.sub_role_ids.as_deref(),
            command.tag_ids.as_deref(),
        )
        .await?;

        let before = target.snapshot();
        let next_role = command.primary_role.unwrap_or(target.user.primary_role);
        let admin_elevation =
            target.user.primary_role != PrimaryRole::Admin && next_role == PrimaryRole::Admin;
        let now = Utc::now().to_rfc3339();
        let update = sqlx::query(
            "UPDATE users SET status = 'active', primary_role = ?, approved_at = ?, \
             approved_by = ?, must_change_password = 0, \
             session_version = session_version + 1 WHERE id = ?",
        )
        .bind(next_role.as_str())
        .bind(&now)
        .bind(command.actor_id.to_string())
        .bind(command.target_id.to_string())
        .execute(&mut **tx)
        .await?;
        if update.rows_affected() != 1 {
            return Err(MemberAccessError::TargetNotFound);
        }
        if let Some(sub_role_ids) = command.sub_role_ids.as_deref() {
            replace_sub_roles_on(&mut *tx, command.target_id, sub_role_ids).await?;
        }
        if let Some(tag_ids) = command.tag_ids.as_deref() {
            replace_tags_on(&mut *tx, command.target_id, tag_ids).await?;
        }
        let revoked_credentials =
            reconcile_credential_policy_on(&mut *tx, command.target_id, next_role).await?;
        let after_member = load_member_on(&mut *tx, command.target_id)
            .await?
            .ok_or(MemberAccessError::TargetNotFound)?;
        let after = after_member.snapshot();
        let audience_changed = before.primary_role != after.primary_role
            || before.status != after.status
            || before.sub_role_ids != after.sub_role_ids
            || before.tag_ids != after.tag_ids;

        Ok(MemberAccessResult {
            user: after_member.user,
            change: MemberAccessChange {
                actor_id: command.actor_id,
                target_id: command.target_id,
                before,
                after,
                admin_elevation,
                audience_changed,
                status_reason: Some(MemberStatusReason::AdministrativeApproval),
                revoked_credentials,
            },
        })
    }
}

fn validate_profile_shape(command: &UpdateAccessProfile) -> Result<(), MemberAccessError> {
    if command
        .display_name
        .as_ref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(MemberAccessError::EmptyDisplayName);
    }
    validate_assignment_shape(command.sub_role_ids.as_deref(), command.tag_ids.as_deref())
}

fn validate_assignment_shape(
    sub_role_ids: Option<&[String]>,
    tag_ids: Option<&[String]>,
) -> Result<(), MemberAccessError> {
    if sub_role_ids.is_some_and(|ids| !all_unique(ids)) {
        return Err(MemberAccessError::DuplicateSubRoleId);
    }
    if tag_ids.is_some_and(|ids| !all_unique(ids)) {
        return Err(MemberAccessError::DuplicateTagId);
    }
    Ok(())
}

fn all_unique(values: &[String]) -> bool {
    let mut seen = HashSet::with_capacity(values.len());
    values.iter().all(|value| seen.insert(value.as_str()))
}

async fn finish_transaction<T>(
    tx: Transaction<'_, Any>,
    result: Result<T, MemberAccessError>,
) -> Result<T, MemberAccessError> {
    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(error) => match tx.rollback().await {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(MemberAccessError::Database(rollback_error)),
        },
    }
}

async fn lock_project_security_state(
    connection: &mut AnyConnection,
    kind: DatabaseKind,
) -> Result<(), MemberAccessError> {
    let query = match kind {
        DatabaseKind::Sqlite => {
            // BEGIN IMMEDIATE already owns SQLite's database-wide writer
            // reservation before this read.
            "SELECT id FROM instance ORDER BY created_at ASC LIMIT 1"
        }
        DatabaseKind::Postgres | DatabaseKind::Mysql => {
            // Both shared-server dialects serialize security changes through
            // the singleton project row. PostgreSQL has a real two-connection
            // regression in this task; the equivalent MySQL path still needs
            // a disposable real-MySQL integration proof before claiming parity.
            "SELECT id FROM instance ORDER BY created_at ASC LIMIT 1 FOR UPDATE"
        }
    };
    let project_id: Option<String> = sqlx::query_scalar(query).fetch_optional(connection).await?;
    let project_id = project_id.ok_or(MemberAccessError::ProjectNotConfigured)?;
    Uuid::parse_str(&project_id).map_err(|_| {
        MemberAccessError::InvalidPersistedPrincipal(InvalidPersistedPrincipal::new(
            None,
            PersistedPrincipalField::ProjectId,
            PersistedSecurityReason::InvalidUuid,
        ))
    })?;
    Ok(())
}

async fn load_member_on(
    connection: &mut AnyConnection,
    id: Uuid,
) -> Result<Option<LoadedMember>, MemberAccessError> {
    let row = sqlx::query(&format!("{USER_SELECT} WHERE id = ?"))
        .bind(id.to_string())
        .fetch_optional(&mut *connection)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };

    let persisted_id: String = row.try_get("id").map_err(|_| {
        invalid_principal(
            None,
            PersistedPrincipalField::Id,
            PersistedSecurityReason::InvalidUuid,
        )
    })?;
    if Uuid::parse_str(&persisted_id).ok() != Some(id) {
        return Err(invalid_principal(
            None,
            PersistedPrincipalField::Id,
            PersistedSecurityReason::InvalidUuid,
        ));
    }
    let role: String = row.try_get("primary_role").map_err(|_| {
        invalid_principal(
            Some(id),
            PersistedPrincipalField::PrimaryRole,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    if PrimaryRole::parse(&role).is_none() {
        return Err(invalid_principal(
            Some(id),
            PersistedPrincipalField::PrimaryRole,
            PersistedSecurityReason::UnknownValue,
        ));
    }
    let status: String = row.try_get("status").map_err(|_| {
        invalid_principal(
            Some(id),
            PersistedPrincipalField::Status,
            PersistedSecurityReason::UnknownValue,
        )
    })?;
    if !matches!(
        status.as_str(),
        "pending" | "invited" | "active" | "disabled"
    ) {
        return Err(invalid_principal(
            Some(id),
            PersistedPrincipalField::Status,
            PersistedSecurityReason::UnknownValue,
        ));
    }
    let session_version: i64 = row.try_get("session_version").map_err(|_| {
        invalid_principal(
            Some(id),
            PersistedPrincipalField::SessionVersion,
            PersistedSecurityReason::InvalidInteger,
        )
    })?;
    if session_version < 0 {
        return Err(invalid_principal(
            Some(id),
            PersistedPrincipalField::SessionVersion,
            PersistedSecurityReason::InvalidInteger,
        ));
    }

    let mut user = map_user_row(&row).map_err(MemberAccessError::from)?;
    user.sub_role_ids = sub_role_ids_for_on(&mut *connection, id).await?;
    user.tag_ids = tag_ids_for_on(&mut *connection, id).await?;
    Ok(Some(LoadedMember {
        user,
        session_version,
    }))
}

fn invalid_principal(
    row_id: Option<Uuid>,
    field: PersistedPrincipalField,
    reason: PersistedSecurityReason,
) -> MemberAccessError {
    MemberAccessError::InvalidPersistedPrincipal(InvalidPersistedPrincipal::new(
        row_id, field, reason,
    ))
}

fn ensure_active_admin(actor: &LoadedMember) -> Result<(), MemberAccessError> {
    if actor.user.primary_role == PrimaryRole::Admin && actor.user.status == UserStatus::Active {
        Ok(())
    } else {
        Err(MemberAccessError::ActorNotAuthorized)
    }
}

async fn active_admin_count_on(connection: &mut AnyConnection) -> Result<i64, MemberAccessError> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE primary_role = 'admin' AND status = 'active'",
    )
    .fetch_one(connection)
    .await?)
}

async fn validate_assignment_rows(
    connection: &mut AnyConnection,
    sub_role_ids: Option<&[String]>,
    tag_ids: Option<&[String]>,
) -> Result<(), MemberAccessError> {
    if let Some(sub_role_ids) = sub_role_ids {
        for sub_role_id in sub_role_ids {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sub_roles WHERE id = ?")
                .bind(sub_role_id)
                .fetch_one(&mut *connection)
                .await?;
            if count != 1 {
                return Err(MemberAccessError::UnknownSubRoleId);
            }
        }
    }
    if let Some(tag_ids) = tag_ids {
        for tag_id in tag_ids {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = ?")
                .bind(tag_id)
                .fetch_one(&mut *connection)
                .await?;
            if count != 1 {
                return Err(MemberAccessError::UnknownTagId);
            }
        }
    }
    Ok(())
}

async fn reconcile_credential_policy_on(
    connection: &mut AnyConnection,
    owner_user_id: Uuid,
    next_role: PrimaryRole,
) -> Result<Vec<CredentialPolicyEffect>, MemberAccessError> {
    let rows = sqlx::query(
        "SELECT id, scopes FROM connection_secrets \
         WHERE owner_user_id = ? AND revoked_at IS NULL ORDER BY created_at ASC",
    )
    .bind(owner_user_id.to_string())
    .fetch_all(&mut *connection)
    .await?;
    let mut effects = Vec::new();
    for row in rows {
        let id: String = row.try_get("id").map_err(|_| {
            invalid_credential(
                None,
                PersistedCredentialField::Id,
                PersistedSecurityReason::InvalidUuid,
            )
        })?;
        let credential_id = Uuid::parse_str(&id).map_err(|_| {
            invalid_credential(
                None,
                PersistedCredentialField::Id,
                PersistedSecurityReason::InvalidUuid,
            )
        })?;
        let scopes_json: String = row.try_get("scopes").map_err(|_| {
            invalid_credential(
                Some(credential_id),
                PersistedCredentialField::Scopes,
                PersistedSecurityReason::MalformedPayload,
            )
        })?;
        let scopes: Vec<SecretScope> = serde_json::from_str(&scopes_json).map_err(|_| {
            invalid_credential(
                Some(credential_id),
                PersistedCredentialField::Scopes,
                PersistedSecurityReason::MalformedPayload,
            )
        })?;
        if scopes.is_empty() {
            return Err(invalid_credential(
                Some(credential_id),
                PersistedCredentialField::Scopes,
                PersistedSecurityReason::EmptyCollection,
            ));
        }
        if has_duplicate_scopes(&scopes) {
            return Err(invalid_credential(
                Some(credential_id),
                PersistedCredentialField::Scopes,
                PersistedSecurityReason::DuplicateValue,
            ));
        }
        if scopes
            .iter()
            .copied()
            .any(|scope| !scope_is_role_compatible(next_role, scope))
        {
            let update = sqlx::query(
                "UPDATE connection_secrets SET revoked_at = ? \
                 WHERE id = ? AND owner_user_id = ? AND revoked_at IS NULL",
            )
            .bind(Utc::now().to_rfc3339())
            .bind(credential_id.to_string())
            .bind(owner_user_id.to_string())
            .execute(&mut *connection)
            .await?;
            if update.rows_affected() != 1 {
                return Err(invalid_credential(
                    Some(credential_id),
                    PersistedCredentialField::RevokedAt,
                    PersistedSecurityReason::InvalidTimestamp,
                ));
            }
            effects.push(CredentialPolicyEffect {
                credential_id,
                scopes,
                reason: "scope_incompatible_with_primary_role",
            });
        }
    }
    Ok(effects)
}

fn invalid_credential(
    credential_id: Option<Uuid>,
    field: PersistedCredentialField,
    reason: PersistedSecurityReason,
) -> MemberAccessError {
    MemberAccessError::InvalidPersistedCredential(InvalidPersistedCredential::new(
        credential_id,
        field,
        reason,
    ))
}

fn has_duplicate_scopes(scopes: &[SecretScope]) -> bool {
    scopes
        .iter()
        .enumerate()
        .any(|(index, scope)| scopes[..index].contains(scope))
}
