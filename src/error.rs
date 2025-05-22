//! Error types for gamecode-context

use thiserror::Error;

/// Result type for context operations
pub type Result<T> = std::result::Result<T, ContextError>;

/// Errors that can occur during context management
#[derive(Error, Debug)]
pub enum ContextError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid session data: {0}")]
    InvalidSession(String),

    #[error("Context compaction failed: {0}")]
    CompactionFailed(String),

    #[error("Token estimation failed: {0}")]
    TokenEstimation(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Configuration error: {0}")]
    Config(String),
}