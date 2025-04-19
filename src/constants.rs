//! # Constants
//!
//! Constants used throughout the application

use crate::types::ExpirationTimeType;

/// Default server port
pub const DEFAULT_PORT: u16 = 6379;
/// Local host IPv4 address
pub const LOCAL_SOCKET_ADDR_STR: &str = "127.0.0.1";
/// Local host IPv4 address and Redis port
pub const LOCAL_SOCKET_ADDR_PORT_STR: &str = "127.0.0.1:6379";

/// Local host IPv4 address and arbitrary port chosen by OS
pub const LOCAL_SOCKET_ADDR_STR_TEST: &str = "127.0.0.1:0";

/// Default maximum number of allowed concurrent connections from clients
pub const DEFAULT_MAX_CONNECTIONS: usize = 10;
/// Connection permit timeout in milliseconds
pub const CONNECTION_PERMIT_TIMEOUT_MS: u64 = 5000;

/// Supported Redis commands
pub const COMMANDS: [&[u8]; 4] = [b"ECHO", b"GET", b"PING", b"SET"];

/// Time period in milliseconds for checking of expired keys
pub const HZ_MS: ExpirationTimeType = 100;

/// Length of buffer for handling connections, 512 bytes
pub const BUFFER_LEN: usize = 512;

/// Application exit codes
#[derive(Debug)]
pub enum ExitCode {
    Ok = 0,
    Shutdown = -1,
}
