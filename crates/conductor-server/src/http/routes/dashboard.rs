use axum::{extract::State, Json};
use conductor_domain::DashboardSummary;

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

pub async fn summary(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<DashboardSummary>> {
    Ok(Json(state.db.dashboard().summary().await?))
}
