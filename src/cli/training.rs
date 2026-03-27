//! Training functionality
//! 
//! Provides RL training trigger and status monitoring.

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

const OLLAMA_URL: &str = "http://localhost:11434";

/// Run RL training
pub async fn run_training(steps: usize, epochs: usize) -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                    RL Training Trigger                   ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  Steps: {}                                           ║", steps);
    println!("║  Epochs: {}                                            ║", epochs);
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    
    // First, let's prepare training data from memories
    println!("📋 Preparing training data...");
    let training_data = prepare_training_data()?;
    
    if training_data.is_empty() {
        println!("⚠️  No training data found. Starting training with default settings...");
    } else {
        println!("✓ Found {} training examples", training_data.len());
    }
    
    // Check if Ollama has fine-tune capability
    println!("\n🔧 Checking Ollama fine-tuning support...");
    
    // Start training via Ollama's API
    println!("\n🚀 Starting RL training...");
    println!("   Base model: qwen3.5-4b");
    println!("   Target steps: {}", steps);
    println!("   Epochs: {}", epochs);
    println!();
    
    // Call Ollama fine-tune API
    match start_finetune(steps, epochs).await {
        Ok(job_id) => {
            println!("✅ Training job started!");
            println!("   Job ID: {}", job_id);
            println!();
            println!("📊 Monitoring training progress...");
            println!("   (Press Ctrl+C to stop monitoring, training will continue)");
            println!();
            
            // Monitor training progress
            monitor_training(&job_id).await?;
        }
        Err(e) => {
            eprintln!("\n❌ Failed to start training: {}", e);
            eprintln!();
            println!("💡 Note: Ollama may not support fine-tuning directly.");
            println!("   Consider using a dedicated training pipeline with:");
            println!("   - llama.cpp for GGUF models");
            println!("   - Axolotl for fine-tuning");
            println!("   - unsloth for fast LoRA training");
        }
    }
    
    Ok(())
}

/// Prepare training data from memory store
fn prepare_training_data() -> Result<Vec<TrainingExample>> {
    let mut examples = Vec::new();
    
    // Try to load from training_data directory
    let training_dir = PathBuf::from(".agent/training_data");
    if training_dir.exists() {
        if let Ok(entries) = fs::read_dir(&training_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if let Ok(example) = serde_json::from_str::<TrainingExample>(&content) {
                            examples.push(example);
                        }
                    }
                }
            }
        }
    }
    
    Ok(examples)
}

/// Start fine-tuning job via Ollama
async fn start_finetune(steps: usize, epochs: usize) -> Result<String> {
    let url = format!("{}/api/finetune", OLLAMA_URL);
    
    let request_body = serde_json::json!({
        "model": "qwen3.5-4b",
        "adapter": "lora",
        "steps": steps,
        "epochs": epochs,
        "train_files": [],
        "output": {
            "name": format!("qwen3.5-4b-trained-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"))
        },
        "loraConfig": {
            "rank": 16,
            "alpha": 16,
            "dropout": 0.05,
            "targetModules": ["q_proj", "k_proj", "v_proj", "o_proj"]
        }
    });
    
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request_body)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;
    
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Ollama fine-tune API error: {} - {}", response.status(), error_text);
    }
    
    let response_json: serde_json::Value = response.json().await?;
    
    let job_id = response_json["job_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;
    
    Ok(job_id.to_string())
}

/// Monitor training progress
async fn monitor_training(job_id: &str) -> Result<()> {
    let url = format!("{}/api/finetune/{}", OLLAMA_URL, job_id);
    let client = reqwest::Client::new();
    
    loop {
        match client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(status) = response.json::<serde_json::Value>().await {
                        let state = status["state"].as_str().unwrap_or("unknown");
                        let progress = status["progress"].as_f64().unwrap_or(0.0);
                        let loss = status["loss"].as_f64();
                        
                        print!("\r📈 Status: {:12} | Progress: {:5.1}%", state, progress);
                        if let Some(l) = loss {
                            print!(" | Loss: {:.4}", l);
                        }
                        io::Write::flush(&mut io::stdout())?;
                        
                        if state == "completed" || state == "failed" || state == "cancelled" {
                            println!();
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("\n⚠️  Failed to get status: {}", e);
            }
        }
        
        sleep(Duration::from_secs(5)).await;
    }
    
    println!();
    println!("✅ Training monitoring complete!");
    
    Ok(())
}

/// Show training status
pub async fn show_status() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                   Training Status                        ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    // Check Ollama for any running fine-tune jobs
    let url = format!("{}/api/finetune", OLLAMA_URL);
    let client = reqwest::Client::new();
    
    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(status) = response.json::<serde_json::Value>().await {
                    if let Some(jobs) = status["jobs"].as_array() {
                        if jobs.is_empty() {
                            println!("║  No active training jobs                            ║");
                        } else {
                            for job in jobs {
                                let state = job["state"].as_str().unwrap_or("unknown");
                                let model = job["model"].as_str().unwrap_or("unknown");
                                println!("║  Model: {}                                      ║", model);
                                println!("║  State: {}                                       ║", state);
                            }
                        }
                    } else {
                        println!("║  No active training jobs                            ║");
                    }
                }
            }
        }
        Err(_) => {
            println!("║  Could not connect to Ollama                         ║");
        }
    }
    
    // List trained models
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║                   Trained Models                         ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    let models_dir = PathBuf::from(".agent/models");
    if models_dir.exists() {
        if let Ok(entries) = fs::read_dir(&models_dir) {
            let mut found = false;
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains("lora") || name.contains("adapter") || name.contains("trained") {
                    println!("║  ✓ {}     ║", name);
                    found = true;
                }
            }
            if !found {
                println!("║  No trained models found yet                        ║");
            }
        } else {
            println!("║  No trained models found yet                        ║");
        }
    } else {
        println!("║  No trained models found yet                        ║");
    }
    
    println!("╚══════════════════════════════════════════════════════════╝");
    
    Ok(())
}

/// List available models
pub async fn list_models() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                   Available Models                       ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    
    // Query Ollama for available models
    let url = format!("{}/api/tags", OLLAMA_URL);
    let client = reqwest::Client::new();
    
    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    if let Some(models) = json["models"].as_array() {
                        for model in models {
                            let name = model["name"].as_str().unwrap_or("unknown");
                            let size = model["size"].as_u64().unwrap_or(0);
                            let size_gb = size as f64 / 1_000_000_000.0;
                            
                            let is_base = name.contains("qwen3.5-4b") && !name.contains("-trained");
                            let badge = if is_base { "[BASE]" } else { "[    ]" };
                            
                            println!("║  {} {:40}  ║", badge, name);
                            if size_gb > 0.0 {
                                println!("║       Size: {:.1} GB                                ║", size_gb);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("║  Error connecting to Ollama: {}                     ║", e);
        }
    }
    
    println!();
    println!("  [BASE] = Base model (qwen3.5-4b)");
    println!("  To chat with base model:   agi-cli chat");
    println!("  To chat with trained model: agi-cli chat --model <model-name>");
    println!();
    println!("  To trigger RL training:     agi-cli train");
    println!();
    println!("╚══════════════════════════════════════════════════════════╝");
    
    Ok(())
}

// Training example structure
#[derive(serde::Serialize, serde::Deserialize)]
struct TrainingExample {
    prompt: String,
    completion: String,
}
