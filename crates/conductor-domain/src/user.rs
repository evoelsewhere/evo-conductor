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
    pub status: UserStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Invited,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub token: String,
    pub user: User,
    pub expires_at: DateTime<Utc>,
}
