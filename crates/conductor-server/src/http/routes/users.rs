use axum::{extract::State, Json};
use conductor_domain::User;

use crate::http::error::ApiResult;
use crate::http::extractors::AuthUser;
use crate::http::state::AppState;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> ApiResult<Json<Vec<User>>> {
    Ok(Json(state.db.users().list().await?))
}
