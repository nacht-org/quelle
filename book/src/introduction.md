# Introduction

Welcome to Quelle, an open-source, extensible, and portable e-book scraper that enables users to download e-books from multiple online sources. Built with Rust and WebAssembly, Quelle provides a powerful yet flexible platform for accessing digital content from various web novel and e-book platforms.

## What is Quelle?

Quelle is designed with modularity and extensibility at its core. Rather than being limited to specific websites or formats, Quelle uses a plugin-based architecture where each source is implemented as a WebAssembly (WASM) extension. This approach provides several key benefits:

- **Cross-platform compatibility**: WASM extensions run consistently across different operating systems
- **Security**: Extensions are sandboxed and cannot access system resources beyond what's explicitly allowed
- **Performance**: Native-level performance with the safety of managed execution
- **Extensibility**: Easy to add support for new sources without modifying the core engine

## Key Features

### Modular Architecture
- **Engine**: Core runtime built with Wasmtime for loading and managing extensions
- **Extensions**: Individual WASM components for different e-book sources
- **Store System**: Comprehensive package management for discovering, installing, and updating extensions
- **CLI Interface**: User-friendly command-line tools for all operations

### Store System
The store system is one of Quelle's most powerful features, providing:

- **Multiple Store Types**: Local file systems, Git repositories, HTTP registries
- **Package Management**: Install, update, and remove extensions with dependency resolution
- **Version Management**: Semantic versioning with conflict resolution
- **Search and Discovery**: Find extensions across multiple stores with advanced filtering
- **Security**: Checksum verification and trusted store management

### Supported Operations
- Search for novels across multiple sources
- Download complete novels with all chapters
- Export to multiple formats (EPUB, PDF, plain text, Markdown)
- Batch operations for multiple novels
- Resume interrupted downloads

## Project Status

🚧 **Current Status**: Pre-MVP Development

Quelle is currently under active development. The core architecture is in place and functional, including:

- ✅ Core WASM runtime engine
- ✅ Extension loading system
- ✅ Complete store management system
- ✅ CLI interface for store and extension management
- ✅ Sample extensions (dragontea, scribblehub)

### What's Coming
- 🔄 Enhanced CLI interface for novel operations
- 🔄 Multiple output formats (EPUB, PDF, etc.)
- 🔄 Cross-platform binaries
- 🔄 Git and HTTP store implementations
- 🔄 Web interface

## Getting Started

To get started with Quelle:

1. **Installation**: Follow the [Installation Guide](./installation.md)
2. **Basic Usage**: Learn the fundamentals in [Basic Usage](./basic-usage.md)
3. **Store Management**: Set up extension stores in [Store Management](./store/management.md)
4. **Extension Management**: Install and manage extensions in [Extension Management](./store/extensions.md)

## Community and Contributing

Quelle is an open-source project that welcomes contributions from the community. Whether you're interested in:

- Adding support for new e-book sources
- Improving the core engine
- Writing documentation
- Reporting bugs or suggesting features

Your contributions help make Quelle better for everyone. Check out our development guides to get started contributing.

## Legal Considerations

**Important**: Quelle is a tool for accessing publicly available content. Users are responsible for ensuring their use complies with the terms of service of the websites they access and applicable copyright laws. Always respect content creators and website policies when using Quelle.

The developer of this application does not have any affiliation with the content providers available through extensions.