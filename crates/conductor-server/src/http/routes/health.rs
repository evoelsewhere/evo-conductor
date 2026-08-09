use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::core::error::ApiResult;
use crate::core::state::AppState;

pub async fn health(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({
        "status": "ok",
        "service": "evo-conductor",
        "version": env!("CARGO_PKG_VERSION"),
        "database": state.db.kind().as_str(),
    })))
}
