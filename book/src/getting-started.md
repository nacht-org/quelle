# Getting Started

> **⚠️ Note**: Quelle is currently in pre-MVP development. Some features may be incomplete or change significantly before the 1.0 release.

## Prerequisites

- Rust toolchain (latest stable)
- Git for cloning the repository
- Basic familiarity with command-line tools

## Installation

Currently, Quelle must be built from source:

```bash
# Clone the repository
git clone https://github.com/nacht-org/quelle
cd quelle

# Install required tools
cargo install just
cargo install cargo-component
rustup target add wasm32-unknown-unknown

# Build the CLI
cargo build --release -p quelle_cli

# The binary will be at target/release/quelle_cli
```

## First Steps

### 1. Verify Installation

Check that Quelle is working:

```bash
./target/release/quelle_cli --help
```

You should see the main command help with store and extension management options.

### 2. Check Current Status

Initially, you won't have any stores or extensions configured:

```bash
# Check stores
./target/release/quelle_cli store list
# Output: No stores configured.

# Check extensions
./target/release/quelle_cli extension list
# Output: No extensions installed.
```

### 3. Add Your First Store

For development, you can create a local store:

```bash
# Create a directory for extensions (this would contain actual extensions in practice)
mkdir -p ./my-extensions

# Add it as a store
./target/release/quelle_cli store add local ./my-extensions --name "dev-store"

# Verify it was added
./target/release/quelle_cli store list
```

### 4. Check Store Health

Verify your store is accessible:

```bash
./target/release/quelle_cli store health
```

## Building Extensions

Currently, you need to build extensions manually. For example, to build the DragonTea extension:

```bash
# Build an extension
just build-extension dragontea

# This creates a WASM file at:
# target/wasm32-unknown-unknown/release/extension_dragontea.wasm
```

## Current Limitations

Since Quelle is in early development:

- **Limited Extensions**: Only a few sample extensions exist
- **Local Stores Only**: Git and HTTP stores are not yet implemented  
- **Manual Setup**: Extension installation requires manual file management
- **Development Focus**: The system is primarily for developers right now

## Development Workflow

If you're developing extensions or contributing to Quelle:

1. **Set up the development environment** as shown above
2. **Build extensions** using the `just` command
3. **Test with the CLI** using direct WASM file paths
4. **Use the store system** for managing multiple extensions

## Getting Help

- Check the [CLI Reference](./store/cli-reference.md) for command details
- Review [Store Management](./store/management.md) for store configuration
- See [Extension Management](./store/extensions.md) for working with extensions
- For development, check the [Development](./development/) section

## What's Next?

Once you have the basics working:

- Explore the store system capabilities
- Try building your own extensions
- Contribute to the project development
- Follow the project for updates as it approaches MVP

Remember that Quelle is rapidly evolving, so check back frequently for updates and new features!