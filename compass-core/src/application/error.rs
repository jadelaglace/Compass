//! Application errors independent of any transport protocol.

#[derive(Debug)]
pub(crate) enum AppError {
    NotFound(String),
    BadRequest(String),
    Unprocessable(String),
    Conflict { code: String, message: String },
    Internal(String),
}

impl AppError {
    pub(crate) fn not_found(id: &str) -> Self {
        Self::NotFound(id.to_string())
    }

    pub(crate) fn bad_request(message: &str) -> Self {
        Self::BadRequest(message.to_string())
    }

    pub(crate) fn unprocessable(message: &str) -> Self {
        Self::Unprocessable(message.to_string())
    }

    pub(crate) fn conflict(code: &str, message: &str) -> Self {
        Self::Conflict {
            code: code.to_string(),
            message: message.to_string(),
        }
    }

    pub(crate) fn internal(message: &str) -> Self {
        Self::Internal(message.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error.to_string())
    }
}
