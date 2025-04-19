//! # The Command-Line Arguments

use crate::constants::{DEFAULT_MAX_CONNECTIONS, DEFAULT_PORT};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "Redis Server")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The server port
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// Maximum number of allowed parallel connections from clients
    #[arg(long, default_value_t = DEFAULT_MAX_CONNECTIONS)]
    pub max_conn: usize,
}
