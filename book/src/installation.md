# Installation

This guide will help you install Quelle on your system. As an MVP-ready project, Quelle currently requires building from source, with pre-built binaries coming in future releases.

## System Requirements

- **Operating System**: Windows, macOS, or Linux
- **Rust**: Latest stable version (1.70+)
- **Git**: For cloning the repository
- **Disk Space**: ~1GB for source code and build artifacts

## Prerequisites

### Install Rust

If you don't have Rust installed:

```bash
# Install Rust (follow prompts to complete installation)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Reload your shell or restart terminal
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

### Install Build Tools

```bash
# Add WebAssembly target for building extensions
rustup target add wasm32-unknown-unknown

# Install cargo-component for WebAssembly components
cargo install cargo-component

# Optional: Install just for convenient shortcuts
cargo install just
```

## Installation Steps

### 1. Clone Repository

```bash
# Clone the project
git clone https://github.com/nacht-org/quelle
cd quelle

# Verify you're in the right directory
ls -la
# You should see Cargo.toml, justfile, and other project files
```

### 2. Build Quelle

Choose your build configuration based on desired features:

```bash
# Build with default features (EPUB + PDF export)
cargo build --release

# Build with EPUB export only
cargo build --release --no-default-features --features git

# Build with specific features
cargo build --release --features git,pdf
```

This creates the binary at `target/release/quelle`.

### 3. Set Up Extension System

Set up the local extension store and build sample extensions:

```bash
# Set up local extension store
./target/release/quelle store add local local ./data/stores/local

# Build and publish ScribbleHub extension
cargo component build -r -p extension_scribblehub --target wasm32-unknown-unknown
./target/release/quelle publish extension \
  ./target/wasm32-unknown-unknown/release/extension_scribblehub.wasm \
  --store local --overwrite

# Install the extension
./target/release/quelle extensions install en.scribblehub
```

### 4. Verify Installation

```bash
# Test the CLI
./target/release/quelle --help

# Check system status
./target/release/quelle status

# List available extensions
./target/release/quelle extensions list
```

You should see the help output and system status confirming everything is working.

## Optional: Add to PATH

To use `quelle` from anywhere:

### Linux/macOS

```bash
# Copy to a directory in your PATH
sudo cp target/release/quelle /usr/local/bin/quelle

# Or create a symlink
sudo ln -s $(pwd)/target/release/quelle /usr/local/bin/quelle

# Test it works
quelle --help
```

### Windows

```powershell
# Copy to a directory in your PATH
copy target\release\quelle.exe C:\Users\%USERNAME%\bin\quelle.exe

# Or add the target/release directory to your PATH environment variable
```

## Quick Setup with Just (Optional)

If you installed `just`, you can use convenient shortcuts:

```bash
# Complete setup in one command
just setup

# This runs:
# - Sets up local store
# - Builds and publishes ScribbleHub extension
# - Makes it ready for use
```

## Building Additional Extensions

Build other available extensions:

```bash
# Build DragonTea extension
cargo component build -r -p extension_dragontea --target wasm32-unknown-unknown
./target/release/quelle publish extension \
  ./target/wasm32-unknown-unknown/release/extension_dragontea.wasm \
  --store local --overwrite
./target/release/quelle extensions install en.dragontea

# Build RoyalRoad extension
cargo component build -r -p extension_royalroad --target wasm32-unknown-unknown
./target/release/quelle publish extension \
  ./target/wasm32-unknown-unknown/release/extension_royalroad.wasm \
  --store local --overwrite
./target/release/quelle extensions install en.royalroad
```

Or use just shortcuts:

```bash
just build scribblehub
just publish scribblehub
just build dragontea
just publish dragontea
just build royalroad
just publish royalroad
```

## Troubleshooting

### Rust Not Found

```bash
# Make sure Rust is in your PATH
echo $PATH | grep cargo

# If not found, add to your shell profile (~/.bashrc, ~/.zshrc, etc.)
export PATH="$HOME/.cargo/bin:$PATH"
source ~/.bashrc  # or restart terminal
```

### Build Fails

```bash
# Update Rust to latest stable
rustup update

# Clean and rebuild
cargo clean
cargo build --release
```

### Extension Build Fails

```bash
# Ensure you have the WASM target
rustup target add wasm32-unknown-unknown

# Reinstall cargo-component
cargo install cargo-component --force

# Try building specific extension
cargo component build -r -p extension_scribblehub --target wasm32-unknown-unknown
```

### Extension Store Issues

```bash
# Check store configuration
quelle store list

# Recreate local store if needed
./target/release/quelle store remove local --force
./target/release/quelle store add local local ./data/stores/local
```

### Permission Errors (Linux/macOS)

```bash
# Copy to a directory in your PATH
mkdir -p ~/bin
cp target/release/quelle ~/bin/quelle

# Add ~/bin to PATH in your shell profile
echo 'export PATH="$HOME/bin:$PATH"' >> ~/.bashrc
# Then restart your terminal or run: source ~/.bashrc
```

## What's Installed

After successful installation, you'll have:

- **quelle**: The main command-line executable
- **Extensions**: WASM files for supported novel sources
- **Local Store**: Extension management system
- **Source Code**: Full project source for development

## File Locations

```text
quelle/
├── target/release/quelle                  # Main executable
├── target/wasm32-unknown-unknown/release/ # Built extensions
│   ├── extension_scribblehub.wasm
│   ├── extension_dragontea.wasm
│   └── extension_royalroad.wasm
└── data/stores/local/                     # Local extension store
    └── extensions/                        # Installed extensions
```

## Configuration

Quelle stores its data in standard locations:

- **Linux**: `~/.local/share/quelle/`
- **macOS**: `~/Library/Application Support/quelle/`
- **Windows**: `%APPDATA%\quelle\`

You can override this with the `--storage-path` option or `QUELLE_DATA_DIR` environment variable.

## Testing Your Installation

Try these commands to verify everything works:

```bash
# Check system health
./target/release/quelle status

# Search for novels (requires internet)
./target/release/quelle search "fantasy adventure" --limit 3

# View available extensions
./target/release/quelle extensions list

# Get help on any command
./target/release/quelle add --help
```

## Next Steps

Now that Quelle is installed:

1. **First Use**: See [Getting Started](./getting-started.md) to add your first novel
2. **Learn the CLI**: Check [Basic Usage](./basic-usage.md) for comprehensive workflows
3. **Explore Extensions**: Learn about extension management and development

## Updating Quelle

Since Quelle is actively developed, update regularly:

```bash
# Update source code
cd quelle
git pull origin main

# Rebuild
cargo build --release

# Rebuild extensions if needed
just build scribblehub
just publish scribblehub
# Repeat for other extensions
```

## Uninstallation

To remove Quelle:

```bash
# Remove from PATH (if you added it)
sudo rm /usr/local/bin/quelle

# Remove source directory
rm -rf quelle/

# Remove data directory (optional)
rm -rf ~/.local/share/quelle/  # Linux
rm -rf ~/Library/Application\ Support/quelle/  # macOS
rm -rf %APPDATA%\quelle\  # Windows

# Remove Rust tools (optional)
cargo uninstall just
cargo uninstall cargo-component
```

## Getting Help

If you encounter issues:

1. Check this troubleshooting section first
2. Verify you have the latest Rust stable version
3. Ensure all prerequisites are installed correctly
4. Check the project's [GitHub issues](https://github.com/nacht-org/quelle/issues) for known problems
5. Use `quelle status` to diagnose system health

The installation process should take 5-15 minutes on most systems, depending on your internet connection and system performance.

## Development Features

As a bonus for building from source, you get access to all development features:

- **Extension Generator**: `quelle dev generate` to create new extensions
- **Development Server**: `quelle dev server <name> --watch` for hot reload testing
- **Extension Validation**: `quelle dev validate <name>` before publishing
- **Testing Tools**: `quelle dev test <name>` for interactive testing

See [Extension Development](./development/extension-development.md) to learn more about creating your own scrapers.
