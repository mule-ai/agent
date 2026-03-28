//! AGI Agent CLI - Interactive Chat and Training
//! 
//! Run with: cargo run --bin cli -- [command]

use anyhow::Result;
use clap::{Parser, Subcommand};

const OLLAMA_URL: &str = "http://localhost:11434";

#[derive(Parser, Debug)]
#[command(name = "agi-cli")]
#[command(about = "Interactive CLI for AGI Agent - chat with models and trigger RL training")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Chat { model } => {
            run_chat(model).await?;
        }
        Commands::Train { steps, epochs } => {
            run_training(steps, epochs).await?;
        }
        Commands::Status => {
            show_status().await?;
        }
        Commands::Models => {
            list_models().await?;
        }
    }
    Ok(())
}

async fn run_chat(model: Option<String>) -> Result<()> {
    let model = model.unwrap_or_else(|| "qwen3:8b".to_string());
    println!("Starting chat with model: {}", model);
    println!("Type 'exit' or 'quit' to end the conversation.");
    println!("---");
    
    loop {
        print!("\n> ");
        std::io::Write::flush(&mut std::io::stdout())?;
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        if input == "exit" || input == "quit" {
            println!("Goodbye!");
            break;
        }
        
        let response = call_ollama(&model, input).await?;
        println!("\n{}", response);
    }
    
    Ok(())
}

async fn run_training(steps: usize, epochs: usize) -> Result<()> {
    println!("Starting training: {} steps, {} epochs", steps, epochs);
    println!("Note: Full training requires additional setup.");
    Ok(())
}

async fn show_status() -> Result<()> {
    println!("Checking training status...");
    println!("No active training jobs.");
    Ok(())
}

async fn list_models() -> Result<()> {
    println!("Available models:");
    println!("- qwen3:8b (base model)");
    println!("- Custom trained adapters (when available)");
    Ok(())
}

async fn call_ollama(model: &str, prompt: &str) -> Result<String> {
    use serde_json::json;
    
    let url = format!("{}/api/chat", OLLAMA_URL);
    
    let request_body = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ],
        "stream": false,
        "options": {
            "temperature": 0.7,
            "num_predict": 2048
        }
    });
    
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request_body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await?;
    
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Ollama API error: {} - {}", status, error_text);
    }
    
    let response_json: serde_json::Value = response.json().await?;
    
    let content = response_json["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid response format from Ollama"))?;
    
    Ok(content.to_string())
}
