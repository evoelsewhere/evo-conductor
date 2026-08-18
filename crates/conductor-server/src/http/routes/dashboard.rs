use axum::{extract::State, Json};
use chrono::Utc;
use conductor_domain::{
    ConductorError, DashboardSummary, PrimaryRole, DASHBOARD_PRESENCE_THRESHOLD_SECONDS,
};

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

pub async fn summary(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<DashboardSummary>> {
    let feedback_owner_user_id = match user.primary_role {
        PrimaryRole::Admin => None,
        PrimaryRole::Contribute => Some(user.id),
        PrimaryRole::User => return Err(ConductorError::Forbidden.into()),
    };
    let mut summary = state
        .db
        .dashboard()
        .summary_at(
            Utc::now(),
            DASHBOARD_PRESENCE_THRESHOLD_SECONDS,
            feedback_owner_user_id,
        )
        .await?;
    summary.realtime.active_owners = count(state.realtime.active_owners());
    summary.realtime.active_streams = count(state.realtime.active_connections());
    summary.host_metrics = state.host_metrics.sample();
    Ok(Json(summary))
}

fn count(value: usize) -> u32 {
    value.try_into().unwrap_or(u32::MAX)
}
