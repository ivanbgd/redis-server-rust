//! # Errors
//!
//! Error types and helper functions used in the library

use std::string::FromUtf8Error;
use thiserror::Error;

/// Application errors
#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::conn`]
#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    CmdError(#[from] CmdError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::cmd`]
#[derive(Debug, Error)]
pub enum CmdError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),

    #[error(transparent)]
    RESPError(#[from] RESPError),

    #[error("Input too short: {0}")]
    InputTooShort(String),

    #[error("Command is not Array")]
    CmdNotArray,

    #[error("Empty Array")]
    EmptyArray,

    #[error("Null Array")]
    NullArray,

    #[error("Not all words are Bulk Strings")]
    NotAllBulk,

    #[error("Unrecognized command: {0}")]
    UnrecognizedCmd(String),

    #[error("CRLF (\\r\\n) characters not at end")]
    CRLFNotAtEnd,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::resp`]
#[derive(Debug, Error)]
pub enum RESPError {
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),

    #[error("A request must be of the RESP Array type")]
    NotArray,

    #[error("Unsupported RESP type: {0}")]
    UnsupportedRESPType(u8),

    #[error("Missing the CR (\\r) character")]
    CRMissing,

    #[error("Excess CR (\\r) character")]
    CRExcess,

    #[error("Missing the LF (\\n) character")]
    LFMissing,

    #[error("Excess LF (\\n) character")]
    LFExcess,

    #[error("Missing the CRLF (\\r\\n) characters at beginning")]
    CRLFMissing,

    #[error("CRLF (\\r\\n) characters not at end")]
    CRLFNotAtEnd,

    #[error("Received negative length")]
    NegativeLength,

    #[error("Couldn't parse {0} to integer")]
    IntegerParseError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
