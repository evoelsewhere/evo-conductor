//! Ad hoc: prove Conductor can actually read Jira with whatever is stored in
//! `data/conductor.db` / `data/secrets/jira_api_token`, without going through
//! browser auth. Prints the real HTTP error instead of a generic 500.
//!
//! Usage: cargo run -p conductor-server --example test_jira_connection

use conductor_domain::JiraSettings;
use conductor_server::core::jira::{search_issues, test_connection, JiraConfig};
use conductor_storage::repos::InstanceRepo;
use sqlx::any::AnyPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite:data/conductor.db")
        .await?;
    let instance = InstanceRepo::new(pool);
    let settings: JiraSettings = instance.jira_settings().await?;
    println!(
        "DB jira settings: enabled={} site_url={} email={} default_project_key={} report_issue_key={} token_set={}",
        settings.enabled,
        settings.site_url,
        settings.email,
        settings.default_project_key,
        settings.report_issue_key,
        settings.api_token_set
    );

    let token_path = std::path::Path::new("data/secrets/jira_api_token");
    let token = std::fs::read_to_string(token_path)
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    println!("Secret file {}: {} bytes", token_path.display(), token.len());

    let config = JiraConfig {
        site_url: settings.site_url.trim_end_matches('/').to_string(),
        email: settings.email,
        api_token: token,
        default_project_key: settings.default_project_key.clone(),
        report_issue_key: settings.report_issue_key.clone(),
    };

    match test_connection(&config).await {
        Ok(value) => println!(
            "myself OK: accountId={} emailAddress={} displayName={}",
            value["accountId"], value["emailAddress"], value["displayName"]
        ),
        Err(error) => {
            println!("myself FAILED: {error:#}");
            return Ok(());
        }
    }

    let jql = format!("project = {}", config.default_project_key);
    match search_issues(&config, &jql).await {
        Ok(value) => {
            let issues = value["issues"].as_array().map(|v| v.len()).unwrap_or(0);
            println!("search OK: {issues} issue(s) returned for JQL `{jql}`");
            if let Some(list) = value["issues"].as_array() {
                for issue in list.iter().take(5) {
                    println!(
                        "  {} — {}",
                        issue["key"], issue["fields"]["summary"]
                    );
                }
            }
        }
        Err(error) => println!("search FAILED: {error:#}"),
    }

    Ok(())
}
