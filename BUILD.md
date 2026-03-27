# Building AGI Agent

## Prerequisites

- Rust 1.94+ (with 2024 edition support)
- OpenSSL development headers (libssl-dev on Ubuntu/Debian)
- SQLite development headers (libsqlite3-dev)

## Quick Build

```bash
# Use the build script (handles path length workaround)
./build.sh

# Or manually set CARGO_TARGET_DIR to a shorter path
export CARGO_TARGET_DIR=/tmp/target
cargo build --release
```

## Run

```bash
# After building
/tmp/target/release/agi-agent

# Or create a symlink
ln -sf /tmp/target/release/agi-agent ./agent
./agent
```

## Development

```bash
# Watch for changes and rebuild
cargo watch -x build

# Run tests
cargo test

# Format code
cargo fmt --check
```

## Build Output

The binary is placed at: `/tmp/target/release/agi-agent`

## Troubleshooting

### Path too long errors

If you see "Invalid argument (os error 22)", set `CARGO_TARGET_DIR` to a shorter path:

```bash
export CARGO_TARGET_DIR=/tmp/target
cargo build --release
```

This is required because Rust's build system creates deep directory structures that can exceed kernel path length limits.
