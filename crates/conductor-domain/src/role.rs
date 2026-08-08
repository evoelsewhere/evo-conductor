use serde::{Deserialize, Serialize};

/// Primary org roles. Every member has exactly one primary role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrimaryRole {
    /// Full control: setup, SSO, roles, members, secrets policy, resources.
    Admin,
    /// Can publish/manage shared agents, skills, MCP and view team telemetry.
    Contribute,
    /// Standard member — consumes shared resources; may create personal secrets.
    User,
}

impl PrimaryRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Contribute => "contribute",
            Self::User => "user",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "admin" => Some(Self::Admin),
            "contribute" => Some(Self::Contribute),
            "user" => Some(Self::User),
            _ => None,
        }
    }

    pub fn can_manage_members(self) -> bool {
        matches!(self, Self::Admin)
    }

    pub fn can_manage_resources(self) -> bool {
        matches!(self, Self::Admin | Self::Contribute)
    }

    pub fn can_view_telemetry(self) -> bool {
        matches!(self, Self::Admin | Self::Contribute)
    }
}

/// Admin-defined project sub-role (e.g. `dev`, `ba`, `tester`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubRole {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubRoleRequest {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}
