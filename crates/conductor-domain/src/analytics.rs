use std::collections::HashSet;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{PrimaryRole, ResourceKind, TelemetryEventStatus, TelemetryResourceRelation};

pub const ANALYTICS_VIEW_SCHEMA_VERSION: u16 = 1;
pub const MAX_ANALYTICS_VIEW_NAME_LENGTH: usize = 80;
pub const MAX_ANALYTICS_VIEW_DESCRIPTION_LENGTH: usize = 500;
pub const MAX_ANALYTICS_WIDGETS: usize = 24;
pub const MAX_ANALYTICS_WIDGET_ID_LENGTH: usize = 64;
pub const MAX_ANALYTICS_WIDGET_TITLE_LENGTH: usize = 100;
pub const MAX_ANALYTICS_LABEL_LENGTH: usize = 120;
pub const MAX_ANALYTICS_BREAKDOWN_LIMIT: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsViewVisibility {
    Private,
    Shared,
}

impl AnalyticsViewVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Shared => "shared",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "shared" => Self::Shared,
            _ => Self::Private,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsDateRange {
    #[serde(rename = "last_24_hours", alias = "last24_hours")]
    Last24Hours,
    #[serde(rename = "last_7_days", alias = "last7_days")]
    Last7Days,
    #[default]
    #[serde(rename = "last_30_days", alias = "last30_days")]
    Last30Days,
    #[serde(rename = "last_90_days", alias = "last90_days")]
    Last90Days,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsComparison {
    PreviousPeriod,
    PreviousWeek,
    PreviousMonth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsDashboardPreset {
    Executive,
    Adoption,
    Reliability,
    Cost,
    #[default]
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsDashboardDensity {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsQuery {
    #[serde(default)]
    pub date_range: AnalyticsDateRange,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub comparison: Option<AnalyticsComparison>,
    pub member_id: Option<Uuid>,
    pub primary_role: Option<PrimaryRole>,
    pub resource_kind: Option<ResourceKind>,
    pub resource_id: Option<Uuid>,
    pub version_id: Option<Uuid>,
    pub status: Option<TelemetryEventStatus>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub installation_id: Option<Uuid>,
    pub relation: Option<TelemetryResourceRelation>,
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsMetric {
    Requests,
    ResourceUses,
    ModelCalls,
    ToolCalls,
    InputTokens,
    OutputTokens,
    TotalTokens,
    EstimatedCost,
    SuccessRate,
    ErrorRate,
    AverageDuration,
    Installations,
    FeedbackRating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsDimension {
    Time,
    Outcome,
    Resource,
    ResourceKind,
    Version,
    Member,
    Role,
    Provider,
    Model,
    Tool,
    Installation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsVisualization {
    Kpi,
    Line,
    Area,
    Bar,
    StackedBar,
    Donut,
    Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsWidgetSize {
    OneThird,
    #[default]
    Half,
    TwoThirds,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsWidget {
    /// Client-stable identifier used for ordering and editor operations. It is
    /// deliberately not executable input and is restricted to a small slug.
    pub id: String,
    pub title: String,
    pub visualization: AnalyticsVisualization,
    pub metric: AnalyticsMetric,
    pub group_by: Option<AnalyticsDimension>,
    #[serde(default)]
    pub size: AnalyticsWidgetSize,
    #[serde(default = "default_breakdown_limit")]
    pub limit: u16,
    #[serde(default)]
    pub show_legend: bool,
}

fn default_breakdown_limit() -> u16 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsViewDefinition {
    pub schema_version: u16,
    #[serde(default)]
    pub preset: AnalyticsDashboardPreset,
    #[serde(default)]
    pub density: AnalyticsDashboardDensity,
    #[serde(default)]
    pub query: AnalyticsQuery,
    pub widgets: Vec<AnalyticsWidget>,
}

impl AnalyticsViewDefinition {
    /// Validate the complete allowlisted dashboard model before persistence.
    /// There is no SQL/expression/string query escape hatch in this type.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != ANALYTICS_VIEW_SCHEMA_VERSION {
            return Err(format!(
                "schema_version must be {ANALYTICS_VIEW_SCHEMA_VERSION}"
            ));
        }
        validate_query(&self.query)?;
        if self.widgets.is_empty() || self.widgets.len() > MAX_ANALYTICS_WIDGETS {
            return Err(format!(
                "widgets must contain 1–{MAX_ANALYTICS_WIDGETS} items"
            ));
        }

        let mut ids = HashSet::with_capacity(self.widgets.len());
        for widget in &self.widgets {
            if widget.id.is_empty()
                || widget.id.len() > MAX_ANALYTICS_WIDGET_ID_LENGTH
                || !widget
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(format!(
                    "widget id must be a 1–{MAX_ANALYTICS_WIDGET_ID_LENGTH} character ASCII slug"
                ));
            }
            if !ids.insert(widget.id.as_str()) {
                return Err("widget ids must be unique".into());
            }
            validate_trimmed_text(
                "widget title",
                &widget.title,
                MAX_ANALYTICS_WIDGET_TITLE_LENGTH,
            )?;
            if widget.limit == 0 || widget.limit > MAX_ANALYTICS_BREAKDOWN_LIMIT {
                return Err(format!(
                    "widget limit must be 1–{MAX_ANALYTICS_BREAKDOWN_LIMIT}"
                ));
            }
            match widget.visualization {
                AnalyticsVisualization::Kpi if widget.group_by.is_some() => {
                    return Err("KPI widgets cannot define group_by".into());
                }
                AnalyticsVisualization::Kpi => {}
                _ if widget.group_by.is_none() => {
                    return Err("chart and table widgets require group_by".into());
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsView {
    pub id: Uuid,
    pub project_id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub visibility: AnalyticsViewVisibility,
    pub definition: AnalyticsViewDefinition,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAnalyticsViewRequest {
    pub name: String,
    pub description: Option<String>,
    pub visibility: AnalyticsViewVisibility,
    pub definition: AnalyticsViewDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateAnalyticsViewRequest {
    pub name: String,
    pub description: Option<String>,
    pub visibility: AnalyticsViewVisibility,
    pub definition: AnalyticsViewDefinition,
    /// Last revision read by the editor. A stale value returns HTTP 409.
    pub revision: u64,
}

pub fn validate_analytics_view_metadata(
    name: &str,
    description: Option<&str>,
) -> Result<(), String> {
    validate_trimmed_text("name", name, MAX_ANALYTICS_VIEW_NAME_LENGTH)?;
    if let Some(description) = description {
        if description.trim() != description {
            return Err("description cannot start or end with whitespace".into());
        }
        if description.len() > MAX_ANALYTICS_VIEW_DESCRIPTION_LENGTH {
            return Err(format!(
                "description must be at most {MAX_ANALYTICS_VIEW_DESCRIPTION_LENGTH} characters"
            ));
        }
        if description.chars().any(char::is_control) {
            return Err("description cannot contain control characters".into());
        }
    }
    Ok(())
}

fn validate_query(query: &AnalyticsQuery) -> Result<(), String> {
    match query.date_range {
        AnalyticsDateRange::Custom => {
            let (Some(from), Some(to)) = (query.from, query.to) else {
                return Err("custom date_range requires from and to".into());
            };
            if from > to {
                return Err("from must be on or before to".into());
            }
            if (to - from).num_days() > 366 {
                return Err("custom date range cannot exceed 366 days".into());
            }
        }
        _ if query.from.is_some() || query.to.is_some() => {
            return Err("from and to are only allowed for custom date_range".into());
        }
        _ => {}
    }

    for (name, value) in [
        ("provider", query.provider.as_deref()),
        ("model", query.model.as_deref()),
        ("tool_name", query.tool_name.as_deref()),
    ] {
        if let Some(value) = value {
            validate_trimmed_text(name, value, MAX_ANALYTICS_LABEL_LENGTH)?;
        }
    }
    Ok(())
}

fn validate_trimmed_text(name: &str, value: &str, max_length: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > max_length || value.trim() != value {
        return Err(format!(
            "{name} must be 1–{max_length} characters without surrounding whitespace"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{name} cannot contain control characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition() -> AnalyticsViewDefinition {
        AnalyticsViewDefinition {
            schema_version: ANALYTICS_VIEW_SCHEMA_VERSION,
            preset: AnalyticsDashboardPreset::Executive,
            density: AnalyticsDashboardDensity::Comfortable,
            query: AnalyticsQuery::default(),
            widgets: vec![AnalyticsWidget {
                id: "requests-trend".into(),
                title: "Requests over time".into(),
                visualization: AnalyticsVisualization::Area,
                metric: AnalyticsMetric::Requests,
                group_by: Some(AnalyticsDimension::Time),
                size: AnalyticsWidgetSize::Full,
                limit: 10,
                show_legend: false,
            }],
        }
    }

    #[test]
    fn definition_accepts_only_structured_allowlisted_queries() {
        let definition = definition();
        assert!(definition.validate().is_ok());
        assert_eq!(
            serde_json::to_value(&definition).unwrap()["query"]["date_range"],
            "last_30_days"
        );

        let raw = serde_json::json!({
            "schema_version": 1,
            "query": {"sql": "select * from telemetry_events"},
            "widgets": [{
                "id": "requests",
                "title": "Requests",
                "visualization": "kpi",
                "metric": "requests",
                "group_by": null
            }]
        });
        assert!(serde_json::from_value::<AnalyticsViewDefinition>(raw).is_err());
    }

    #[test]
    fn definition_rejects_invalid_layout_and_custom_range() {
        let mut value = definition();
        value.widgets[0].group_by = None;
        assert_eq!(
            value.validate().unwrap_err(),
            "chart and table widgets require group_by"
        );

        let mut value = definition();
        value.query.date_range = AnalyticsDateRange::Custom;
        assert_eq!(
            value.validate().unwrap_err(),
            "custom date_range requires from and to"
        );
    }
}
