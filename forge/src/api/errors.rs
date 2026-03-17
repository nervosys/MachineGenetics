use std::fmt;

/// Errors returned by the Forge API.
#[derive(Debug)]
pub enum ApiError {
    /// Requested resource does not exist.
    NotFound(String),
    /// Request payload is malformed or invalid.
    BadRequest(String),
    /// Caller lacks required permissions.
    Unauthorized(String),
    /// Version conflict (e.g. duplicate publish).
    Conflict(String),
    /// Unexpected internal error.
    Internal(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::NotFound(msg) => write!(f, "not found: {}", msg),
            ApiError::BadRequest(msg) => write!(f, "bad request: {}", msg),
            ApiError::Unauthorized(msg) => write!(f, "unauthorized: {}", msg),
            ApiError::Conflict(msg) => write!(f, "conflict: {}", msg),
            ApiError::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = ApiError::NotFound("module 'foo'".to_string());
        assert_eq!(err.to_string(), "not found: module 'foo'");
    }
}
