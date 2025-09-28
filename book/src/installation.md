# Installation

This guide will help you install Quelle on your system. Currently, Quelle must be built from source since it's still in early development.

## System Requirements

- **Operating System**: Windows, macOS, or Linux
- **Rust**: Latest stable version (1.70+)
- **Git**: For cloning the repository
- **Disk Space**: ~500MB for source code and build artifacts

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
# Install just (command runner)
cargo install just

# Install cargo-component (for building WASM extensions)
cargo install cargo-component

# Add WASM target
rustup target add wasm32-unknown-unknown
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

```bash
# Build the CLI tool
cargo build --release -p quelle_cli

# This creates the binary at target/release/quelle_cli
# Build time: 2-5 minutes depending on your system
```

### 3. Build Sample Extensions

```bash
# Build DragonTea extension
just build-extension dragontea

# Build ScribbleHub extension  
just build-extension scribblehub

# Verify extensions were built
ls target/wasm32-unknown-unknown/release/extension_*.wasm
```

### 4. Verify Installation

```bash
# Test the CLI
./target/release/quelle_cli --help

# Check version info
./target/release/quelle_cli --version
```

You should see the help output with available commands.

## Optional: Add to PATH

To use `quelle` from anywhere:

### Linux/macOS

```bash
# Copy to a directory in your PATH
sudo cp target/release/quelle_cli /usr/local/bin/quelle

# Or create a symlink
sudo ln -s $(pwd)/target/release/quelle_cli /usr/local/bin/quelle

# Test it works
quelle --help
```

### Windows

```powershell
# Copy to a directory in your PATH, or add the target/release directory to PATH
# Example: copy to C:\Users\<username>\bin\ (if that's in your PATH)
copy target\release\quelle_cli.exe C:\Users\%USERNAME%\bin\quelle.exe
```

## Troubleshooting

### Rust Not Found

```bash
# Make sure Rust is in your PATH
echo $PATH | grep cargo

# If not found, add to your shell profile (~/.bashrc, ~/.zshrc, etc.)
export PATH="$HOME/.cargo/bin:$PATH"
```

### Build Fails

```bash
# Update Rust to latest stable
rustup update

# Clean and rebuild
cargo clean
cargo build --release -p quelle_cli
```

### Extension Build Fails

```bash
# Make sure you have the WASM target
rustup target add wasm32-unknown-unknown

# Make sure cargo-component is installed
cargo install cargo-component --force

# Try building again
just build-extension dragontea
```

### Permission Errors (Linux/macOS)

```bash
# If you can't write to /usr/local/bin
mkdir -p ~/bin
cp target/release/quelle_cli ~/bin/quelle

# Add ~/bin to PATH in your shell profile
echo 'export PATH="$HOME/bin:$PATH"' >> ~/.bashrc
# Then restart your terminal or run: source ~/.bashrc
```

## What's Installed

After successful installation, you'll have:

- **quelle_cli**: The main command-line tool
- **Extensions**: WASM files for DragonTea and ScribbleHub
- **Source Code**: Full project source for development

## File Locations

```text
quelle/
├── target/release/quelle_cli          # Main executable
├── target/wasm32-unknown-unknown/
│   └── release/
│       ├── extension_dragontea.wasm   # DragonTea extension
│       └── extension_scribblehub.wasm # ScribbleHub extension
└── data/                              # Created when you first run CLI
    ├── config.json                    # Store configuration
    └── registry/                      # Installed extensions
```

## Next Steps

Now that Quelle is installed:

1. **Set up your first store**: See [Getting Started](./getting-started.md)
2. **Try basic commands**: Check out [Basic Usage](./basic-usage.md)
3. **Test with a novel URL**: Try fetching from a supported site

## Updating

Since Quelle is in active development, you may want to update regularly:

```bash
# Update source code
cd quelle
git pull origin main

# Rebuild
cargo build --release -p quelle_cli

# Rebuild extensions if needed
just build-extension dragontea
just build-extension scribblehub
```

## Uninstallation

To remove Quelle:

```bash
# Remove from PATH (if you added it)
sudo rm /usr/local/bin/quelle

# Remove source directory
rm -rf quelle/

# Remove Rust tools (optional)
cargo uninstall just
cargo uninstall cargo-component
```

## Getting Help

If you run into issues:

1. Check this troubleshooting section
2. Make sure you have the latest Rust stable
3. Verify all prerequisites are installed
4. Check the project's GitHub issues for known problems

The installation process should take 5-10 minutes on most systems. If it takes much longer, something may be wrong with your setup.