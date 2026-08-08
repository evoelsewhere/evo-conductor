use sqlx::{Any, Pool};

/// Portable schema (TEXT ids, INTEGER flags) — works on SQLite, Postgres, MySQL.
pub async fn run(pool: &Pool<Any>) -> Result<(), sqlx::Error> {
    let statements = [
        r#"
        CREATE TABLE IF NOT EXISTS instance (
            id TEXT PRIMARY KEY NOT NULL,
            project_name TEXT NOT NULL,
            display_name TEXT,
            bind_host TEXT NOT NULL,
            bind_port INTEGER NOT NULL,
            public_url TEXT,
            logo_url TEXT,
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
            invited_by TEXT,
            approved_at TEXT,
            approved_by TEXT,
            last_seen_at TEXT,
            created_at TEXT NOT NULL
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
        CREATE TABLE IF NOT EXISTS resources (
            id TEXT PRIMARY KEY NOT NULL,
            kind TEXT NOT NULL,
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            version TEXT NOT NULL DEFAULT '0.1.0',
            owner_user_id TEXT,
            visibility TEXT NOT NULL DEFAULT 'shared',
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(kind, slug)
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
        r#"
        CREATE TABLE IF NOT EXISTS telemetry_events (
            id TEXT PRIMARY KEY NOT NULL,
            user_id TEXT NOT NULL,
            session_id TEXT,
            tokens_in INTEGER NOT NULL DEFAULT 0,
            tokens_out INTEGER NOT NULL DEFAULT 0,
            tool_calls INTEGER NOT NULL DEFAULT 0,
            active_agents INTEGER NOT NULL DEFAULT 0,
            reported_at TEXT NOT NULL
        )
        "#,
        "CREATE INDEX IF NOT EXISTS idx_users_status ON users(status)",
        "CREATE INDEX IF NOT EXISTS idx_users_primary_role ON users(primary_role)",
        "CREATE INDEX IF NOT EXISTS idx_user_tags_tag ON user_tags(tag_id)",
        "CREATE INDEX IF NOT EXISTS idx_tag_assignments_entity ON tag_assignments(entity_type, entity_id)",
        "CREATE INDEX IF NOT EXISTS idx_tag_assignments_tag ON tag_assignments(tag_id)",
        "CREATE INDEX IF NOT EXISTS idx_user_sub_roles_role ON user_sub_roles(sub_role_id)",
    ];

    for sql in statements {
        sqlx::query(sql.trim()).execute(pool).await?;
    }

    // Best-effort column upgrades for databases created before this revision.
    let alters = [
        "ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE users ADD COLUMN invited_by TEXT",
        "ALTER TABLE users ADD COLUMN approved_at TEXT",
        "ALTER TABLE users ADD COLUMN approved_by TEXT",
        "ALTER TABLE instance ADD COLUMN logo_url TEXT",
    ];
    for sql in alters {
        let _ = sqlx::query(sql).execute(pool).await;
    }

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
