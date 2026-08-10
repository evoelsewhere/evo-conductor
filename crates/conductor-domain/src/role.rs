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

    pub fn can_list_members(self) -> bool {
        matches!(self, Self::Admin | Self::Contribute)
    }

    pub fn can_manage_resources(self) -> bool {
        matches!(self, Self::Admin | Self::Contribute)
    }

    /// Tags are shared taxonomy for members, resources, and future entities.
    pub fn can_manage_tags(self) -> bool {
        matches!(self, Self::Admin | Self::Contribute)
    }

    pub fn can_view_telemetry(self) -> bool {
        matches!(self, Self::Admin | Self::Contribute)
    }

    pub fn can_manage_settings(self) -> bool {
        matches!(self, Self::Admin)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSubRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
}

/// Org / sub-team label (many-to-many with members).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTagRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    type CapabilityCase = (&'static str, fn(PrimaryRole) -> bool, [bool; 3]);

    const ALL: [PrimaryRole; 3] = [
        PrimaryRole::Admin,
        PrimaryRole::Contribute,
        PrimaryRole::User,
    ];

    #[test]
    fn as_str_and_parse_round_trip() {
        for role in ALL {
            assert_eq!(PrimaryRole::parse(role.as_str()), Some(role));
        }
    }

    #[test]
    fn parse_rejects_unknown_values() {
        for value in ["", "administrator", "Admin", "owner", "contribute "] {
            assert_eq!(PrimaryRole::parse(value), None, "accepted {value:?}");
        }
    }

    /// The capability matrix, stated once so a change to any predicate has to be
    /// deliberate. Columns are admin, contribute, user.
    #[test]
    fn capability_matrix() {
        let cases: [CapabilityCase; 6] = [
            (
                "manage_members",
                PrimaryRole::can_manage_members,
                [true, false, false],
            ),
            (
                "list_members",
                PrimaryRole::can_list_members,
                [true, true, false],
            ),
            (
                "manage_resources",
                PrimaryRole::can_manage_resources,
                [true, true, false],
            ),
            (
                "manage_tags",
                PrimaryRole::can_manage_tags,
                [true, true, false],
            ),
            (
                "view_telemetry",
                PrimaryRole::can_view_telemetry,
                [true, true, false],
            ),
            (
                "manage_settings",
                PrimaryRole::can_manage_settings,
                [true, false, false],
            ),
        ];

        for (name, predicate, expected) in cases {
            for (role, want) in ALL.into_iter().zip(expected) {
                assert_eq!(
                    predicate(role),
                    want,
                    "{name} for {} should be {want}",
                    role.as_str()
                );
            }
        }
    }

    /// A plain user must not hold any project-wide capability. Stated separately
    /// from the matrix because it is the property that matters, and a future
    /// predicate added without a matrix row would still be caught here.
    #[test]
    fn plain_user_holds_no_project_wide_capability() {
        let user = PrimaryRole::User;
        assert!(!user.can_manage_members());
        assert!(!user.can_list_members());
        assert!(!user.can_manage_resources());
        assert!(!user.can_manage_tags());
        assert!(!user.can_view_telemetry());
        assert!(!user.can_manage_settings());
    }
}
