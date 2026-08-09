use chrono::{DateTime, Utc};
use conductor_domain::{
    ManagedResource, PrimaryRole, ResourceKind, ResourceStatus, ResourceVisibility, User,
    UserStatus,
};
use sqlx::any::AnyRow;
use sqlx::Row;
use uuid::Uuid;

pub fn parse_dt(value: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

pub fn map_user_row(r: &AnyRow) -> Result<User, sqlx::Error> {
    let id_str: String = r.get("id");
    let role_str: String = r.get("primary_role");
    let status_str: String = r.get("status");
    let last_seen: Option<String> = r.get("last_seen_at");
    let must_change: i64 = r.try_get("must_change_password").unwrap_or(0);

    Ok(User {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::nil()),
        email: r.get("email"),
        display_name: r.get("display_name"),
        primary_role: PrimaryRole::parse(&role_str).unwrap_or(PrimaryRole::User),
        sub_role_ids: vec![],
        tag_ids: vec![],
        status: UserStatus::parse(&status_str),
        must_change_password: must_change == 1,
        last_seen_at: last_seen.map(parse_dt),
        created_at: parse_dt(r.get("created_at")),
    })
}

pub fn map_resource(r: &AnyRow) -> Result<ManagedResource, sqlx::Error> {
    let kind_str: String = r.get("kind");
    let visibility: String = r.get("visibility");
    let payload: String = r.get("payload");
    let owner: Option<String> = r.get("owner_user_id");
    let published_at: Option<String> = r.get("published_at");

    Ok(ManagedResource {
        id: Uuid::parse_str(r.get::<String, _>("id").as_str()).unwrap_or_else(|_| Uuid::nil()),
        kind: match kind_str.as_str() {
            "skill" => ResourceKind::Skill,
            "mcp" => ResourceKind::Mcp,
            "workflow" => ResourceKind::Workflow,
            "command" => ResourceKind::Command,
            _ => ResourceKind::Agent,
        },
        slug: r.get("slug"),
        name: r.get("name"),
        description: r.get("description"),
        version: r.get("version"),
        owner_user_id: owner.and_then(|s| Uuid::parse_str(&s).ok()),
        visibility: if visibility == "private" {
            ResourceVisibility::Private
        } else {
            ResourceVisibility::Shared
        },
        status: ResourceStatus::parse(r.get::<String, _>("status").as_str()),
        payload: serde_json::from_str(&payload).unwrap_or(serde_json::json!({})),
        published_at: published_at.map(parse_dt),
        created_at: parse_dt(r.get("created_at")),
        updated_at: parse_dt(r.get("updated_at")),
    })
}
