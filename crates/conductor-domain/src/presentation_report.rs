//! A single composed report for presentation (Excel/PowerPoint export): the
//! same figures every other analytics view already shows, gathered into one
//! structure with a prior-period comparison and the call-outs a
//! presentation needs -- built once here so xlsx and pptx rendering don't
//! each re-derive "top 5" or "% change" themselves, and so the two file
//! formats can never disagree about what the numbers are.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    MemberCostReportRow, ModelCostReportRow, ModelCostReportTotals, ResourceUsageDay,
    ResourceUsageTotals, TaskCostRow,
};

/// The requested range, plus the immediately-preceding range of the same
/// length -- used for every "vs. last period" call-out. Not a rolling
/// average or a calendar-aligned period: just "the same number of days,
/// right before `from`".
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReportWindow {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub previous_from: DateTime<Utc>,
    pub previous_to: DateTime<Utc>,
}

impl ReportWindow {
    pub fn new(from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        let span = to - from;
        Self {
            from,
            to,
            previous_from: from - span,
            previous_to: from,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Member,
    Model,
    Jira,
}

impl ReportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Model => "model",
            Self::Jira => "jira",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "member" => Some(Self::Member),
            "model" => Some(Self::Model),
            "jira" => Some(Self::Jira),
            _ => None,
        }
    }
}

/// One row of a "top N by spend" call-out -- the same shape for members,
/// models, or Jira tasks, so a renderer draws one bar-chart function instead
/// of three near-identical ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopEntry {
    pub label: String,
    pub total_cost_usd_micros: u64,
    pub total_tokens: u64,
}

/// A Jira task whose cost is well above the mean of other tasks sharing its
/// `resolved_type` -- a plain-language "this one might be worth a look"
/// signal (repeated rework, scope creep), not a statistical claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierTask {
    pub issue_key: String,
    pub title: String,
    pub resolved_type: String,
    pub total_cost_usd_micros: u64,
    /// How many times the mean cost of same-`resolved_type` siblings this
    /// task's own cost is -- 3.0 means "3x the typical task of this type".
    pub times_the_type_average: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportCallouts {
    pub top_members: Vec<TopEntry>,
    pub top_models: Vec<TopEntry>,
    pub top_tasks: Vec<TopEntry>,
    /// Percentage change in total cost vs. the previous period; positive
    /// means spend went up. `None` when the previous period had zero spend
    /// -- a ratio against zero has no honest percentage to report.
    pub cost_change_pct: Option<f64>,
    pub cache_savings_usd_micros: u64,
    pub cache_savings_change_pct: Option<f64>,
    /// Basis points (1/100 of a percent) of model calls that were unpriced --
    /// integer so the figure round-trips exactly rather than drifting
    /// through floating point.
    pub unpriced_ratio_bps: u32,
    pub cost_outlier_tasks: Vec<OutlierTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresentationReport {
    pub project_name: String,
    pub window: ReportWindow,
    pub kinds: Vec<ReportKind>,
    pub totals: ResourceUsageTotals,
    pub daily: Vec<ResourceUsageDay>,
    pub members: Vec<MemberCostReportRow>,
    pub member_totals: ModelCostReportTotals,
    pub models: Vec<ModelCostReportRow>,
    pub model_totals: ModelCostReportTotals,
    pub jira_tasks: Vec<TaskCostRow>,
    pub callouts: ReportCallouts,
}

/// A generated report file, ready to stream back or attach elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    Xlsx,
    Pptx,
}

impl ReportFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "xlsx" => Some(Self::Xlsx),
            "pptx" => Some(Self::Pptx),
            _ => None,
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Xlsx => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            }
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
        }
    }
}

/// Where a generated report should be delivered, instead of (or in addition
/// to) being downloaded directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReportDestination {
    Email { address: String },
    Jira { issue_key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDeliverRequest {
    pub format: ReportFormat,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub kinds: Vec<ReportKind>,
    pub destination: ReportDestination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDeliverResponse {
    pub delivered: bool,
    pub destination: ReportDestination,
}
