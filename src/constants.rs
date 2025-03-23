//! # Constants and Types
//!
//! Constants and types used throughout the application

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Local host IPv4 address and Redis port
pub const LOCAL_SOCKET_ADDR_STR: &str = "127.0.0.1:6379";

/// Local host IPv4 address and arbitrary port chosen by OS
pub const LOCAL_SOCKET_ADDR_STR_TEST: &str = "127.0.0.1:0";

/// Length of buffer for handling connections, 512 bytes
pub const BUFFER_LEN: usize = 512;

pub const COMMANDS: [&[u8]; 4] = [b"ECHO", b"GET", b"PING", b"SET"];

/// Application exit codes
#[derive(Debug)]
pub enum ExitCode {
    Ok = 0,
    Shutdown = -1,
}

pub type StorageKey = String;
pub type StorageValue = String;
type InMemoryStorageHashMap = HashMap<StorageKey, StorageValue>;
pub type StorageType = InMemoryStorageHashMap;
pub type ConcurrentStorageType = Arc<RwLock<StorageType>>;
