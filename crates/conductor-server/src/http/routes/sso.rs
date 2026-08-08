use axum::{extract::State, Json};
use conductor_domain::{ConductorError, SsoConfig};

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

pub async fn get_config(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<SsoConfig>> {
    if !user.primary_role.can_manage_members() {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(Json(state.db.instance().sso_config().await?))
}
