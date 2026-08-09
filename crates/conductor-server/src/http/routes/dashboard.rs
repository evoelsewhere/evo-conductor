use axum::{extract::State, Json};
use conductor_domain::DashboardSummary;

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

pub async fn summary(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<DashboardSummary>> {
    let mut summary = state.db.dashboard().summary().await?;
    summary.members_online = state
        .realtime
        .active_owners()
        .try_into()
        .unwrap_or(u32::MAX);
    Ok(Json(summary))
}
