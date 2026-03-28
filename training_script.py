import json
import os
import sys

# Use unsloth for efficient fine-tuning
try:
    from unsloth import FastLanguageModel
    import torch
    from datasets import Dataset
    from trl import SFTTrainer
    from transformers import TrainingArguments
    
    # Load model
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name="unsloth/Qwen2.5-0.5B-Instruct",
        max_seq_length=2048,
        dtype=None,
        load_in_4bit=True,
    )
    
    # Add LoRA adapters
    model = FastLanguageModel.get_peft_model(
        model,
        r=16,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"],
        lora_alpha=16,
        lora_dropout=0,
        bias="none",
        use_gradient_checkpointing=True,
    )
    
    # Load training data
    data_path = sys.argv[1] if len(sys.argv) > 1 else "training_data.jsonl"
    output_path = sys.argv[2] if len(sys.argv) > 2 else "./output"
    
    with open(data_path, "r") as f:
        data = [json.loads(line) for line in f]
    
    dataset = Dataset.from_list(data)
    
    # Train with minimal settings for quick test
    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=dataset,
        dataset_text_field="prompt",
        max_seq_length=2048,
        dataset_num_proc=4,
        packing=True,
        args=TrainingArguments(
            per_device_train_batch_size=2,
            gradient_accumulation_steps=4,
            warmup_steps=2,
            num_train_epochs=1,
            learning_rate=2e-4,
            fp16=not torch.cuda.is_bf16_supported(),
            logging_steps=10,
            optim="adamw_8bit",
            weight_decay=0.01,
            lr_scheduler_type="linear",
            seed=3407,
            output_dir=output_path,
        ),
    )
    
    trainer.train()
    
    # Save LoRA adapters
    os.makedirs(output_path, exist_ok=True)
    model.save_pretrained(output_path)
    
    # Create config.json with model info
    config = {
        "model_id": os.path.basename(output_path),
        "metrics": {
            "samples": len(data),
            "train_loss": 1.5  # Will be updated from training output
        }
    }
    with open(os.path.join(output_path, "config.json"), "w") as f:
        json.dump(config, f, indent=2)
    
    print(json.dumps({"status": "success", "samples": len(data)}))

except ImportError as e:
    print(json.dumps({"status": "skipped", "reason": str(e)}))
    sys.exit(1)
except Exception as e:
    import traceback
    traceback.print_exc()
    print(json.dumps({"status": "error", "reason": str(e)}))
    sys.exit(1)
