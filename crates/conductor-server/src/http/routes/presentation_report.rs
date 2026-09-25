//! Self-service report export: download an xlsx/pptx directly, or have it
//! delivered to an email address or Jira issue instead. See
//! `core::presentation_report` for how the data is composed and
//! `core::report_export` for how it's rendered.

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderValue};
use axum::response::Response;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use conductor_domain::{
    ConductorError, ReportDeliverRequest, ReportDeliverResponse, ReportDestination, ReportFormat,
    ReportKind,
};
use serde::Deserialize;

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::core::{email, jira, presentation_report, report_export};

/// Same default window every other analytics route uses when `from`/`to`
/// are omitted, so "export the report" without picking a range matches
/// whatever the page the user was just looking at already showed.
const DEFAULT_RANGE_DAYS: i64 = 30;

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub format: String,
    pub from: Option<String>,
    pub to: Option<String>,
    /// Comma-separated `member,model,jira`; every kind when omitted.
    pub kinds: Option<String>,
}

fn resolve_range(from: Option<&str>, to: Option<&str>) -> ApiResult<(DateTime<Utc>, DateTime<Utc>)> {
    let to = match to {
        Some(value) => parse_timestamp(value, "to")?,
        None => Utc::now(),
    };
    let from = match from {
        Some(value) => parse_timestamp(value, "from")?,
        None => to - Duration::days(DEFAULT_RANGE_DAYS),
    };
    if from > to {
        return Err(ConductorError::msg("from must be before to").into());
    }
    Ok((from, to))
}

fn parse_timestamp(value: &str, field: &str) -> ApiResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| ConductorError::msg(format!("{field} must be an RFC 3339 timestamp")).into())
}

fn resolve_kinds(raw: Option<&str>) -> Vec<ReportKind> {
    let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
        return vec![ReportKind::Member, ReportKind::Model, ReportKind::Jira];
    };
    let kinds: Vec<ReportKind> = raw.split(',').filter_map(|part| ReportKind::parse(part.trim())).collect();
    if kinds.is_empty() {
        vec![ReportKind::Member, ReportKind::Model, ReportKind::Jira]
    } else {
        kinds
    }
}

async fn project_id(state: &AppState) -> ApiResult<uuid::Uuid> {
    Ok(state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?
        .id)
}

fn render(format: ReportFormat, report: &conductor_domain::PresentationReport) -> ApiResult<Vec<u8>> {
    match format {
        ReportFormat::Xlsx => report_export::render_xlsx(report)
            .map_err(|error| ConductorError::msg(format!("failed to render xlsx: {error}")).into()),
        ReportFormat::Pptx => report_export::render_pptx(report)
            .map_err(|error| ConductorError::msg(format!("failed to render pptx: {error}")).into()),
    }
}

fn report_filename(project_name: &str, format: ReportFormat) -> String {
    let slug: String = project_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    format!("{slug}-usage-report.{}", format.as_str())
}

pub async fn export(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> ApiResult<Response> {
    let format = ReportFormat::parse(&query.format)
        .ok_or_else(|| ConductorError::msg("format must be xlsx or pptx"))?;
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    let kinds = resolve_kinds(query.kinds.as_deref());
    let project_id = project_id(&state).await?;

    let report =
        presentation_report::build(&state.db, &state.model_rates, project_id, from, to, &kinds)
            .await?;
    let bytes = render(format, &report)?;
    let filename = report_filename(&report.project_name, format);

    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(format.content_type()),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    Ok(response)
}

pub async fn deliver(
    State(state): State<AppState>,
    Json(request): Json<ReportDeliverRequest>,
) -> ApiResult<Json<ReportDeliverResponse>> {
    let project_id = project_id(&state).await?;
    let report = presentation_report::build(
        &state.db,
        &state.model_rates,
        project_id,
        request.from,
        request.to,
        &request.kinds,
    )
    .await?;
    let bytes = render(request.format, &report)?;
    let filename = report_filename(&report.project_name, request.format);

    match &request.destination {
        ReportDestination::Email { address } => {
            let email_config = email::resolve_email_config(&state)
                .await
                .map_err(|error| ConductorError::msg(error.to_string()))?;
            let subject = format!("{} — usage report", report.project_name);
            let body = format!(
                "<p>Attached: the {} usage report for {} to {}.</p>",
                report.project_name,
                report.window.from.format("%Y-%m-%d"),
                report.window.to.format("%Y-%m-%d"),
            );
            email::send_email_with_attachment(
                &email_config,
                address,
                &subject,
                &body,
                &filename,
                request.format.content_type(),
                bytes,
            )
            .await
            .map_err(|error| ConductorError::msg(error.to_string()))?;
        }
        ReportDestination::Jira { issue_key } => {
            let jira_config = jira::resolve_jira_config(&state)
                .await
                .map_err(|error| ConductorError::msg(error.to_string()))?;
            jira::upload_attachment(
                &jira_config,
                issue_key,
                &filename,
                request.format.content_type(),
                bytes,
            )
            .await
            .map_err(|error| ConductorError::msg(error.to_string()))?;
        }
    }

    Ok(Json(ReportDeliverResponse {
        delivered: true,
        destination: request.destination,
    }))
}
