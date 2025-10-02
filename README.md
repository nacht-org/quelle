# Quelle

Quelle is a powerful, extensible novel scraper and library manager that enables you to search, download, and manage e-books from multiple online sources. Built with a modular WebAssembly architecture, it provides high performance and cross-platform compatibility.

## 🚀 Quick Start

### Installation

```bash
# Clone and build
git clone https://github.com/nacht-org/quelle
cd quelle
cargo build --release

# Set up extension system and install ScribbleHub extension
just setup

# Or manually:
# just reset-store
# just publish scribblehub
# cargo run -p quelle_cli -- extensions install en.scribblehub
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
- 🛠️ **Easy development** - Simple API for creating new source extensions
- 📦 **Package management** - Version control and dependency management

### Current Sources
- **ScribbleHub** - Original novels and translations
- **DragonTea** - Light novels and web novels
- *More sources coming soon...*

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
quelle extensions install <id>     # Install an extension
quelle extensions search <query>   # Search available extensions

# System Management
quelle status                       # Show system status
quelle config show                  # Show configuration
quelle store list                   # List configured stores
```

### Example Workflow

```bash
# 1. Set up Quelle
just setup

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
```

## 🏗️ Architecture

Quelle uses a modular WebAssembly-based architecture:

- **CLI (`crates/cli`)**: User interface and command handling
- **Engine (`crates/engine`)**: Core runtime built with Wasmtime
- **Extension Framework (`crates/extension`)**: Shared library for WASM extensions  
- **Storage (`crates/storage`)**: Data persistence and library management
- **Store System (`crates/store`)**: Extension package management
- **Extensions (`extensions/`)**: Individual scrapers (dragontea, scribblehub)
- **WIT Interfaces (`wit/`)**: WebAssembly Interface Types definitions

## 🛠️ Development

### Prerequisites

```bash
# Install Rust and required tools
rustup target add wasm32-unknown-unknown
cargo install just cargo-component
```

### Building Extensions

```bash
# Build a specific extension
just build-extension scribblehub

# Publish to local store
just publish scribblehub

# Build and run CLI
cargo run -p quelle_cli -- --help
```

### Creating New Extensions

1. Copy an existing extension as template
2. Implement the required WIT interfaces
3. Build and test: `just build-extension <name>`
4. Publish: `just publish <name>`

See existing extensions in `extensions/` for reference implementations.

## 📚 Documentation

📖 **Comprehensive documentation is available in the [Quelle Book](./book/)**

The book contains detailed guides for:
- **User Guide**: Installation and usage
- **Store System**: Extension management
- **Development**: Architecture and extension development
- **API Reference**: Technical documentation

## 🤝 Contributing

We welcome contributions! Priority areas:

1. **New Extensions**: Add support for more novel sources
2. **Export Formats**: Improve PDF generation, add new formats
3. **Search Enhancement**: Better filtering and aggregation
4. **Testing**: Improve test coverage
5. **Documentation**: User guides and tutorials

### Contribution Guidelines

- Follow Rust coding standards (`cargo fmt`)
- Add tests for new functionality
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