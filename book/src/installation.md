# Installation

> **⚠️ Note**: Quelle is currently in pre-MVP development. The installation process will become simpler as the project matures.

## System Requirements

- **Operating System**: Linux, macOS, or Windows
- **Rust**: Latest stable version (1.70+)
- **Memory**: 512MB RAM minimum, 2GB recommended
- **Storage**: 100MB for base installation, additional space for extensions

## Development Installation (Current)

Since Quelle is in active development, installation currently requires building from source:

### 1. Install Prerequisites

#### Rust Toolchain
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Add WebAssembly target
rustup target add wasm32-unknown-unknown
```

#### Required Tools
```bash
# Install just (task runner)
cargo install just

# Install cargo-component (for WASM components)
cargo install cargo-component
```

### 2. Clone and Build

```bash
# Clone the repository
git clone https://github.com/nacht-org/quelle.git
cd quelle

# Build the CLI
cargo build --release -p quelle_cli

# Verify installation
./target/release/quelle_cli --help
```

### 3. Optional: Install to PATH

```bash
# Copy to local binary directory
cp target/release/quelle_cli ~/.local/bin/quelle

# Or create a symbolic link
ln -s $(pwd)/target/release/quelle_cli ~/.local/bin/quelle

# Verify it's in your PATH
quelle --help
```

## Future Installation Methods

The following installation methods are planned for future releases:

### Pre-compiled Binaries (Planned)
```bash
# Download and install (future)
curl -sSL https://get.quelle.org | sh
```

### Package Managers (Planned)

#### Cargo (Rust Package Manager)
```bash
# Install from crates.io (future)
cargo install quelle
```

#### Homebrew (macOS/Linux)
```bash
# Install via Homebrew (future)
brew install quelle
```

#### APT (Debian/Ubuntu)
```bash
# Install via APT (future)
sudo apt update
sudo apt install quelle
```

#### Scoop (Windows)
```bash
# Install via Scoop (future)
scoop install quelle
```

## Directory Structure

After installation, Quelle uses the following directories:

### Default Locations

- **Extensions**: `~/.local/share/quelle/extensions/`
- **Cache**: `~/.cache/quelle/`
- **Configuration**: `~/.config/quelle/`
- **Logs**: `~/.local/share/quelle/logs/`

### Custom Locations

You can override default locations using environment variables:

```bash
export QUELLE_INSTALL_DIR="/custom/extensions/path"
export QUELLE_CACHE_DIR="/custom/cache/path"
export QUELLE_CONFIG_DIR="/custom/config/path"
```

## Verification

After installation, verify everything is working:

```bash
# Check version and help
quelle --help

# Check store system
quelle store list

# Check extension system
quelle extension list

# Test with a simple command
quelle store health
```

## Building Extensions

To work with extensions, you'll also need to build them:

```bash
# Build sample extensions
just build-extension dragontea
just build-extension scribblehub

# Verify extensions were built
ls target/wasm32-unknown-unknown/release/extension_*.wasm
```

## Troubleshooting

### Common Issues

#### Rust Not Found
```bash
# Ensure Rust is in your PATH
source ~/.cargo/env

# Or restart your terminal
```

#### WASM Target Missing
```bash
# Add the WebAssembly target
rustup target add wasm32-unknown-unknown
```

#### Build Failures
```bash
# Clean and rebuild
cargo clean
cargo build --release -p quelle_cli
```

#### Permission Errors
```bash
# Ensure directories exist and are writable
mkdir -p ~/.local/share/quelle
mkdir -p ~/.cache/quelle
mkdir -p ~/.config/quelle
```

### Getting Help

If you encounter issues during installation:

1. Check the [Troubleshooting Guide](./advanced/troubleshooting.md)
2. Review the [GitHub Issues](https://github.com/nacht-org/quelle/issues)
3. Join the community discussions
4. File a bug report if needed

## Updating

Currently, updates require rebuilding from source:

```bash
# Pull latest changes
cd quelle
git pull

# Rebuild
cargo build --release -p quelle_cli

# Replace binary if installed to PATH
cp target/release/quelle_cli ~/.local/bin/quelle
```

In the future, updates will be available through standard package managers and automated update mechanisms.

## Uninstallation

To remove Quelle:

```bash
# Remove binary
rm ~/.local/bin/quelle

# Remove data directories (optional)
rm -rf ~/.local/share/quelle
rm -rf ~/.cache/quelle
rm -rf ~/.config/quelle

# Remove source directory
rm -rf /path/to/quelle
```

Note that removing data directories will delete all installed extensions and configuration.