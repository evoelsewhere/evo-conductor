use conductor_domain::{TelemetryEventStatus, TelemetryEventType};
use sqlx::{Any, Pool, Row};
use uuid::Uuid;

use crate::core::dialect::DatabaseKind;

/// Money columns, in micro-USD.
///
/// These must be `BIGINT`, not `INTEGER`: Postgres and MySQL read `INTEGER` as
/// 32 bits, so anything above about $2,147 overflows on insert — an ordinary
/// monthly budget. SQLite widens `INTEGER` on its own, which is why the fault
/// was invisible there. Token and duration columns stay `INTEGER` because
/// ingest caps them well inside 32 bits.
const MONEY_COLUMNS: &[(&str, &str)] = &[
    ("spend_limits", "limit_usd_micros"),
    ("telemetry_events", "estimated_cost_usd_micros"),
    ("telemetry_events", "server_cost_usd_micros"),
    ("model_prices", "input_micro_per_million"),
    ("model_prices", "output_micro_per_million"),
    ("model_prices", "cache_read_micro_per_million"),
    ("model_prices", "cache_write_micro_per_million"),
    ("model_prices", "reasoning_micro_per_million"),
];

/// Widen money columns on a database created while they were `INTEGER`.
///
/// Checked rather than best-effort: a silent failure here leaves a Postgres
/// deployment that rejects any allowance over about $2,147, which is the very
/// fault this repairs. SQLite needs nothing — its `INTEGER` already holds 64
/// bits — and re-running the widening on a column that is already `BIGINT` is
/// a no-op on both other engines.
async fn widen_money_columns(pool: &Pool<Any>, kind: DatabaseKind) -> Result<(), sqlx::Error> {
    // The schema function differs, and both engines spell the widened type
    // "bigint" in `information_schema`.
    let current_schema = match kind {
        DatabaseKind::Sqlite => return Ok(()),
        DatabaseKind::Postgres => "current_schema()",
        DatabaseKind::Mysql => "DATABASE()",
    };
    let probe = format!(
        "SELECT data_type FROM information_schema.columns          WHERE table_schema = {current_schema} AND table_name = ? AND column_name = ?"
    );

    for (table, column) in MONEY_COLUMNS {
        let observed: Option<String> = sqlx::query_scalar(&probe)
            .bind(table)
            .bind(column)
            .fetch_optional(pool)
            .await?;
        // Absent means the table predates this column; the ALTER above adds it
        // already widened. Already "bigint" means there is nothing to do — and
        // skipping it matters, because on Postgres the statement takes an
        // exclusive lock on a table this size at every startup.
        let Some(observed) = observed else { continue };
        if observed.eq_ignore_ascii_case("bigint") {
            continue;
        }

        let statement = match kind {
            DatabaseKind::Sqlite => unreachable!("returned above"),
            DatabaseKind::Postgres => {
                format!("ALTER TABLE {table} ALTER COLUMN {column} TYPE BIGINT")
            }
            // MySQL restates the definition, and `MODIFY` keeps the existing
            // nullability only if it is repeated.
            DatabaseKind::Mysql => match *column {
                "limit_usd_micros" => {
                    format!("ALTER TABLE {table} MODIFY {column} BIGINT NOT NULL")
                }
                _ => format!("ALTER TABLE {table} MODIFY {column} BIGINT"),
            },
        };
        tracing::info!(table, column, from = %observed, "widening money column to BIGINT");
        sqlx::query(&statement).execute(pool).await?;
    }
    Ok(())
}

/// Portable schema (TEXT ids, INTEGER flags) — works on SQLite, Postgres, MySQL.
pub async fn run(pool: &Pool<Any>, kind: DatabaseKind) -> Result<(), sqlx::Error> {
    let telemetry_table = format!(
        r#"
        CREATE TABLE IF NOT EXISTS telemetry_events (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT,
            user_id TEXT NOT NULL,
            installation_id TEXT,
            request_id TEXT,
            session_id TEXT,
            event_type TEXT NOT NULL DEFAULT '{}',
            sequence INTEGER NOT NULL DEFAULT 0,
            agent_name TEXT,
            provider TEXT,
            model TEXT,
            response_model TEXT,
            tokens_in INTEGER NOT NULL DEFAULT 0,
            tokens_out INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens INTEGER NOT NULL DEFAULT 0,
            reasoning_tokens INTEGER NOT NULL DEFAULT 0,
            tool_use_tokens INTEGER NOT NULL DEFAULT 0,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            tool_name TEXT,
            tool_category TEXT,
            status TEXT NOT NULL DEFAULT '{}',
            error_category TEXT,
            estimated_cost_usd_micros BIGINT,
            cost_source TEXT,
            service_tier TEXT,
            server_cost_usd_micros BIGINT,
            priced_catalog_version TEXT,
            pricing_basis TEXT,
            unpriced_reason TEXT,
            evoflux_version TEXT,
            primary_role_snapshot TEXT,
            sub_role_ids_snapshot TEXT,
            tag_ids_snapshot TEXT,
            tool_calls INTEGER NOT NULL DEFAULT 0,
            active_agents INTEGER NOT NULL DEFAULT 0,
            reported_at TEXT NOT NULL,
            received_at TEXT
        )
        "#,
        TelemetryEventType::ModelCall.as_str(),
        TelemetryEventStatus::Success.as_str(),
    );
    let statements = [
        r#"
        CREATE TABLE IF NOT EXISTS instance (
            id TEXT PRIMARY KEY NOT NULL,
            project_name TEXT NOT NULL,
            display_name TEXT,
            description TEXT,
            bind_host TEXT NOT NULL,
            bind_port INTEGER NOT NULL,
            public_url TEXT,
            logo_url TEXT,
            logo_artifact_key TEXT,
            logo_content_sha256 TEXT,
            logo_content_size INTEGER NOT NULL DEFAULT 0,
            logo_media_type TEXT,
            collection_level TEXT NOT NULL DEFAULT 'L1',
            storage_backend TEXT NOT NULL DEFAULT 'local',
            storage_config TEXT NOT NULL DEFAULT '{}',
            email_config TEXT NOT NULL DEFAULT '{}',
            jira_config TEXT NOT NULL DEFAULT '{}',
            setup_completed INTEGER NOT NULL DEFAULT 0,
            jwt_secret TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS sso_config (
            id INTEGER PRIMARY KEY NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 0,
            provider TEXT NOT NULL DEFAULT 'oidc',
            issuer_url TEXT,
            client_id TEXT,
            client_secret_enc TEXT,
            redirect_uri TEXT,
            scopes TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY NOT NULL,
            email TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            password_hash TEXT,
            primary_role TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            must_change_password INTEGER NOT NULL DEFAULT 0,
            session_version INTEGER NOT NULL DEFAULT 0,
            sso_issuer TEXT,
            sso_subject TEXT,
            invited_by TEXT,
            approved_at TEXT,
            approved_by TEXT,
            last_seen_at TEXT,
            created_at TEXT NOT NULL,
            jira_account_email TEXT,
            UNIQUE (sso_issuer, sso_subject)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS sub_roles (
            id TEXT PRIMARY KEY NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            description TEXT,
            color TEXT,
            created_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS user_sub_roles (
            user_id TEXT NOT NULL,
            sub_role_id TEXT NOT NULL,
            PRIMARY KEY (user_id, sub_role_id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            description TEXT,
            color TEXT,
            created_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS user_tags (
            user_id TEXT NOT NULL,
            tag_id TEXT NOT NULL,
            PRIMARY KEY (user_id, tag_id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS tag_assignments (
            tag_id TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (tag_id, entity_type, entity_id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS connection_secrets (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            prefix TEXT NOT NULL,
            token_hash TEXT NOT NULL,
            owner_user_id TEXT NOT NULL,
            scopes TEXT NOT NULL,
            last_used_at TEXT,
            expires_at TEXT,
            revoked_at TEXT,
            created_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS client_installations (
            id TEXT PRIMARY KEY NOT NULL,
            instance_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            installation_key TEXT NOT NULL,
            display_name TEXT NOT NULL,
            platform TEXT NOT NULL,
            evoflux_version TEXT NOT NULL,
            workspace_association TEXT,
            connected_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(instance_id, installation_key),
            FOREIGN KEY(instance_id) REFERENCES instance(id),
            FOREIGN KEY(user_id) REFERENCES users(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS client_registration_idempotency (
            instance_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            request_hash TEXT NOT NULL,
            installation_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY(instance_id, user_id, idempotency_key),
            FOREIGN KEY(instance_id) REFERENCES instance(id),
            FOREIGN KEY(user_id) REFERENCES users(id),
            FOREIGN KEY(installation_id) REFERENCES client_installations(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resources (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            version TEXT NOT NULL DEFAULT '0.1.0',
            owner_user_id TEXT,
            visibility TEXT NOT NULL DEFAULT 'shared',
            status TEXT NOT NULL DEFAULT 'draft',
            payload TEXT NOT NULL,
            draft_revision INTEGER NOT NULL DEFAULT 0,
            draft_artifact_key TEXT,
            draft_content_sha256 TEXT NOT NULL DEFAULT '',
            draft_content_size INTEGER NOT NULL DEFAULT 0,
            highest_semver TEXT,
            release_channel TEXT,
            published_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(project_id, kind, slug),
            FOREIGN KEY(project_id) REFERENCES instance(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resource_versions (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            version TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'draft',
            payload TEXT NOT NULL,
            changelog TEXT,
            release_channel TEXT,
            content_sha256 TEXT NOT NULL,
            content_size INTEGER NOT NULL DEFAULT 0,
            artifact_key TEXT,
            artifact_schema_version TEXT,
            minimum_evoflux_version TEXT,
            created_by TEXT NOT NULL,
            created_at TEXT NOT NULL,
            published_at TEXT,
            deprecated_at TEXT,
            deprecated_by TEXT,
            deprecation_reason TEXT,
            UNIQUE(project_id, resource_id, version),
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(resource_id) REFERENCES resources(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resource_access_rules (
            project_id TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            subject_type TEXT NOT NULL,
            subject_id TEXT NOT NULL,
            effect TEXT NOT NULL DEFAULT 'allow',
            created_at TEXT NOT NULL,
            PRIMARY KEY(project_id, resource_id, subject_type, subject_id, effect),
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(resource_id) REFERENCES resources(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resource_release_channels (
            project_id TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            version_id TEXT NOT NULL,
            updated_by TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(project_id, resource_id, channel),
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(resource_id) REFERENCES resources(id),
            FOREIGN KEY(version_id) REFERENCES resource_versions(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resource_beta_members (
            project_id TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            assigned_by TEXT NOT NULL,
            assigned_at TEXT NOT NULL,
            PRIMARY KEY(project_id, resource_id, user_id),
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(resource_id) REFERENCES resources(id),
            FOREIGN KEY(user_id) REFERENCES users(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resource_changes (
            sequence INTEGER PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            effective_user_id TEXT,
            change_kind TEXT NOT NULL,
            version_id TEXT,
            channel TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(resource_id) REFERENCES resources(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resource_version_events (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            version_id TEXT NOT NULL,
            action TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            reason TEXT,
            confirmed_deprecated INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(resource_id) REFERENCES resources(id),
            FOREIGN KEY(version_id) REFERENCES resource_versions(id),
            FOREIGN KEY(actor_id) REFERENCES users(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS installation_resource_inventory (
            project_id TEXT NOT NULL,
            installation_id TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            desired_version_id TEXT,
            applied_version_id TEXT,
            release_channel TEXT,
            content_sha256 TEXT,
            plugin_installation_id TEXT,
            observed_state TEXT NOT NULL,
            error_category TEXT,
            observed_at TEXT NOT NULL,
            PRIMARY KEY(project_id, installation_id, resource_id),
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(installation_id) REFERENCES client_installations(id),
            FOREIGN KEY(resource_id) REFERENCES resources(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resource_usage_events (
            event_id TEXT PRIMARY KEY NOT NULL,
            resource_id TEXT NOT NULL,
            resource_version TEXT NOT NULL,
            user_id TEXT NOT NULL,
            session_id TEXT,
            outcome TEXT NOT NULL,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            tokens_in INTEGER NOT NULL DEFAULT 0,
            tokens_out INTEGER NOT NULL DEFAULT 0,
            occurred_at TEXT NOT NULL,
            received_at TEXT NOT NULL
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS resource_feedback (
            id TEXT PRIMARY KEY NOT NULL,
            resource_id TEXT NOT NULL,
            resource_version TEXT NOT NULL,
            user_id TEXT NOT NULL,
            rating INTEGER NOT NULL,
            comment TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(resource_id, user_id)
        )
        "#,
        // `subject_id` is the member id for a member limit and the primary
        // role for a role limit; project limits leave it as the empty string
        // so the uniqueness constraint stays usable (NULL would not dedupe).
        r#"
        CREATE TABLE IF NOT EXISTS spend_limits (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            scope TEXT NOT NULL,
            subject_id TEXT NOT NULL DEFAULT '',
            period TEXT NOT NULL,
            limit_usd_micros BIGINT NOT NULL,
            warn_percent INTEGER NOT NULL DEFAULT 80,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(project_id, scope, subject_id, period),
            FOREIGN KEY(project_id) REFERENCES instance(id)
        )
        "#,
        // `subject_id` is the primary role for a role policy; project
        // policies leave it as the empty string, same convention as
        // `spend_limits` above.
        r#"
        CREATE TABLE IF NOT EXISTS project_ai_policies (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            scope TEXT NOT NULL,
            subject_id TEXT NOT NULL DEFAULT '',
            default_provider TEXT,
            default_model TEXT,
            allowed_providers TEXT NOT NULL DEFAULT '[]',
            allowed_tools TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(project_id, scope, subject_id),
            FOREIGN KEY(project_id) REFERENCES instance(id)
        )
        "#,
        // A local mirror of one Jira project's issues (Phase 4 follow-on:
        // task-level usage reporting). `project_id` scopes it to Conductor's
        // one project, same convention as `spend_limits`/`project_ai_policies`.
        // `resolved_type` is recomputed from `task_type_rules` every sync, so
        // it is derived data, not a second source of truth.
        r#"
        CREATE TABLE IF NOT EXISTS jira_tasks (
            issue_key TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            title TEXT NOT NULL,
            issue_type TEXT NOT NULL DEFAULT '',
            resolved_type TEXT NOT NULL DEFAULT '',
            resolved_project TEXT,
            assignee_display_name TEXT,
            assignee_email TEXT,
            assignee_account_id TEXT,
            parent_key TEXT,
            status TEXT NOT NULL DEFAULT '',
            jira_updated_at TEXT,
            synced_at TEXT NOT NULL,
            FOREIGN KEY(project_id) REFERENCES instance(id)
        )
        "#,
        // Recorded by the jira-task-assistant plugin's own tool call (via a
        // connection token, no evoflux core involvement) whenever the agent
        // starts work on a task -- "user X began task Y at T". The task
        // report turns a sequence of these per user into time windows and
        // sums that user's telemetry inside each window, for a real
        // per-task figure instead of always falling back to their whole
        // period total.
        r#"
        CREATE TABLE IF NOT EXISTS task_activations (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            issue_key TEXT NOT NULL,
            started_at TEXT NOT NULL,
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(user_id) REFERENCES users(id)
        )
        "#,
        // Jira's own changelog for an issue, synced only for issues that
        // have at least one `task_activations` row -- most issues in a
        // project are never opened through the plugin, so there's nothing
        // to split by status for them and no reason to page their history.
        // Replaced wholesale per issue on each sync tick rather than
        // diffed, since a changelog page is cheap and always authoritative.
        r#"
        CREATE TABLE IF NOT EXISTS jira_status_history (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            issue_key TEXT NOT NULL,
            status TEXT NOT NULL,
            changed_at TEXT NOT NULL,
            FOREIGN KEY(project_id) REFERENCES instance(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS model_price_catalogs (
            version TEXT PRIMARY KEY NOT NULL,
            source TEXT NOT NULL,
            source_url TEXT,
            fetched_at TEXT NOT NULL,
            model_count INTEGER NOT NULL DEFAULT 0,
            priced_model_count INTEGER NOT NULL DEFAULT 0,
            changed_model_count INTEGER NOT NULL DEFAULT 0
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS model_prices (
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            effective_from TEXT NOT NULL,
            catalog_version TEXT NOT NULL,
            input_micro_per_million BIGINT,
            output_micro_per_million BIGINT,
            cache_read_micro_per_million BIGINT,
            cache_write_micro_per_million BIGINT,
            reasoning_micro_per_million BIGINT,
            -- Long-context bands and alternate service tiers, as JSON. Both
            -- are variable-length lists rather than a fixed rate set, and
            -- both are only ever read as a whole for one model, so a column
            -- each beats a side table that every price lookup would join.
            tiers_json TEXT,
            service_tiers_json TEXT,
            PRIMARY KEY (provider, model, effective_from),
            FOREIGN KEY(catalog_version) REFERENCES model_price_catalogs(version)
        )
        "#,
        // One row per installation per UTC day it was in contact, so a day
        // with no usage is distinguishable from a day with no client.
        //
        // Without this, silence reads as zero: an installation that stopped
        // syncing makes project spend look lower rather than incomplete, and
        // nothing in the data says which it was. `last_seen_at` alone cannot
        // answer it — a heartbeat overwrites it, so history is lost.
        //
        // `contact_day` comes from the server clock, like every other
        // accounting column: a client with a wrong date must not be able to
        // move which day it appears to have been alive on.
        r#"
        CREATE TABLE IF NOT EXISTS installation_contact_days (
            installation_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            contact_day TEXT NOT NULL,
            first_contact_at TEXT NOT NULL,
            last_contact_at TEXT NOT NULL,
            telemetry_events INTEGER NOT NULL DEFAULT 0,
            heartbeats INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (installation_id, contact_day)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS analytics_views (
            id TEXT PRIMARY KEY NOT NULL,
            project_id TEXT NOT NULL,
            owner_user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            name_key TEXT NOT NULL,
            description TEXT,
            visibility TEXT NOT NULL DEFAULT 'private',
            definition TEXT NOT NULL,
            revision INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(project_id, owner_user_id, name_key),
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(owner_user_id) REFERENCES users(id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS member_inventory (
            user_id TEXT PRIMARY KEY NOT NULL,
            evoflux_connected INTEGER NOT NULL DEFAULT 0,
            last_heartbeat_at TEXT,
            agents_count INTEGER NOT NULL DEFAULT 0,
            skills_count INTEGER NOT NULL DEFAULT 0,
            mcp_count INTEGER NOT NULL DEFAULT 0
        )
        "#,
        telemetry_table.as_str(),
        r#"
        CREATE TABLE IF NOT EXISTS telemetry_resource_attributions (
            event_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            version_id TEXT NOT NULL,
            relation TEXT NOT NULL,
            plugin_installation_id TEXT,
            PRIMARY KEY(event_id, resource_id, version_id, relation),
            FOREIGN KEY(event_id) REFERENCES telemetry_events(id),
            FOREIGN KEY(project_id) REFERENCES instance(id),
            FOREIGN KEY(resource_id) REFERENCES resources(id),
            FOREIGN KEY(version_id) REFERENCES resource_versions(id)
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_users_status ON users(status)",
        "CREATE INDEX IF NOT EXISTS idx_users_primary_role ON users(primary_role)",
        "CREATE INDEX IF NOT EXISTS idx_user_tags_tag ON user_tags(tag_id)",
        "CREATE INDEX IF NOT EXISTS idx_tag_assignments_entity ON tag_assignments(entity_type, entity_id)",
        "CREATE INDEX IF NOT EXISTS idx_tag_assignments_tag ON tag_assignments(tag_id)",
        "CREATE INDEX IF NOT EXISTS idx_user_sub_roles_role ON user_sub_roles(sub_role_id)",
        "CREATE INDEX IF NOT EXISTS idx_client_installations_user_seen ON client_installations(user_id, last_seen_at)",
        "CREATE INDEX IF NOT EXISTS idx_client_installations_instance_seen ON client_installations(instance_id, last_seen_at)",
        "CREATE INDEX IF NOT EXISTS idx_client_registration_replay_window ON client_registration_idempotency(instance_id, created_at)",
        "CREATE INDEX IF NOT EXISTS idx_resource_versions_resource ON resource_versions(resource_id, created_at)",
        "CREATE INDEX IF NOT EXISTS idx_resource_access_subject ON resource_access_rules(subject_type, subject_id)",
        "CREATE INDEX IF NOT EXISTS idx_resource_changes_audience ON resource_changes(project_id, effective_user_id, sequence)",
        "CREATE INDEX IF NOT EXISTS idx_resource_changes_resource ON resource_changes(project_id, resource_id, sequence)",
        "CREATE INDEX IF NOT EXISTS idx_resource_version_events_resource ON resource_version_events(project_id, resource_id, created_at)",
        "CREATE INDEX IF NOT EXISTS idx_resource_inventory_state ON installation_resource_inventory(project_id, observed_state, observed_at)",
        "CREATE INDEX IF NOT EXISTS idx_telemetry_resource_time ON telemetry_resource_attributions(project_id, resource_id, version_id)",
        "CREATE INDEX IF NOT EXISTS idx_resource_usage_resource_time ON resource_usage_events(resource_id, occurred_at)",
        "CREATE INDEX IF NOT EXISTS idx_resource_usage_user_time ON resource_usage_events(user_id, occurred_at)",
        "CREATE INDEX IF NOT EXISTS idx_resource_feedback_resource ON resource_feedback(resource_id, updated_at)",
        "CREATE INDEX IF NOT EXISTS idx_analytics_views_project_visibility ON analytics_views(project_id, visibility, updated_at)",
        "CREATE INDEX IF NOT EXISTS idx_analytics_views_owner ON analytics_views(project_id, owner_user_id, updated_at)",
        "CREATE INDEX IF NOT EXISTS idx_model_prices_catalog ON model_prices(catalog_version)",
        "CREATE INDEX IF NOT EXISTS idx_spend_limits_project ON spend_limits(project_id, enabled)",
        "CREATE INDEX IF NOT EXISTS idx_project_ai_policies_project ON project_ai_policies(project_id)",
        "CREATE INDEX IF NOT EXISTS idx_model_price_catalogs_fetched ON model_price_catalogs(fetched_at)",
        "CREATE INDEX IF NOT EXISTS idx_jira_tasks_project ON jira_tasks(project_id)",
        "CREATE INDEX IF NOT EXISTS idx_jira_tasks_parent ON jira_tasks(parent_key)",
        "CREATE INDEX IF NOT EXISTS idx_task_activations_user_started ON task_activations(project_id, user_id, started_at)",
        "CREATE INDEX IF NOT EXISTS idx_jira_status_history_issue ON jira_status_history(project_id, issue_key, changed_at)",
    ];

    for sql in statements {
        sqlx::query(sql.trim()).execute(pool).await?;
    }

    reject_duplicate_connection_token_hashes(pool).await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_connection_secrets_token_hash ON connection_secrets(token_hash)",
    )
    .execute(pool)
    .await?;

    // Best-effort column upgrades for databases created before this revision.
    let telemetry_event_type_alter = format!(
        "ALTER TABLE telemetry_events ADD COLUMN event_type TEXT NOT NULL DEFAULT '{}'",
        TelemetryEventType::ModelCall.as_str(),
    );
    let telemetry_status_alter = format!(
        "ALTER TABLE telemetry_events ADD COLUMN status TEXT NOT NULL DEFAULT '{}'",
        TelemetryEventStatus::Success.as_str(),
    );
    let alters = [
        "ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE users ADD COLUMN session_version INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE users ADD COLUMN sso_issuer TEXT",
        "ALTER TABLE users ADD COLUMN sso_subject TEXT",
        "ALTER TABLE users ADD COLUMN invited_by TEXT",
        "ALTER TABLE users ADD COLUMN approved_at TEXT",
        "ALTER TABLE users ADD COLUMN approved_by TEXT",
        "ALTER TABLE instance ADD COLUMN logo_url TEXT",
        "ALTER TABLE instance ADD COLUMN description TEXT",
        "ALTER TABLE instance ADD COLUMN logo_artifact_key TEXT",
        "ALTER TABLE instance ADD COLUMN logo_content_sha256 TEXT",
        "ALTER TABLE instance ADD COLUMN logo_content_size INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE instance ADD COLUMN logo_media_type TEXT",
        "ALTER TABLE instance ADD COLUMN collection_level TEXT NOT NULL DEFAULT 'L1'",
        "ALTER TABLE instance ADD COLUMN realtime_max_connections INTEGER",
        "ALTER TABLE instance ADD COLUMN realtime_max_per_secret INTEGER",
        "ALTER TABLE instance ADD COLUMN realtime_heartbeat_seconds INTEGER",
        "ALTER TABLE instance ADD COLUMN storage_backend TEXT NOT NULL DEFAULT 'local'",
        "ALTER TABLE instance ADD COLUMN storage_config TEXT NOT NULL DEFAULT '{}'",
        "ALTER TABLE instance ADD COLUMN email_config TEXT NOT NULL DEFAULT '{}'",
        "ALTER TABLE instance ADD COLUMN jira_config TEXT NOT NULL DEFAULT '{}'",
        "ALTER TABLE resources ADD COLUMN status TEXT NOT NULL DEFAULT 'published'",
        "ALTER TABLE resources ADD COLUMN published_at TEXT",
        "ALTER TABLE resources ADD COLUMN project_id TEXT",
        "ALTER TABLE resources ADD COLUMN draft_revision INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE resources ADD COLUMN draft_artifact_key TEXT",
        "ALTER TABLE resources ADD COLUMN draft_content_sha256 TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE resources ADD COLUMN draft_content_size INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE resources ADD COLUMN highest_semver TEXT",
        "ALTER TABLE resources ADD COLUMN release_channel TEXT",
        "ALTER TABLE resource_versions ADD COLUMN project_id TEXT",
        "ALTER TABLE resource_versions ADD COLUMN release_channel TEXT",
        "ALTER TABLE resource_versions ADD COLUMN content_sha256 TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE resource_versions ADD COLUMN content_size INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE resource_versions ADD COLUMN artifact_key TEXT",
        "ALTER TABLE resource_versions ADD COLUMN artifact_schema_version TEXT",
        "ALTER TABLE resource_versions ADD COLUMN minimum_evoflux_version TEXT",
        "ALTER TABLE resource_versions ADD COLUMN deprecated_at TEXT",
        "ALTER TABLE resource_versions ADD COLUMN deprecated_by TEXT",
        "ALTER TABLE resource_versions ADD COLUMN deprecation_reason TEXT",
        "ALTER TABLE resource_access_rules ADD COLUMN project_id TEXT",
        "ALTER TABLE resource_access_rules ADD COLUMN effect TEXT NOT NULL DEFAULT 'allow'",
        "ALTER TABLE telemetry_events ADD COLUMN installation_id TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN project_id TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN request_id TEXT",
        telemetry_event_type_alter.as_str(),
        "ALTER TABLE telemetry_events ADD COLUMN sequence INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE telemetry_events ADD COLUMN agent_name TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN provider TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN model TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN response_model TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN cache_read_tokens INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE telemetry_events ADD COLUMN reasoning_tokens INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE telemetry_events ADD COLUMN tool_use_tokens INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE telemetry_events ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE telemetry_events ADD COLUMN tool_name TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN tool_category TEXT",
        telemetry_status_alter.as_str(),
        "ALTER TABLE telemetry_events ADD COLUMN error_category TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN estimated_cost_usd_micros BIGINT",
        "ALTER TABLE telemetry_events ADD COLUMN cost_source TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN evoflux_version TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN primary_role_snapshot TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN sub_role_ids_snapshot TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN tag_ids_snapshot TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN received_at TEXT",
        // Conductor-computed cost. `estimated_cost_usd_micros` stays as the
        // client's own figure so the two can be compared; these three are the
        // authoritative value, the rate snapshot that produced it, and why it
        // is absent when it is.
        "ALTER TABLE telemetry_events ADD COLUMN server_cost_usd_micros BIGINT",
        "ALTER TABLE telemetry_events ADD COLUMN priced_catalog_version TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN unpriced_reason TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN cache_write_tokens INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE telemetry_events ADD COLUMN service_tier TEXT",
        "ALTER TABLE telemetry_events ADD COLUMN pricing_basis TEXT",
        "ALTER TABLE model_prices ADD COLUMN tiers_json TEXT",
        "ALTER TABLE model_prices ADD COLUMN service_tiers_json TEXT",
        "ALTER TABLE users ADD COLUMN jira_account_email TEXT",
        "ALTER TABLE jira_tasks ADD COLUMN assignee_email TEXT",
        "ALTER TABLE jira_tasks ADD COLUMN resolved_project TEXT",
    ];
    for sql in alters {
        let _ = sqlx::query(sql).execute(pool).await;
    }

    widen_money_columns(pool, kind).await?;

    // Backfill singleton-project rows created before project-scoped resources.
    for sql in [
        "UPDATE resources SET project_id = (SELECT id FROM instance LIMIT 1) WHERE project_id IS NULL OR project_id = ''",
        "UPDATE resource_versions SET project_id = (SELECT project_id FROM resources WHERE resources.id = resource_versions.resource_id) WHERE project_id IS NULL OR project_id = ''",
        "UPDATE resource_access_rules SET project_id = (SELECT project_id FROM resources WHERE resources.id = resource_access_rules.resource_id) WHERE project_id IS NULL OR project_id = ''",
        "UPDATE resources SET highest_semver = version WHERE highest_semver IS NULL AND status IN ('beta', 'published')",
        "UPDATE resources SET kind = 'plugin' WHERE kind = 'mcp'",
        // ResourceKind::Agent was renamed to ResourceKind::AgentTeam without a
        // data migration (commit 5de2ad5); rows written before that commit
        // still carry the old string and fail to parse, taking down
        // resources.list with a 500 for the whole project.
        "UPDATE resources SET kind = 'agent_team' WHERE kind = 'agent'",
    ] {
        let _ = sqlx::query(sql).execute(pool).await;
    }

    // Saved analytics views are loaded strictly: one definition that no longer
    // parses fails the whole list, not just its own row. Tool identity has
    // been retired from telemetry, so a stored view that groups by it or
    // filters on it would take the views list down for everyone in that
    // project. Drop the group-by views, which can no longer render anything,
    // and strip the retired filter from the rest so they keep working.
    for sql in [
        "DELETE FROM analytics_views WHERE definition LIKE '%\"group_by\":\"tool\"%'",
        "UPDATE analytics_views SET definition = REPLACE(definition, ',\"tool_name\":null', '') WHERE definition LIKE '%\"tool_name\"%'",
        "UPDATE analytics_views SET definition = REPLACE(definition, '\"tool_name\":null,', '') WHERE definition LIKE '%\"tool_name\"%'",
    ] {
        let _ = sqlx::query(sql).execute(pool).await;
    }

    for sql in [
        "CREATE INDEX IF NOT EXISTS idx_telemetry_user_time ON telemetry_events(user_id, reported_at)",
        "CREATE INDEX IF NOT EXISTS idx_telemetry_request ON telemetry_events(user_id, request_id)",
        "CREATE INDEX IF NOT EXISTS idx_telemetry_installation_time ON telemetry_events(installation_id, reported_at)",
        "CREATE INDEX IF NOT EXISTS idx_telemetry_project_received ON telemetry_events(project_id, received_at)",
        "CREATE INDEX IF NOT EXISTS idx_contact_days_project ON installation_contact_days(project_id, contact_day)",
        "CREATE INDEX IF NOT EXISTS idx_telemetry_resource_time ON telemetry_resource_attributions(project_id, resource_id, version_id)",
    ] {
        sqlx::query(sql).execute(pool).await?;
    }

    // The columns may have been added just above for an older database.
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_sso_identity ON users(sso_issuer, sso_subject)",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "UPDATE resources SET published_at = created_at WHERE status = 'published' AND published_at IS NULL",
    )
    .execute(pool)
    .await;

    remove_legacy_initial_draft_versions(pool).await?;
    backfill_resource_versions(pool).await?;

    // Upgrade legacy member-only tag links into the generic assignment model.
    let _ = sqlx::query(
        r#"
        INSERT INTO tag_assignments (tag_id, entity_type, entity_id, created_at)
        SELECT tag_id, 'member', user_id, '1970-01-01T00:00:00Z'
        FROM user_tags
        "#,
    )
    .execute(pool)
    .await;

    Ok(())
}

async fn reject_duplicate_connection_token_hashes(pool: &Pool<Any>) -> Result<(), sqlx::Error> {
    let duplicate_groups: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM (
            SELECT token_hash
            FROM connection_secrets
            GROUP BY token_hash
            HAVING COUNT(*) > 1
        ) duplicate_token_hashes
        "#,
    )
    .fetch_one(pool)
    .await?;

    if duplicate_groups > 0 {
        return Err(sqlx::Error::Protocol(
            "connection credential uniqueness check failed".into(),
        ));
    }
    Ok(())
}

async fn backfill_resource_versions(pool: &Pool<Any>) -> Result<(), sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT r.id, r.project_id, r.version, r.status, r.payload, r.owner_user_id,
               r.created_at, r.published_at
        FROM resources r
        WHERE r.status IN ('beta', 'published') AND NOT EXISTS (
            SELECT 1 FROM resource_versions rv WHERE rv.resource_id = r.id
        )
        "#,
    )
    .fetch_all(pool)
    .await?;

    for row in rows {
        let resource_id: String = row.get("id");
        let resource_status: String = row.get("status");
        let created_at: String = row.get("created_at");
        let published_at: Option<String> = row.get("published_at");
        let version_status = resource_status.as_str();
        let created_by = row
            .get::<Option<String>, _>("owner_user_id")
            .unwrap_or_else(|| Uuid::nil().to_string());

        sqlx::query(
            r#"
            INSERT INTO resource_versions (
                id, project_id, resource_id, version, status, payload, changelog,
                release_channel, content_sha256, content_size, created_by,
                created_at, published_at
            ) VALUES (?, ?, ?, ?, ?, ?, NULL, ?, '', 0, ?, ?, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(row.get::<String, _>("project_id"))
        .bind(resource_id)
        .bind(row.get::<String, _>("version"))
        .bind(version_status)
        .bind(row.get::<String, _>("payload"))
        .bind(Some(version_status))
        .bind(created_by)
        .bind(created_at)
        .bind(published_at)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn remove_legacy_initial_draft_versions(pool: &Pool<Any>) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM resource_versions
        WHERE status = 'draft'
          AND release_channel IS NULL
          AND content_sha256 = ''
          AND EXISTS (
              SELECT 1 FROM resources r
              WHERE r.id = resource_versions.resource_id
                AND r.status = 'draft'
                AND r.highest_semver IS NULL
          )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::any::AnyPoolOptions;

    use super::*;

    #[tokio::test]
    async fn migration_rejects_duplicate_token_hashes_without_exposing_them() {
        sqlx::any::install_default_drivers();
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");
        run(&pool, DatabaseKind::Sqlite)
            .await
            .expect("run initial migration");

        sqlx::query("DROP INDEX idx_connection_secrets_token_hash")
            .execute(&pool)
            .await
            .expect("simulate pre-index schema");
        let hash_canary = "MIGRATION_DUPLICATE_TOKEN_HASH_CANARY_never_serialize";
        for owner in ["first-owner", "second-owner"] {
            sqlx::query(
                r#"
                INSERT INTO connection_secrets (
                    id, name, prefix, token_hash, owner_user_id, scopes, created_at
                ) VALUES (?, 'migration canary', 'evc_test', ?, ?,
                          '["subscribe_resources"]', '2026-08-17T00:00:00Z')
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(hash_canary)
            .bind(owner)
            .execute(&pool)
            .await
            .expect("seed duplicate legacy credential");
        }

        let error = run(&pool, DatabaseKind::Sqlite)
            .await
            .expect_err("migration must reject ambiguous token hashes");
        let rendered = error.to_string();
        assert!(rendered.contains("credential uniqueness check failed"));
        assert!(!rendered.contains(hash_canary));
    }

    #[tokio::test]
    async fn adds_description_to_an_existing_instance_table() {
        sqlx::any::install_default_drivers();
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");

        sqlx::query(
            r#"
            CREATE TABLE instance (
                id TEXT PRIMARY KEY NOT NULL,
                project_name TEXT NOT NULL,
                display_name TEXT,
                bind_host TEXT NOT NULL,
                bind_port INTEGER NOT NULL,
                public_url TEXT,
                logo_url TEXT,
                logo_artifact_key TEXT,
                logo_content_sha256 TEXT,
                logo_content_size INTEGER NOT NULL DEFAULT 0,
                logo_media_type TEXT,
                collection_level TEXT NOT NULL DEFAULT 'L1',
                storage_backend TEXT NOT NULL DEFAULT 'local',
                storage_config TEXT NOT NULL DEFAULT '{}',
                setup_completed INTEGER NOT NULL DEFAULT 0,
                jwt_secret TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create pre-description schema");
        sqlx::query(
            r#"
            INSERT INTO instance (
                id, project_name, display_name, bind_host, bind_port,
                setup_completed, jwt_secret, created_at, updated_at
            ) VALUES ('project-id', 'Legacy project', 'Legacy', '127.0.0.1',
                      4700, 1, 'test-secret', '2026-08-12T00:00:00Z',
                      '2026-08-12T00:00:00Z')
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert legacy project");

        run(&pool, DatabaseKind::Sqlite)
            .await
            .expect("upgrade schema");

        let description = sqlx::query_scalar::<_, Option<String>>(
            "SELECT description FROM instance WHERE id = 'project-id'",
        )
        .fetch_one(&pool)
        .await
        .expect("read migrated description");
        assert_eq!(description, None);
    }

    #[tokio::test]
    async fn rerunning_migrations_removes_legacy_initial_draft_version() {
        sqlx::any::install_default_drivers();
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");
        run(&pool, DatabaseKind::Sqlite)
            .await
            .expect("run initial migration");

        let project_id = Uuid::new_v4().to_string();
        let owner_id = Uuid::new_v4().to_string();
        let resource_id = Uuid::new_v4().to_string();
        let version_id = Uuid::new_v4().to_string();
        let now = "2026-08-11T00:00:00Z";

        sqlx::query(
            r#"
            INSERT INTO instance (
                id, project_name, bind_host, bind_port, setup_completed, jwt_secret,
                created_at, updated_at
            ) VALUES (?, 'Migration test', '127.0.0.1', 4700, 1, 'test', ?, ?)
            "#,
        )
        .bind(&project_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert project");

        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, display_name, primary_role, status, created_at
            ) VALUES (?, 'owner@example.test', 'Owner', 'admin', 'active', ?)
            "#,
        )
        .bind(&owner_id)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert owner");

        sqlx::query(
            r#"
            INSERT INTO resources (
                id, project_id, kind, slug, name, version, owner_user_id,
                visibility, status, payload, draft_revision, highest_semver,
                created_at, updated_at
            ) VALUES (?, ?, 'plugin', 'draft-plugin', 'Draft Plugin', '0.1.0', ?,
                      'shared', 'draft', '{}', 0, NULL, ?, ?)
            "#,
        )
        .bind(&resource_id)
        .bind(&project_id)
        .bind(&owner_id)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert draft resource");

        // Older builds persisted a placeholder draft version during resource creation.
        // It is not a release and must not reserve 0.1.0 after an upgrade or restart.
        sqlx::query(
            r#"
            INSERT INTO resource_versions (
                id, project_id, resource_id, version, status, payload,
                content_sha256, content_size, created_by, created_at
            ) VALUES (?, ?, ?, '0.1.0', 'draft', '{}', '', 0, ?, ?)
            "#,
        )
        .bind(&version_id)
        .bind(&project_id)
        .bind(&resource_id)
        .bind(&owner_id)
        .bind(now)
        .execute(&pool)
        .await
        .expect("insert legacy draft version");

        run(&pool, DatabaseKind::Sqlite)
            .await
            .expect("rerun migration");

        let version_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM resource_versions WHERE resource_id = ?",
        )
        .bind(&resource_id)
        .fetch_one(&pool)
        .await
        .expect("count versions");
        assert_eq!(version_count, 0);
    }
}
