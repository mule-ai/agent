# Building AGI Agent

## Prerequisites

- Rust 1.94+ (with 2024 edition support)
- OpenSSL development headers (`libssl-dev` on Ubuntu/Debian)
- SQLite development headers (`libsqlite3-dev`)

## Quick Build

```bash
# Build the agent
cargo build --release

# Binary is at:
./target/release/agent
```

## Run

```bash
# Direct
./target/release/agent

# Or via systemd
sudo systemctl start agi-agent
```

## Development

```bash
# Build
cargo build --release

# Run tests
cargo test

# Format code
cargo fmt --check
```

## Systemd Service

Create `/etc/systemd/system/agi-agent.service`:

```ini
[Unit]
Description=AGI Agent
After=network.target llama-qwen.service

[Service]
Type=simple
User=administrator
WorkingDirectory=/data/jbutler/mule/agent
ExecStart=/data/jbutler/mule/agent/target/release/agent
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Then:
```bash
sudo systemctl daemon-reload
sudo systemctl enable agi-agent
sudo systemctl start agi-agent
```

## All Services

| Service | Binary | Port |
|---------|--------|------|
| agi-agent | `./target/release/agent` | 8080 |
| llama-qwen | llama-*.sh | 8081 |
| llama-embedding | embedding.sh | 8083 |
| llama-rerank | rerank.sh | 8084 |

## Troubleshooting

### Build fails with path errors

Set `CARGO_TARGET_DIR` to a shorter path:
```bash
export CARGO_TARGET_DIR=/tmp/target
cargo build --release
```
