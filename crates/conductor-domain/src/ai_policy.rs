//! Per-project AI provider/tool allow-list policy.
//!
//! Lives alongside [`crate::spend`]: same authorization model, same
//! `(scope, subject_id)` convention — a dollar ceiling there, an allow/deny
//! list here. Only `Project` and `Role` scopes exist (no per-member scope);
//! narrower-than-role targeting was explicitly deferred.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPolicyScope {
    /// Everyone in the project.
    Project,
    /// Everyone holding a primary role — narrows, never widens, past what
    /// the project-scope row already allows.
    Role,
}

impl AiPolicyScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Role => "role",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "project" => Some(Self::Project),
            "role" => Some(Self::Role),
            _ => None,
        }
    }

    pub fn requires_subject(self) -> bool {
        matches!(self, Self::Role)
    }
}

/// Create or replace one policy row. A replace, not a patch — every field is
/// written, matching [`crate::spend::UpsertSpendLimitRequest`]'s own
/// rationale: a caller that sends half a policy gets a whole one it can see.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpsertAiPolicyRequest {
    pub scope: AiPolicyScope,
    /// Primary role for a role policy; absent or empty for a project policy,
    /// which covers everyone.
    #[serde(default)]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    /// Empty means unrestricted — no row, or an empty list, both mean "every
    /// provider is allowed," never "none are."
    #[serde(default)]
    pub allowed_providers: Vec<String>,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

/// Which policy row to remove.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiPolicySelector {
    pub scope: AiPolicyScope,
    #[serde(default)]
    pub subject_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPolicyView {
    pub scope: AiPolicyScope,
    pub subject_id: String,
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub allowed_providers: Vec<String>,
    pub allowed_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPolicyListResponse {
    pub policies: Vec<AiPolicyView>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_round_trips_through_its_string_form() {
        for scope in [AiPolicyScope::Project, AiPolicyScope::Role] {
            assert_eq!(AiPolicyScope::parse(scope.as_str()), Some(scope));
        }
        assert_eq!(AiPolicyScope::parse("member"), None);
    }

    #[test]
    fn only_role_scope_names_a_subject() {
        assert!(!AiPolicyScope::Project.requires_subject());
        assert!(AiPolicyScope::Role.requires_subject());
    }
}
