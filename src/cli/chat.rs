//! Interactive chat functionality
//! 
//! Provides an interactive shell for chatting with models.

use crate::config::AppConfig;
use crate::models::Message;
use anyhow::Result;
use std::io::{self, Write};

const OLLAMA_URL: &str = "http://localhost:11434";

/// Run interactive chat session
pub async fn run_chat(model_name: Option<String>) -> Result<()> {
    let config = AppConfig::load()?;
    let model = model_name.unwrap_or_else(|| config.model.name.clone());
    
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║             AGI Agent Chat - Interactive Mode             ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Model: {}                              ║", model);
    println!("║  Type 'exit' or 'quit' to end the session               ║");
    println!("║  Type 'clear' to clear the conversation                 ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    let mut messages: Vec<Message> = vec![Message::system(
        r#"You are a helpful AI assistant. Be concise and informative.
Format your responses clearly. Use code blocks for code examples."#.to_string()
    )];
    
    loop {
        print!("\n👤 You: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        match input.to_lowercase().as_str() {
            "exit" | "quit" => {
                println!("\n👋 Goodbye!");
                break;
            }
            "clear" => {
                messages = vec![Message::system(
                    r#"You are a helpful AI assistant. Be concise and informative.
Format your responses clearly. Use code blocks for code examples."#.to_string()
                )];
                println!("✓ Conversation cleared");
                continue;
            }
            _ => {}
        }
        
        messages.push(Message::user(input.to_string()));
        
        print!("\n🤖 Assistant: ");
        io::stdout().flush()?;
        
        match call_ollama(&model, &messages).await {
            Ok(response) => {
                println!("{}", response);
                messages.push(Message::assistant(response));
            }
            Err(e) => {
                eprintln!("\n❌ Error: {}", e);
                messages.pop(); // Remove failed user message
            }
        }
    }
    
    Ok(())
}

/// Chat with a specific LoRA adapter
pub async fn run_chat_with_adapter(adapter_name: String) -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║        AGI Agent Chat - LoRA Adapter Mode                ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Adapter: {}                                            ║", adapter_name);
    println!("║  Type 'exit' or 'quit' to end the session               ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    let model = "qwen3.5-4b"; // Base model for adapter
    let mut messages: Vec<Message> = vec![Message::system(
        r#"You are a helpful AI assistant. Be concise and informative.
Format your responses clearly. Use code blocks for code examples."#.to_string()
    )];
    
    loop {
        print!("\n👤 You: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        match input.to_lowercase().as_str() {
            "exit" | "quit" => {
                println!("\n👋 Goodbye!");
                break;
            }
            _ => {}
        }
        
        messages.push(Message::user(input.to_string()));
        
        print!("\n🤖 Assistant: ");
        io::stdout().flush()?;
        
        // Call with adapter parameter
        match call_ollama_with_adapter(model, &adapter_name, &messages).await {
            Ok(response) => {
                println!("{}", response);
                messages.push(Message::assistant(response));
            }
            Err(e) => {
                eprintln!("\n❌ Error: {}", e);
                messages.pop();
            }
        }
    }
    
    Ok(())
}

/// Call Ollama API for chat completion
async fn call_ollama(model: &str, messages: &[Message]) -> Result<String> {
    let url = format!("{}/api/chat", OLLAMA_URL);
    
    let ollama_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            let role = match m.role {
                crate::models::Role::System => "system",
                crate::models::Role::User => "user",
                crate::models::Role::Assistant => "assistant",
            };
            serde_json::json!({
                "role": role,
                "content": m.content
            })
        })
        .collect();
    
    let request_body = serde_json::json!({
        "model": model,
        "messages": ollama_messages,
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

/// Call Ollama with a specific LoRA adapter
async fn call_ollama_with_adapter(model: &str, adapter: &str, messages: &[Message]) -> Result<String> {
    let url = format!("{}/api/chat", OLLAMA_URL);
    
    let ollama_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            let role = match m.role {
                crate::models::Role::System => "system",
                crate::models::Role::User => "user",
                crate::models::Role::Assistant => "assistant",
            };
            serde_json::json!({
                "role": role,
                "content": m.content
            })
        })
        .collect();
    
    let request_body = serde_json::json!({
        "model": model,
        "messages": ollama_messages,
        "stream": false,
        "options": {
            "temperature": 0.7,
            "num_predict": 2048
        },
        "adapter": adapter
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
