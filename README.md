# Quelle

This project provides an open-source, extensible and portable application that enables users to scrape e-books from multiple online sources. With its flexibility and support for multiple online sources, this e-book scraper provides a valuable tool for e-book enthusiasts who want to easily search and download e-books from the internet.

The repository holds both the Rust source code for the extensions and the runtime built using Wasmtime. This architecture allows for high-performance execution and cross-platform compatibility, making the application efficient and versatile.

## Project Architecture

Quelle uses a modular WebAssembly-based architecture:

- **Engine (`crates/engine`)**: Core runtime built with Wasmtime that loads and manages extensions
- **Extension Framework (`crates/extension`)**: Shared library for building WASM extensions
- **Extensions (`extensions/`)**: Individual scrapers for different e-book sources (dragontea, scribblehub)
- **WIT Interfaces (`wit/`)**: WebAssembly Interface Types defining the contract between engine and extensions

## Current Status

🚧 **Project Status**: Pre-MVP Development

This project is currently under active development. The core architecture is in place, but the MVP is not yet complete.

## Documentation

📚 **Comprehensive documentation is available in the [Quelle Book](./book/)**

- [Introduction](./book/src/introduction.md) - Project overview and key concepts
- [Getting Started](./book/src/getting-started.md) - Installation and first steps
- [Store System](./book/src/store/overview.md) - Extension package management
- [CLI Reference](./book/src/store/cli-reference.md) - Complete command reference
- [Development Guide](./book/src/development/store-implementation.md) - Technical implementation details

The book contains detailed guides for users, developers, and contributors. Since Quelle is under active development, the documentation reflects current capabilities while noting planned features.

### What Works
- ✅ Core WASM runtime engine
- ✅ Extension loading system
- ✅ Basic WIT interface definitions
- ✅ Sample extensions (dragontea, scribblehub)

### What's Coming
- 🔄 Complete CLI interface
- 🔄 Multiple output formats (EPUB, PDF, etc.)
- 🔄 Extension management system
- 🔄 Cross-platform binaries
- 🔄 Comprehensive documentation

## MVP Development Plan

### Phase 1: Core Infrastructure
- **CLI Interface Enhancement**
  - Comprehensive command-line interface with `search`, `download`, `list-sources`, `install-extension` commands
  - Configuration file support and progress indicators
- **Extension System Refinement**
  - Dynamic extension discovery and management
  - Extension metadata and validation system
- **WIT Interface Completion**
  - Review and enhance interface definitions
  - Comprehensive error handling

### Phase 2: Extension Development
- **Expand Existing Extensions**
  - Complete DragonTea and ScribbleHub implementations
  - Add search, metadata extraction, and chapter downloading
- **Add New Extensions** (3-4 popular sources from):
  - NovelFull, ReadLightNovel, Wuxiaworld, Royal Road, Archive.org, Project Gutenberg
- **Quality Assurance**
  - Test suites, integration tests, and performance optimization

### Phase 3: User Experience
- **Multiple Export Formats**: EPUB, PDF, plain text, Markdown
- **Enhanced Search**: Multi-source aggregation, filtering, caching
- **Download Management**: Batch downloads, resume capability, queue management

### Phase 4: Documentation & Distribution
- **Comprehensive Documentation**: Installation guides, tutorials, troubleshooting
- **Build Pipeline**: CI/CD with cross-platform binaries
- **Extension Ecosystem**: Registry, packaging standards, contribution guidelines

### MVP Success Criteria
- ✅ Successfully download complete e-books from 5+ sources

## Quick Start (Development)

```bash
# Clone the repository
git clone https://github.com/nacht-org/quelle
cd quelle

# Build an extension
just build-extension dragontea

# Run the engine (when CLI is complete)
cargo run -p quelle_engine -- --help
```

## Contributing

We welcome contributions! Here are ways you can help:

### Priority Areas for MVP
1. **Extension Development**: Add support for new e-book sources
2. **CLI Enhancement**: Improve user interface and experience
3. **Output Formats**: Implement EPUB, PDF generation
4. **Testing**: Add tests for existing functionality
5. **Documentation**: Improve guides and examples

### Development Setup
1. Install Rust (latest stable)
2. Install `just` command runner: `cargo install just`
3. Install WASM target: `rustup target add wasm32-unknown-unknown`
4. Install `cargo-component`: `cargo install cargo-component`

### Building Extensions
```bash
# Build a specific extension
just build-extension <extension-name>

# Example
just build-extension dragontea
```

### Contribution Guidelines
- Follow Rust coding standards and run `cargo fmt`
- Add tests for new functionality
- Update documentation for user-facing changes
- Extensions should handle rate limiting and respect robots.txt
- Ensure legal compliance with targeted sites' terms of service

### Extension Development
To create a new extension:
1. Copy an existing extension as a template
2. Implement the required WIT interfaces
3. Add appropriate error handling and logging
4. Test thoroughly with the target site
5. Add documentation and examples

See existing extensions in `extensions/` for reference implementations.

## License

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

## Disclaimer

The developer of this application does not have any affiliation with the content providers available.
