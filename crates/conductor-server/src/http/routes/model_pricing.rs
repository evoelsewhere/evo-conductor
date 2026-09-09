//! World AI pricing catalog — a reference-price screen, not a usage/spend
//! report (see `resources::resource_usage_analytics` for that). Every
//! authenticated member may read it: it is public data from models.dev, not
//! scoped to any project or organization.

use axum::extract::State;
use axum::Json;
use conductor_domain::ModelPricingCatalogSnapshot;

use crate::core::error::ApiResult;
use crate::core::state::AppState;

pub async fn list_catalog(
    State(state): State<AppState>,
) -> ApiResult<Json<ModelPricingCatalogSnapshot>> {
    Ok(Json(state.pricing.snapshot().await))
}

pub async fn refresh_catalog(
    State(state): State<AppState>,
) -> ApiResult<Json<ModelPricingCatalogSnapshot>> {
    Ok(Json(state.pricing.refresh().await))
}
