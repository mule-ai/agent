//! AGI Agent CLI - Interactive Chat and Training
//! 
//! Run with: cargo run --bin cli -- [command]

mod agent;
mod api;
mod cli;
mod config;
mod memory;
mod models;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = cli::Cli::parse();
    
    // Run the command
    cli.run().await
}
