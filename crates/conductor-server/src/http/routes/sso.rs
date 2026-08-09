use axum::{extract::State, Json};
use conductor_domain::{ConductorError, SsoConfig};

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::extractors::AuthUser;

pub async fn get_config(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<SsoConfig>> {
    if !user.primary_role.can_manage_settings() {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(Json(state.db.instance().sso_config().await?))
}
