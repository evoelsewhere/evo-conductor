use axum::{extract::State, Json};
use conductor_domain::DashboardSummary;

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

pub async fn summary(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<DashboardSummary>> {
    Ok(Json(state.db.dashboard().summary().await?))
}
