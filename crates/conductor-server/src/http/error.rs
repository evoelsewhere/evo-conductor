use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use conductor_domain::ConductorError;
use serde_json::json;

pub struct ApiError(pub ConductorError);

impl From<ConductorError> for ApiError {
    fn from(value: ConductorError) -> Self {
        Self(value)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        tracing::error!(error = %value, "database request failed");
        Self(ConductorError::Internal)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::BAD_REQUEST);
        let body = Json(json!({
            "error": self.0.to_string(),
            "code": self.0.status_code(),
        }));
        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
