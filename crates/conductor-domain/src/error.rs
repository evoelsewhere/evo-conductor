use thiserror::Error;

pub type Result<T> = std::result::Result<T, ConductorError>;

#[derive(Debug, Error)]
pub enum ConductorError {
    #[error("{0}")]
    Message(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("setup already completed")]
    SetupAlreadyCompleted,

    #[error("instance not configured — complete setup first")]
    SetupRequired,

    #[error("invalid credentials")]
    InvalidCredentials,

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl ConductorError {
    pub fn msg(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::NotFound(_) => 404,
            Self::Unauthorized | Self::InvalidCredentials => 401,
            Self::Forbidden => 403,
            Self::Conflict(_) | Self::SetupAlreadyCompleted => 409,
            Self::SetupRequired => 428,
            Self::Message(_) | Self::Other(_) => 400,
        }
    }
}
