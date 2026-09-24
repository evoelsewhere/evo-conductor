//! Jira connection — Basic Auth with an Atlassian API token, not OAuth.
//!
//! A 3-legged OAuth (Jira Cloud "Connect"/"Forge" style) flow needs a
//! registered Atlassian OAuth app (client id/secret) this deployment has
//! none of. An API token is what an admin can generate today, from their own
//! Atlassian account, with no app registration — the same mechanism Jira's
//! own docs recommend for script/integration access. Storage follows the
//! exact write-only pattern `core::email` already uses for the SMTP
//! password: the token never round-trips through an API response, and lives
//! in its own permission-restricted file outside SQL.

use std::path::PathBuf;

use anyhow::{bail, Context};
use chrono::{DateTime, Utc};
use conductor_domain::{
    resolve_project_label, resolve_task_type, JiraSettings, JiraTask, ModelCostReport,
    ProjectPrefixRule, TaskTypeRule,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;

use crate::core::state::AppState;

/// Resolved, ready-to-call Jira configuration — the DB-stored settings plus
/// the token read back from its secret file.
#[derive(Debug, Clone)]
pub struct JiraConfig {
    pub site_url: String,
    pub email: String,
    pub api_token: String,
    pub default_project_key: String,
    pub report_issue_key: String,
}

fn jira_token_path() -> PathBuf {
    let data_root = std::env::var("CONDUCTOR_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data"));
    data_root.join("secrets").join("jira_api_token")
}

async fn write_jira_token(bytes: &[u8]) -> anyhow::Result<()> {
    let path = jira_token_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("create Jira secret directory")?;
    }
    match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("Jira token path must not be a symlink");
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("inspect Jira token path"),
    }
    let temporary = path.with_extension(format!("conductor-{}.tmp", uuid::Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes)
        .await
        .context("write Jira token")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .await
            .context("protect Jira token")?;
    }
    tokio::fs::rename(&temporary, &path)
        .await
        .context("activate Jira token")?;
    Ok(())
}

async fn read_jira_token() -> anyhow::Result<Option<String>> {
    let path = jira_token_path();
    match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("Jira token path must not be a symlink");
        }
        Ok(_) => {
            let bytes = tokio::fs::read(&path).await.context("read Jira token")?;
            Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("inspect Jira token path"),
    }
}

async fn clear_jira_token() -> anyhow::Result<()> {
    match tokio::fs::remove_file(jira_token_path()).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("remove Jira token"),
    }
}

/// Applies a `JiraSettings` update: persists the write-only token to its
/// secret file (or removes it), then returns settings safe to store in SQL
/// (`api_token` stripped, `api_token_set` reflecting the outcome).
pub async fn apply_jira_settings_update(mut settings: JiraSettings) -> anyhow::Result<JiraSettings> {
    if settings.clear_api_token {
        clear_jira_token().await?;
        settings.api_token_set = false;
    } else if let Some(token) = settings.api_token.take() {
        if !token.is_empty() {
            write_jira_token(token.as_bytes()).await?;
            settings.api_token_set = true;
        }
    }
    settings.api_token = None;
    settings.clear_api_token = false;
    Ok(settings)
}

/// Resolves the DB-stored Jira settings plus the token from its secret file,
/// erroring with an actionable message if anything required is missing.
pub async fn resolve_jira_config(state: &AppState) -> anyhow::Result<JiraConfig> {
    let settings = state.db.instance().jira_settings().await?;
    if !settings.enabled {
        bail!("Jira is not enabled — configure it in Settings first");
    }
    if settings.site_url.trim().is_empty() || settings.email.trim().is_empty() {
        bail!("Jira is enabled but the site URL or account email is not set");
    }
    let api_token = read_jira_token()
        .await?
        .filter(|token| !token.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Jira is enabled but no API token is stored"))?;
    Ok(JiraConfig {
        site_url: settings.site_url.trim_end_matches('/').to_string(),
        email: settings.email,
        api_token,
        default_project_key: settings.default_project_key,
        report_issue_key: settings.report_issue_key,
    })
}

fn client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?)
}

async fn get_json<T: DeserializeOwned>(config: &JiraConfig, path: &str) -> anyhow::Result<T> {
    let response = client()?
        .get(format!("{}{path}", config.site_url))
        .basic_auth(&config.email, Some(&config.api_token))
        .header("Accept", "application/json")
        .send()
        .await
        .context("Jira request failed")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("Jira returned {status}: {}", truncate(&body, 500));
    }
    response
        .json::<T>()
        .await
        .context("Jira response was not the expected shape")
}

fn truncate(value: &str, max: usize) -> &str {
    match value.char_indices().nth(max) {
        Some((index, _)) => &value[..index],
        None => value,
    }
}

/// The authenticated Atlassian account — the cheapest possible proof the
/// site URL, email, and token are all correct together.
pub async fn test_connection(config: &JiraConfig) -> anyhow::Result<serde_json::Value> {
    get_json(config, "/rest/api/3/myself").await
}

/// Issues matching a JQL query, for the "can Conductor actually read your
/// project" verification the site-wide `myself` check alone can't prove.
pub async fn search_issues(config: &JiraConfig, jql: &str) -> anyhow::Result<serde_json::Value> {
    let query = url_encode(jql);
    get_json(
        config,
        &format!(
            "/rest/api/3/search/jql?jql={query}&maxResults=25&fields=summary,status,issuetype"
        ),
    )
    .await
}

async fn post_json(
    config: &JiraConfig,
    path: &str,
    body: &serde_json::Value,
) -> anyhow::Result<()> {
    let response = client()?
        .post(format!("{}{path}", config.site_url))
        .basic_auth(&config.email, Some(&config.api_token))
        .header("Accept", "application/json")
        .json(body)
        .send()
        .await
        .context("Jira request failed")?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        bail!("Jira returned {status}: {}", truncate(&text, 500));
    }
    Ok(())
}

/// Posts a usage-report comment to the configured issue, built directly from
/// Conductor's own in-process cost report — never round-tripping through its
/// own HTTP API to build the numbers it's about to post about.
pub async fn post_comment(
    config: &JiraConfig,
    issue_key: &str,
    adf_body: serde_json::Value,
) -> anyhow::Result<()> {
    if issue_key.trim().is_empty() {
        bail!("no report_issue_key is configured — set which issue reports post to");
    }
    post_json(
        config,
        &format!("/rest/api/3/issue/{issue_key}/comment"),
        &json!({ "body": adf_body }),
    )
    .await
}

/// Renders a `ModelCostReport` as an Atlassian Document Format comment body —
/// a heading plus a preformatted text block. A `codeBlock` node, not an ADF
/// table: far less markup to get right, and a monospace breakdown reads
/// perfectly well for a periodic automated comment.
pub fn render_report_adf(
    report: &ModelCostReport,
    project_name: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> serde_json::Value {
    let mut lines = vec![format!(
        "{project_name} usage report — {} to {}",
        from.format("%Y-%m-%d"),
        to.format("%Y-%m-%d")
    )];
    let usd = |micros: u64| format!("${:.2}", micros as f64 / 1_000_000.0);
    lines.push(String::new());
    lines.push(format!(
        "Total: {} calls, {} tokens, {}",
        report.totals.calls,
        report.totals.total_tokens,
        usd(report.totals.total_cost_usd_micros)
    ));
    if report.totals.unpriced_calls > 0 {
        lines.push(format!(
            "({} of those calls have no Conductor price yet)",
            report.totals.unpriced_calls
        ));
    }
    if report.rows.is_empty() {
        lines.push(String::new());
        lines.push("No model calls in this period.".to_string());
    } else {
        lines.push(String::new());
        lines.push("By model:".to_string());
        for row in &report.rows {
            lines.push(format!(
                "  {}:{} — {} calls, {} tokens, {}",
                row.provider,
                row.model,
                row.calls,
                row.total_tokens,
                usd(row.total_cost_usd_micros)
            ));
        }
    }
    let text = lines.join("\n");
    json!({
        "version": 1,
        "type": "doc",
        "content": [
            {
                "type": "codeBlock",
                "content": [{"type": "text", "text": text}]
            }
        ]
    })
}

/// The full T4.3 pipeline: build the period's cost report in-process, render
/// it, and post it as a comment. Returns the period-start key (`YYYY-MM-DD`)
/// on success, for T4.4's idempotency guard to record.
pub async fn post_usage_report(
    state: &AppState,
    project_id: uuid::Uuid,
    project_name: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> anyhow::Result<String> {
    let config = resolve_jira_config(state).await?;
    let report = crate::core::model_cost_report::build(
        &state.db,
        &state.model_rates,
        project_id,
        from,
        to,
        &conductor_storage::repos::CostReportFilters::default(),
    )
    .await
    .context("building the usage report failed")?;
    let adf = render_report_adf(&report, project_name, from, to);
    post_comment(&config, &config.report_issue_key.clone(), adf).await?;
    Ok(from.format("%Y-%m-%d").to_string())
}

#[derive(Debug, Deserialize)]
struct JiraSearchPage {
    issues: Vec<JiraIssue>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "isLast", default)]
    is_last: bool,
}

#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraIssueFields,
}

#[derive(Debug, Deserialize)]
struct JiraIssueFields {
    summary: String,
    #[serde(default)]
    issuetype: JiraNamedRef,
    assignee: Option<JiraAssignee>,
    parent: Option<JiraParent>,
    #[serde(default)]
    status: JiraNamedRef,
    updated: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct JiraNamedRef {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct JiraAssignee {
    #[serde(rename = "accountId")]
    account_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    /// Absent when the API token's account can't see it — Atlassian hides
    /// this behind the site's user-privacy setting for some viewers, not
    /// only unlicensed ones. Matching falls back accordingly, never errors.
    #[serde(rename = "emailAddress")]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JiraParent {
    key: String,
}

/// Jira's own date format (`2024-01-01T10:00:00.000+0000`) isn't RFC 3339 —
/// the offset has no colon. Falls back to `None` rather than failing the
/// whole sync over one field a report never depends on.
fn parse_jira_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f%z")
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Pages through every issue in `project_key` via `/rest/api/3/search/jql`,
/// applying `task_type_rules`/`project_prefix_rules` to resolve each task's
/// display type and sub-project label. Ordered by `updated DESC` so a sync
/// interrupted partway through still lands the most recently active tasks
/// first.
pub async fn list_project_tasks(
    config: &JiraConfig,
    project_key: &str,
    task_type_rules: &[TaskTypeRule],
    project_prefix_rules: &[ProjectPrefixRule],
) -> anyhow::Result<Vec<JiraTask>> {
    let jql = url_encode(&format!("project = {project_key} ORDER BY updated DESC"));
    let fields = "summary,issuetype,assignee,parent,status,updated";
    let mut tasks = Vec::new();
    let mut page_token: Option<String> = None;
    let synced_at = Utc::now();

    loop {
        let mut path = format!(
            "/rest/api/3/search/jql?jql={jql}&maxResults=100&fields={fields}"
        );
        if let Some(token) = &page_token {
            path.push_str(&format!("&nextPageToken={}", url_encode(token)));
        }
        let page: JiraSearchPage = get_json(config, &path).await?;
        let is_last = page.is_last || page.issues.is_empty();
        let next_page_token = page.next_page_token.clone();
        for issue in page.issues {
            let resolved_type =
                resolve_task_type(task_type_rules, &issue.fields.summary, &issue.fields.issuetype.name)
                    .to_string();
            let resolved_project =
                resolve_project_label(project_prefix_rules, &issue.fields.summary)
                    .map(str::to_string);
            tasks.push(JiraTask {
                issue_key: issue.key,
                title: issue.fields.summary,
                issue_type: issue.fields.issuetype.name,
                resolved_type,
                resolved_project,
                assignee_display_name: issue.fields.assignee.as_ref().and_then(|a| a.display_name.clone()),
                assignee_email: issue.fields.assignee.as_ref().and_then(|a| a.email.clone()),
                assignee_account_id: issue.fields.assignee.and_then(|a| a.account_id),
                parent_key: issue.fields.parent.map(|p| p.key),
                status: issue.fields.status.name,
                jira_updated_at: issue.fields.updated.as_deref().and_then(parse_jira_datetime),
                synced_at,
            });
        }
        if is_last || next_page_token.is_none() {
            break;
        }
        page_token = next_page_token;
    }
    Ok(tasks)
}

#[derive(Debug, Deserialize)]
struct JiraChangelogPage {
    values: Vec<JiraChangelogEntry>,
    #[serde(rename = "isLast", default)]
    is_last: bool,
}

#[derive(Debug, Deserialize)]
struct JiraChangelogEntry {
    created: String,
    items: Vec<JiraChangelogItem>,
}

#[derive(Debug, Deserialize)]
struct JiraChangelogItem {
    field: String,
    #[serde(rename = "fromString")]
    from_string: Option<String>,
    #[serde(rename = "toString")]
    to_string: Option<String>,
}

/// Every status transition Jira has ever recorded for one issue, oldest
/// first, plus a synthetic entry for "whatever it was before the first
/// recorded transition" pinned to the Unix epoch — so a report event from
/// before Conductor ever saw this issue's changelog still resolves to a
/// real status rather than `None`. Only called for issues that have a
/// `task_activations` row (see `TaskActivationRepo::distinct_issue_keys`):
/// most issues in a project are never opened through the plugin, and paging
/// a changelog nobody needs would just be wasted Jira API budget.
pub async fn fetch_status_history(
    config: &JiraConfig,
    issue_key: &str,
) -> anyhow::Result<Vec<conductor_domain::JiraStatusChange>> {
    let mut changes = Vec::new();
    let mut initial_status: Option<String> = None;
    let mut start_at = 0i64;
    loop {
        let page: JiraChangelogPage = get_json(
            config,
            &format!("/rest/api/3/issue/{issue_key}/changelog?startAt={start_at}&maxResults=100"),
        )
        .await?;
        let fetched = page.values.len();
        for entry in &page.values {
            let Some(changed_at) = parse_jira_datetime(&entry.created) else {
                continue;
            };
            for item in &entry.items {
                if item.field != "status" {
                    continue;
                }
                if initial_status.is_none() {
                    initial_status = item.from_string.clone();
                }
                if let Some(status) = &item.to_string {
                    changes.push(conductor_domain::JiraStatusChange {
                        issue_key: issue_key.to_string(),
                        status: status.clone(),
                        changed_at,
                    });
                }
            }
        }
        if page.is_last || fetched == 0 {
            break;
        }
        start_at += fetched as i64;
    }
    changes.sort_by_key(|change| change.changed_at);
    if let Some(status) = initial_status {
        changes.insert(
            0,
            conductor_domain::JiraStatusChange {
                issue_key: issue_key.to_string(),
                status,
                changed_at: DateTime::<Utc>::UNIX_EPOCH,
            },
        );
    }
    Ok(changes)
}

fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
