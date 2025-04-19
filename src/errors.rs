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
    ServerError(#[from] ServerError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::server`]
#[derive(Debug, Error)]
pub enum ServerError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("couldn't obtain permit on time: {0}")]
    // ElapsedError(#[from] tokio::time::error::Elapsed), // TODO remove
    ElapsedError(String),

    #[error("couldn't obtain permit: {0}")]
    AcquireError(#[from] tokio::sync::AcquireError),

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
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("Clock may have gone backwards: {0}")]
    TimeError(#[from] std::time::SystemTimeError),

    #[error(transparent)]
    RESPError(#[from] RESPError),

    #[error("Input too short: {0}")]
    InputTooShort(String),

    #[error("CMD: CRLF (\\r\\n) characters not present at end")]
    CRLFNotAtEnd,

    #[error("Null Array")]
    NullArray,

    #[error("Command is not Array")]
    CmdNotArray,

    #[error("Empty Array")]
    EmptyArray,

    #[error("Not all words are Bulk Strings")]
    NotAllBulk,

    #[error("Command missing argument")]
    MissingArg,

    #[error("Unrecognized command: {0}")]
    UnrecognizedCmd(String),

    #[error("Wrong argument: {0}")]
    WrongArg(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Errors related to working with [`crate::resp`]
#[derive(Debug, Error)]
pub enum RESPError {
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

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

    #[error("RESP: CRLF (\\r\\n) characters not present at end")]
    CRLFNotAtEnd,

    #[error("Received negative length")]
    NegativeLength,

    #[error("Couldn't parse {0} to integer")]
    IntegerParseError(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
