//! Training functionality
//! 
//! Provides RL training trigger and status monitoring via the agent REST API.

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const AGENT_URL: &str = "http://localhost:8080";

#[derive(Deserialize)]
struct BatchStats {
    example_count: usize,
    is_ready: bool,
}

#[derive(Deserialize)]
struct BatchStatus {
    status: String,
    examples_collected: usize,
    models_trained: usize,
    last_training: Option<String>,
}

#[derive(Deserialize)]
struct MemoryStats {
    total: usize,
    #[serde(rename = "by_namespace")]
    by_namespace: Vec<NamespaceCount>,
}

#[derive(Deserialize)]
struct NamespaceCount {
    namespace: String,
    count: usize,
}

#[derive(Deserialize)]
struct TrainingStatus {
    status: String,
    current_job: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct MemoryEntry {
    content: String,
    namespace: String,
    memory_type: String,
}

/// Run training via the agent REST API
pub async fn run_training(steps: usize, epochs: usize) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    RL Training                         ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    // Check batch training stats
    let stats_url = format!("{}/training/batch/stats", AGENT_URL);
    match client.get(&stats_url).send().await {
        Ok(response) => {
            if let Ok(stats) = response.json::<BatchStats>().await {
                println!("║  Training examples: {:>3} / 50                         ║", stats.example_count);
                println!("║  Ready for training: {:<5}                            ║", 
                    if stats.is_ready { "Yes" } else { "No" });
            }
        }
        Err(e) => {
            println!("║  Warning: Could not fetch stats: {}             ║", e);
        }
    }
    
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // Trigger training
    println!("🚀 Triggering batch training...");
    
    let trigger_url = format!("{}/training/batch/run", AGENT_URL);
    let body = serde_json::json!({ "force": true });
    
    match client.post(&trigger_url)
        .json(&body)
        .timeout(Duration::from_secs(60))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(result) = response.json::<serde_json::Value>().await {
                    if result["success"] == true {
                        println!("✅ Training started successfully!");
                        if let Some(job_id) = result["job_id"].as_str() {
                            println!("   Job ID: {}", job_id);
                        }
                    } else {
                        println!("⚠️  Training not started: {}", result["message"]);
                    }
                }
            } else {
                println!("❌ Training request failed: HTTP {}", response.status());
                let error_text = response.text().await.unwrap_or_default();
                if !error_text.is_empty() {
                    println!("   Details: {}", error_text);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to connect to agent: {}", e);
            println!();
            println!("💡 Make sure the agent is running: ./agent");
        }
    }
    
    println!();
    println!("📊 Check status with: ./agi status");
    
    Ok(())
}

/// Show comprehensive training status
pub async fn show_status() -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                       Agent Status                      ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    // Check agent health
    let health_url = format!("{}/health", AGENT_URL);
    match client.get(&health_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(health) = response.json::<serde_json::Value>().await {
                    let version = health["version"].as_str().unwrap_or("unknown");
                    let status = health["status"].as_str().unwrap_or("unknown");
                    println!("  Service: AGI Agent v{} (Status: {})", version, status);
                }
            }
        }
        Err(e) => {
            println!("  ⚠️  Agent not responding: {}", e);
            println!();
            println!("  Start the agent with: ./agent");
            println!("╚══════════════════════════════════════════════════════════╝");
            return Ok(());
        }
    }
    
    // Check memories
    println!();
    println!("  Memory:");
    let memory_url = format!("{}/memories/stats", AGENT_URL);
    if let Ok(response) = client.get(&memory_url).send().await {
        if let Ok(stats) = response.json::<MemoryStats>().await {
            let mut retrieval = 0;
            let mut training = 0;
            for ns in stats.by_namespace {
                match ns.namespace.as_str() {
                    "retrieval" => retrieval = ns.count,
                    "training" => training = ns.count,
                    _ => {}
                }
            }
            println!("    Total memories: {}", stats.total);
            println!("    Retrieval: {}", retrieval);
            println!("    Training: {}", training);
        }
    }
    
    // Check batch training
    println!();
    println!("  Batch Training:");
    let batch_url = format!("{}/training/batch/stats", AGENT_URL);
    if let Ok(response) = client.get(&batch_url).send().await {
        if let Ok(stats) = response.json::<BatchStats>().await {
            println!("    Examples: {} (need 50 for training)", stats.example_count);
            println!("    Ready: {}", if stats.is_ready { "Yes ✅" } else { "No ❌" });
        }
    }
    
    let batch_status_url = format!("{}/training/batch/status", AGENT_URL);
    if let Ok(response) = client.get(&batch_status_url).send().await {
        if let Ok(status) = response.json::<BatchStatus>().await {
            println!();
            println!("  Training Status: {}", status.status);
            println!("    Models trained: {}", status.models_trained);
            if let Some(last) = status.last_training {
                println!("    Last training: {}", last);
            }
        }
    }
    
    // Check scheduler
    println!();
    println!("  Scheduler:");
    let scheduler_url = format!("{}/scheduler/stats", AGENT_URL);
    if let Ok(response) = client.get(&scheduler_url).send().await {
        if let Ok(stats) = response.json::<serde_json::Value>().await {
            let batch_runs = stats["stats"]["batch_training_runs"].as_u64().unwrap_or(0);
            println!("    Batch training runs: {}", batch_runs);
        }
    }
    
    // List some training memories
    println!();
    println!("  Recent Training Memories:");
    let training_url = format!("{}/memories?namespace=training&limit=5", AGENT_URL);
    if let Ok(response) = client.get(&training_url).send().await {
        if let Ok(data) = response.json::<serde_json::Value>().await {
            if let Some(memories) = data["memories"].as_array() {
                if memories.is_empty() {
                    println!("    (none)");
                } else {
                    for mem in memories {
                        let content = mem["content"].as_str().unwrap_or("");
                        let short = if content.len() > 50 { &content[..50] } else { content };
                        println!("    • {}", short.replace('\n', " "));
                    }
                }
            }
        }
    }
    
    println!("╚══════════════════════════════════════════════════════════╝");
    
    Ok(())
}

/// List available models
pub async fn list_models() -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                   Available Models                      ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    // Query agent for models
    let models_url = format!("{}/v1/models", AGENT_URL);
    match client.get(&models_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(models) = data["data"].as_array() {
                        for model in models {
                            let name = model["id"].as_str().unwrap_or("unknown");
                            println!("║  {:<50}  ║", name);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("║  Error: {}                                    ║", e);
        }
    }
    
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║                   Commands                           ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  ./agi chat              - Chat with base model       ║");
    println!("║  ./agi train             - Trigger training            ║");
    println!("║  ./agi status            - Check status                ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    
    Ok(())
}
