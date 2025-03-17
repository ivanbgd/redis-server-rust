//! # Errors
//!
//! Error types and helper functions used in the library

use thiserror::Error;

/// Application errors
#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    RequestError(#[from] RequestError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::requests`]
#[derive(Debug, Error)]
pub enum RequestError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("missing CRLF at the end of simple string")]
    MissingCRLF,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::cmd`]
#[derive(Debug, Error)]
pub enum CmdError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
