# Quelle

[![Publish Extensions](https://github.com/nacht-org/quelle/actions/workflows/publish-extensions.yml/badge.svg)](https://github.com/nacht-org/quelle/actions/workflows/publish-extensions.yml)

Quelle is a powerful, extensible novel scraper and library manager that enables you to search, download, and manage e-books from multiple online sources. Built with a modular WebAssembly architecture, it provides high performance and cross-platform compatibility.

## 🚀 Quick Start

### Installation

```bash
# Clone and build
git clone https://github.com/nacht-org/quelle
cd quelle
cargo build --release

# Build with specific features
cargo build --release --no-default-features --features git     # EPUB export only
cargo build --release --features git,pdf                       # EPUB + PDF export (default)

# Set up extension system manually
cargo run -p quelle_cli -- store add local local ./data/stores/local
cargo component build -r -p extension_scribblehub --target wasm32-unknown-unknown
cargo run -p quelle_cli -- publish extension \
  ./target/wasm32-unknown-unknown/release/extension_scribblehub.wasm \
  --store local --overwrite
cargo run -p quelle_cli -- extensions install en.scribblehub
```

### Basic Usage

```bash
# Search for novels
quelle search "fantasy adventure" --limit 5

# Add a novel to your library (downloads all chapters)
quelle add https://www.scribblehub.com/series/123456/novel-name/

# List your library
quelle library list

# Read a chapter
quelle read "Novel Title" 1

# Update all novels with new chapters
quelle update

# Export to EPUB
quelle export "Novel Title" --format epub
```

## ✨ Features

### Core Functionality
- 🔍 **Multi-source search** - Search across different novel platforms simultaneously
- 📚 **Library management** - Organize and track your novel collection
- 📖 **Chapter reading** - Read chapters directly in your terminal
- 📤 **Multiple export formats** - Export to EPUB, PDF, and more
- 🔄 **Auto-updates** - Keep your novels updated with new chapters
- 🎯 **Flexible filtering** - Search by author, tags, categories

### Extension System
- 🧩 **WebAssembly extensions** - High-performance, sandboxed scrapers
- 🏪 **Extension stores** - Install extensions from local or remote repositories
- 🌐 **Official registry** - Pre-configured with [nacht-org/extensions](https://github.com/nacht-org/extensions)
- 🛠️ **Easy development** - Simple API for creating new source extensions
- 📦 **Package management** - Version control and dependency management

### Available Extensions
- **ScribbleHub** - Original novels and translations
- **DragonTea** - Light novels and web novels
- *Additional extensions available at [github.com/nacht-org/extensions](https://github.com/nacht-org/extensions)*

## 📋 Project Status

**Current Status**: ✅ **MVP Ready**

Quelle has reached MVP status with a fully functional CLI, working extension system, and reliable core features.

### What Works
- ✅ Complete CLI interface with all major commands
- ✅ Extension system (build, install, manage extensions)
- ✅ Store management (local and Git-based stores)
- ✅ Novel search and discovery
- ✅ Library management (add, update, remove novels)
- ✅ Chapter reading and export
- ✅ Working extensions for ScribbleHub and DragonTea

### In Development
- 🔄 Additional output formats (PDF improvements)
- 🔄 More extension sources
- 🔄 Enhanced search capabilities
- 🔄 Cross-platform binary distribution

## 📖 CLI Reference

### Core Commands

```bash
# Library Management
quelle add <url>                    # Add novel to library
quelle update [novel]               # Update novels with new chapters
quelle remove <novel> --force       # Remove novel from library
quelle library list                 # List all novels
quelle library show <novel>         # Show novel details

# Reading and Export
quelle read <novel> [chapter]       # Read a chapter
quelle export <novel> --format epub # Export novel

# Discovery
quelle search <query>               # Search for novels
quelle search <query> --author "Name" --tags "fantasy,adventure"

# Extension Management
quelle extensions list              # List installed extensions
quelle extensions install <id>     # Install extension from official registry
quelle extensions search <query>   # Search available extensions

# System Management
quelle status                       # Show system status
quelle config show                  # Show configuration

# Extension Store Management
quelle store list                   # List configured stores
quelle store add git <name> <url>   # Add a git-based extension store
quelle store update <name>          # Update store data
quelle store info <name>            # Show store information
```

### Example Workflow

```bash
# 1. Set up Quelle and install ScribbleHub extension
cargo build --release
cargo run -p quelle_cli -- store add local local ./data/stores/local
cargo component build -r -p extension_scribblehub --target wasm32-unknown-unknown
cargo run -p quelle_cli -- publish extension \
  ./target/wasm32-unknown-unknown/release/extension_scribblehub.wasm \
  --store local --overwrite
cargo run -p quelle_cli -- extensions install en.scribblehub

# 2. Search for a novel
quelle search "overlord light novel" --limit 5

# 3. Add a novel to your library
quelle add https://www.scribblehub.com/series/123456/overlord/

# 4. Read the first chapter
quelle read "Overlord" 1

# 5. Export to EPUB for your e-reader
quelle export "Overlord" --format epub --output ./books/

# 6. Keep your library updated
quelle update

# 7. Manage extension stores (optional)
quelle store list                   # See available stores
quelle extensions search "royal"    # Search for more extensions
```

## 🏗️ Architecture

Quelle uses a modular WebAssembly-based architecture with a distributed extension system:

- **CLI (`crates/cli`)**: User interface and command handling
- **Engine (`crates/engine`)**: Core runtime built with Wasmtime
- **Extension Framework (`crates/extension`)**: Shared library for WASM extensions  
- **Storage (`crates/storage`)**: Data persistence and library management
- **Store System (`crates/store`)**: Extension package management
- **Extensions (`extensions/`)**: Individual scrapers (dragontea, scribblehub)
- **WIT Interfaces (`wit/`)**: WebAssembly Interface Types definitions

### Extension Distribution
- **Official Registry**: [github.com/nacht-org/extensions](https://github.com/nacht-org/extensions) (configured by default)
- **Local Development**: Build and publish extensions locally for testing  
- **Custom Stores**: Add additional Git repositories or local directories

```bash
# View configured stores (official registry included by default)
quelle store list

# Add a custom extension store
quelle store add git custom-store https://github.com/user/my-extensions

# Search across all stores
quelle extensions search "light novel"

# Install from any configured store  
quelle extensions install custom.extension
```

## 🛠️ Development

### Prerequisites

```bash
# Install Rust and required tools
rustup target add wasm32-unknown-unknown
cargo install cargo-component

# Optional: Install just for convenient shortcuts
cargo install just
```

### Optional: Quick Commands (Justfile)

For developers who prefer shorter commands, a `justfile` is provided with convenient shortcuts:

```bash
# Quick setup
just setup                          # Set up local store and publish scribblehub

# Extension development  
just build scribblehub               # Build extension
just publish scribblehub             # Build and publish to local store
just dev scribblehub                 # Start development server
just test scribblehub --url <url>    # Test extension
just validate scribblehub            # Validate extension
just generate                        # Generate new extension

# Utilities
just list                           # List available extensions
just cli store list                 # Run CLI commands
just help                           # Show CLI help
```

All `just` commands are optional shortcuts for the full CLI commands shown throughout this documentation.

### Extension Development Workflow

```bash
# Validate extension structure and build
cargo run -p quelle_cli -- dev validate scribblehub --extended

# Quick test novel info fetching
cargo run -p quelle_cli -- dev test scribblehub --url "https://www.scribblehub.com/series/123456/novel/"

# Quick test search functionality  
cargo run -p quelle_cli -- dev test scribblehub --query "fantasy adventure"

# Start development server with hot reload
cargo run -p quelle_cli -- dev server scribblehub --watch
```

The development server provides:
- **Hot reload**: Automatic rebuild on file changes
- **Interactive testing**: Test novel fetching, search, and chapters
- **Real-time feedback**: Detailed timing and error information
- **Pure extensions**: No debugging code needed in extensions

### Creating New Extensions

**Interactive Mode (Recommended)**
```bash
# Interactive generation - prompts for all information
cargo run -p quelle_cli -- dev generate
```

**Command Line Mode**
```bash
# All parameters specified
cargo run -p quelle_cli -- dev generate mysite --display-name "My Site" --base-url "https://mysite.com"
```

Development workflow:
1. **Generate extension**: Interactive mode guides you through setup
2. **Customize selectors**: Update CSS selectors for your target site  
3. **Test iteratively**: Use `cargo run -p quelle_cli -- dev server <name> --watch` for hot reload testing
4. **Validate**: Use `cargo run -p quelle_cli -- dev validate <name> --extended` before publishing
5. **Publish**: Build and publish with CLI commands shown above

📖 **[Extension Generation Guide](./docs/EXTENSION_GENERATION.md)**
📖 **[Complete Extension Development Guide](./docs/EXTENSION_DEVELOPMENT.md)**

### Publishing Extensions

#### Automated Publishing (GitHub Actions)

Extensions are automatically published to the [official store](https://github.com/nacht-org/extensions) through multiple triggers:

- **PR Merge**: Triggered when pull requests with extension changes are merged
- **Release**: Publishes all extensions when a new release is created
- **Manual**: Workflow dispatch with options for specific or all extensions

The automated workflow:
1. Detects which extensions have changed (or all for releases)
2. Builds each extension to WebAssembly
3. Publishes to the official store with proper authentication
4. Creates build artifacts and summaries

#### Local Publishing with CLI

**CLI Publishing Commands:**
```bash
# Build extension first
cargo component build -r -p extension_scribblehub --target wasm32-unknown-unknown

# Basic publish to local store
cargo run -p quelle_cli -- publish extension \
  ./target/wasm32-unknown-unknown/release/extension_scribblehub.wasm \
  --store local --overwrite

# Publish with metadata
cargo run -p quelle_cli -- publish extension \
  ./target/wasm32-unknown-unknown/release/extension_scribblehub.wasm \
  --store local --notes "Bug fixes" --tags "manga,novels" --overwrite

# Dry run (show what would be done)
cargo run -p quelle_cli -- publish extension \
  ./target/wasm32-unknown-unknown/release/extension_scribblehub.wasm \
  --store local --dry-run

# Development mode (all dev flags)
cargo run -p quelle_cli -- publish extension \
  ./target/wasm32-unknown-unknown/release/extension_scribblehub.wasm \
  --store local --dev

# Show all options
cargo run -p quelle_cli -- publish extension --help
```

**Available Stores:**
- `local` - Local development store
- `remote` - Remote Git-based store  
- `official` - Official nacht-org/extensions registry

#### Manual Publishing Options

**GitHub Actions Workflow:**
- **Auto-Publish Extensions**: Manual dispatch with option to publish all extensions

**Local CLI:**
- Build extensions with `cargo component build -r -p extension_<name> --target wasm32-unknown-unknown`
- Use CLI publish commands for precise control over store, notes, tags, and options

#### Requirements for Official Publishing

To publish extensions to the official store automatically:

1. **Repository Setup**: Fork or contribute to this repository
2. **GitHub Token**: Set `EXTENSIONS_PUBLISH_TOKEN` secret with write access to `nacht-org/extensions`
3. **Extension Structure**: Follow the existing extension patterns in `extensions/`
4. **Testing**: Ensure extensions build successfully with `cargo component build -r -p extension_<name> --target wasm32-unknown-unknown`

#### Publishing Workflow

**Development → Production:**
1. Create feature branch with extension changes
2. Test locally with CLI commands or dry-run mode
3. Create pull request
4. Merge PR → Automatic publishing triggered
5. Extensions available in official store immediately

**Batch Updates:**
- Create release → All extensions published together

## 📚 Documentation

📖 **Comprehensive documentation is available in the [Quelle Book](./book/)**

The book contains detailed guides for:
- **User Guide**: Installation and usage
- **Store System**: Extension management
- **Development**: Architecture and extension development
- **API Reference**: Technical documentation

## 🤝 Contributing

We welcome contributions! Priority areas:

- **New Extensions**: Add support for more novel sources  
- **Export Formats**: Improve PDF generation, add new formats
- **Search Enhancement**: Better filtering and aggregation
- **Extension Development**: Improved debugging and testing tools
- **Documentation**: User guides and tutorials

### Contribution Guidelines

- Follow Rust coding standards (`cargo fmt`)
- Use the extension development tools for testing (`cargo run -p quelle_cli -- dev validate`, `cargo run -p quelle_cli -- dev test`)
- Keep extension code pure (no debugging utilities in production extensions)
- Update documentation for user-facing changes
- Respect websites' terms of service and robots.txt
- Handle rate limiting appropriately

## ⚖️ Legal

### License

```text
Copyright 2025 Mohamed Haisham

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

### Disclaimer

This application is not affiliated with any content providers. Users are responsible for ensuring their usage complies with the terms of service of the websites they access. The developers do not endorse or encourage any violation of copyright or terms of service.

## 🔗 Links

- **Documentation**: [Quelle Book](./book/)
- **Issues**: [GitHub Issues](https://github.com/nacht-org/quelle/issues)
- **Discussions**: [GitHub Discussions](https://github.com/nacht-org/quelle/discussions)