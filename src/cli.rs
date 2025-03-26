//! # The Command-Line Arguments

use crate::constants::DEFAULT_PORT;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "Redis Server")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// The server port
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    pub port: u16,
}
