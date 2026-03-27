//! CLI module for interactive agent interaction
//! 
//! Provides an interactive shell for chatting with models and triggering RL training.

pub mod chat;
pub mod training;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// AGI Agent CLI
#[derive(Parser, Debug)]
#[command(name = "agi-cli")]
#[command(about = "Interactive CLI for AGI Agent - chat with models and trigger RL training")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start an interactive chat session with a model
    Chat {
        /// Model name to chat with (defaults to base model)
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Trigger RL training
    Train {
        /// Number of training steps
        #[arg(short, long, default_value = "500")]
        steps: usize,
        /// Number of epochs
        #[arg(short, long, default_value = "3")]
        epochs: usize,
    },
    /// Check training status
    Status,
    /// List available models (base + trained)
    Models,
    /// Chat with a trained LoRA adapter
    ChatWithAdapter {
        /// Name of the trained adapter
        adapter: String,
    },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Commands::Chat { model } => {
                chat::run_chat(model).await?;
            }
            Commands::Train { steps, epochs } => {
                training::run_training(steps, epochs).await?;
            }
            Commands::Status => {
                training::show_status().await?;
            }
            Commands::Models => {
                training::list_models().await?;
            }
            Commands::ChatWithAdapter { adapter } => {
                chat::run_chat_with_adapter(adapter).await?;
            }
        }
        Ok(())
    }
}
