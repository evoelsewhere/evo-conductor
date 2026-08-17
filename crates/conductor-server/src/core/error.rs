use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use conductor_domain::ConductorError;
use conductor_storage::StorageError;
use serde_json::json;

use crate::core::request_context::current as current_request_context;

pub struct ApiError {
    pub error: ConductorError,
    public_error_code: Option<&'static str>,
}

impl ApiError {
    pub fn with_public_code(error: ConductorError, public_error_code: &'static str) -> Self {
        Self {
            error,
            public_error_code: Some(public_error_code),
        }
    }

    pub fn scope_denied() -> Self {
        Self::with_public_code(ConductorError::Forbidden, "scope_denied")
    }

    pub fn conflict(public_error_code: &'static str, message: impl Into<String>) -> Self {
        Self::with_public_code(ConductorError::Conflict(message.into()), public_error_code)
    }
}

impl From<ConductorError> for ApiError {
    fn from(value: ConductorError) -> Self {
        Self {
            error: value,
            public_error_code: None,
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        tracing::error!(error = %value, "database request failed");
        Self::from(ConductorError::Internal)
    }
}

impl From<StorageError> for ApiError {
    fn from(value: StorageError) -> Self {
        match value {
            StorageError::InvalidPersistedPrincipal(error) => {
                tracing::error!(
                    row_id = ?error.row_id,
                    field = error.field.as_str(),
                    reason = error.reason.as_str(),
                    "invalid persisted principal rejected"
                );
                Self::from(ConductorError::Unauthorized)
            }
            StorageError::InvalidPersistedCredential(error) => {
                tracing::error!(
                    credential_id = ?error.credential_id,
                    field = error.field.as_str(),
                    reason = error.reason.as_str(),
                    "invalid persisted credential rejected"
                );
                if error.field == conductor_storage::PersistedCredentialField::TokenHash
                    && error.reason == conductor_storage::PersistedSecurityReason::DuplicateValue
                {
                    Self::from(ConductorError::Internal)
                } else {
                    Self::from(ConductorError::Unauthorized)
                }
            }
            StorageError::InvalidPersistedResource(error) => {
                tracing::error!(
                    resource_id = ?error.resource_id,
                    field = error.field.as_str(),
                    reason = error.reason.as_str(),
                    "invalid persisted resource rejected"
                );
                Self::from(ConductorError::Internal)
            }
            StorageError::Database(error) => Self::from(error),
            StorageError::Serialization(error) => {
                tracing::error!(error = %error, "storage serialization failed");
                Self::from(ConductorError::Internal)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.error.status_code()).unwrap_or(StatusCode::BAD_REQUEST);
        let request_id = current_request_context().map(|context| context.request_id);
        let error = if status.is_server_error() {
            "internal server error".to_owned()
        } else {
            self.error.to_string()
        };
        let public_error_code = self
            .public_error_code
            .unwrap_or_else(|| default_public_error_code(&self.error));
        let body = Json(json!({
            "error": error,
            "code": self.error.status_code(),
            "error_code": public_error_code,
            "request_id": request_id,
        }));
        (status, body).into_response()
    }
}

fn default_public_error_code(error: &ConductorError) -> &'static str {
    match error {
        ConductorError::Message(_) => "invalid_request",
        ConductorError::NotFound(_) => "not_found",
        ConductorError::Unauthorized | ConductorError::InvalidCredentials => "unauthorized",
        ConductorError::Forbidden => "permission_denied",
        ConductorError::Conflict(_) => "conflict",
        ConductorError::SetupAlreadyCompleted => "setup_already_completed",
        ConductorError::SetupRequired => "setup_required",
        ConductorError::Internal | ConductorError::Other(_) => "internal_error",
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use conductor_storage::{
        InvalidPersistedCredential, InvalidPersistedResource, PersistedCredentialField,
        PersistedResourceField, PersistedSecurityReason,
    };

    use super::*;

    #[test]
    fn corrupt_resource_state_is_an_internal_error_not_an_authentication_denial() {
        let response = ApiError::from(StorageError::InvalidPersistedResource(
            InvalidPersistedResource::new(
                None,
                PersistedResourceField::Id,
                PersistedSecurityReason::InvalidUuid,
            ),
        ))
        .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn duplicate_token_hash_is_an_internal_error_not_an_arbitrary_authentication_result() {
        let response = ApiError::from(StorageError::InvalidPersistedCredential(
            InvalidPersistedCredential::new(
                None,
                PersistedCredentialField::TokenHash,
                PersistedSecurityReason::DuplicateValue,
            ),
        ))
        .into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
