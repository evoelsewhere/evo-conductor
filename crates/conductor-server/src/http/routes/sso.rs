use axum::{extract::State, Json};
use conductor_domain::SsoConfig;

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

pub async fn get_config(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<SsoConfig>> {
    Ok(Json(state.db.instance().sso_config().await?))
}
