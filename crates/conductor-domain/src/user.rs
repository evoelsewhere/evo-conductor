use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::role::PrimaryRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub primary_role: PrimaryRole,
    pub sub_role_ids: Vec<String>,
    pub tag_ids: Vec<String>,
    pub status: UserStatus,
    #[serde(default)]
    pub must_change_password: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    /// SSO first login — waiting for admin approval.
    Pending,
    /// Admin pre-provisioned with a temporary password.
    Invited,
    Active,
    Disabled,
}

impl UserStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Invited => "invited",
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "pending" => Self::Pending,
            "invited" => Self::Invited,
            "disabled" => Self::Disabled,
            _ => Self::Active,
        }
    }

    pub fn can_authenticate(self) -> bool {
        matches!(self, Self::Active | Self::Invited)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub token: String,
    pub user: User,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemberRequest {
    pub email: String,
    pub display_name: String,
    pub primary_role: PrimaryRole,
    #[serde(default)]
    pub sub_role_ids: Vec<String>,
    #[serde(default)]
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedMember {
    pub user: User,
    /// Shown once — never stored in plaintext.
    pub temporary_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemberRequest {
    pub display_name: Option<String>,
    pub primary_role: Option<PrimaryRole>,
    pub sub_role_ids: Option<Vec<String>>,
    pub tag_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveMemberRequest {
    pub primary_role: Option<PrimaryRole>,
    pub sub_role_ids: Option<Vec<String>>,
    pub tag_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetPasswordResponse {
    pub temporary_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: Option<String>,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberListResponse {
    pub items: Vec<User>,
    pub total: u64,
    pub page: u32,
    pub limit: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [UserStatus; 4] = [
        UserStatus::Pending,
        UserStatus::Invited,
        UserStatus::Active,
        UserStatus::Disabled,
    ];

    #[test]
    fn as_str_and_parse_round_trip() {
        for status in ALL {
            assert_eq!(UserStatus::parse(status.as_str()), status);
        }
    }

    /// `parse` falls back to `Active` for anything it does not recognise. That
    /// is a permissive default on a security-relevant field, so it is asserted
    /// deliberately rather than left implicit.
    #[test]
    fn parse_falls_back_to_active_for_unknown_values() {
        for value in ["", "unknown", "ACTIVE", "deleted"] {
            assert_eq!(
                UserStatus::parse(value),
                UserStatus::Active,
                "unexpected fallback for {value:?}"
            );
        }
    }

    #[test]
    fn only_active_and_invited_may_authenticate() {
        assert!(UserStatus::Active.can_authenticate());
        assert!(UserStatus::Invited.can_authenticate());
        assert!(!UserStatus::Pending.can_authenticate());
        assert!(!UserStatus::Disabled.can_authenticate());
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemberListQuery {
    pub q: Option<String>,
    pub status: Option<UserStatus>,
    pub role: Option<PrimaryRole>,
    pub tag: Option<String>,
    pub page: u32,
    pub limit: u32,
    /// When true, restrict to active members only (non-admin list).
    pub active_only: bool,
}
