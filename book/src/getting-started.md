# Getting Started

This guide will help you get Quelle running on your system. Since Quelle is still in early development, you'll need to build it from source.

## Prerequisites

You need these tools installed:

- **Rust** (latest stable version)
- **Git** for cloning the repository
- **just** command runner: `cargo install just`
- **cargo-component**: `cargo install cargo-component`

## Installation

### 1. Clone and Build

```bash
# Clone the repository
git clone https://github.com/nacht-org/quelle
cd quelle

# Add WASM target for building extensions
rustup target add wasm32-unknown-unknown

# Build the CLI
cargo build --release -p quelle_cli

# The binary will be at target/release/quelle_cli
```

### 2. Verify Installation

```bash
# Check that it works
./target/release/quelle_cli --help
```

You should see the main help with available commands.

## First Steps

### 1. Check Current Status

When you first run Quelle, nothing is configured:

```bash
# Check stores (should be empty)
./target/release/quelle_cli store list

# Check extensions (should be empty)  
./target/release/quelle_cli extension list

# Check overall status
./target/release/quelle_cli status
```

### 2. Build Sample Extensions

Build the included sample extensions:

```bash
# Build DragonTea extension
just build-extension dragontea

# Build ScribbleHub extension  
just build-extension scribblehub
```

This creates WASM files in `target/wasm32-unknown-unknown/release/`.

### 3. Set Up a Local Store

Create a local store to manage your extensions:

```bash
# Create a directory for your extensions
mkdir -p ./my-extensions

# Add it as a store
./target/release/quelle_cli store add local ./my-extensions --name "dev"

# Verify it was added
./target/release/quelle_cli store list
```

## Basic Usage

### Fetch Novel Information

Once you have extensions built, you can fetch novel info:

```bash
# Example with a DragonTea URL (replace with real URL)
./target/release/quelle_cli fetch novel https://dragontea.ink/novel/example

# Example with ScribbleHub URL
./target/release/quelle_cli fetch novel https://scribblehub.com/series/123456/example/
```

### Fetch Chapter Content

```bash
# Fetch a specific chapter
./target/release/quelle_cli fetch chapter https://dragontea.ink/novel/example/chapter-1
```

### Search (Basic)

```bash
# Search for novels (if extensions support it)
./target/release/quelle_cli search "novel title"

# Search with filters
./target/release/quelle_cli search "novel title" --author "author name"
```

### List Available Extensions

```bash
# See what extensions are available in your stores
./target/release/quelle_cli list

# Check store health
./target/release/quelle_cli status
```

## Current Limitations

Keep these in mind while using Quelle:

- **Manual extension setup**: You need to build extensions yourself
- **Limited extensions**: Only DragonTea and ScribbleHub work
- **No export formats**: Can't save as EPUB/PDF yet
- **Basic functionality**: Missing many planned features
- **Development tool**: Made for developers, not end users yet

## Troubleshooting

### Extension Build Fails

```bash
# Make sure you have the WASM target
rustup target add wasm32-unknown-unknown

# Make sure cargo-component is installed
cargo install cargo-component

# Try building again
just build-extension dragontea
```

### CLI Not Found

```bash
# Make sure you built the CLI
cargo build --release -p quelle_cli

# Run with full path
./target/release/quelle_cli --help
```

### Store Issues

```bash
# Check store health
./target/release/quelle_cli status

# Make sure directory exists and has permissions
ls -la ./my-extensions
```

## Next Steps

Once you have the basics working:

1. **Try the extensions**: Test fetching novels from DragonTea or ScribbleHub
2. **Explore commands**: Run `--help` on different commands to see options  
3. **Check the code**: Look at `extensions/` to understand how they work
4. **Follow development**: Watch for updates as new features are added

## Development Notes

If you want to contribute or modify Quelle:

- Extension code is in `extensions/`
- CLI code is in `crates/cli/`  
- Engine code is in `crates/engine/`
- Use `just build-extension <name>` to build extensions
- Use `cargo run -p quelle_cli -- <args>` to run without building

The project is evolving quickly, so check back often for updates!