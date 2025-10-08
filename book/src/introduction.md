# Introduction

Quelle is an open-source novel scraper and library manager that downloads novels from web sources using WebAssembly extensions. Built with a modular architecture, it provides high performance and cross-platform compatibility for managing your digital novel collection.

## What is Quelle?

Quelle uses a secure, modular extension system where each website gets its own WebAssembly (WASM) extension. This architecture ensures security through sandboxing while making it easy to add support for new sources.

**Key Features:**
- **Secure**: Extensions run in a WebAssembly sandbox for complete isolation
- **Fast**: Built with Rust for maximum performance
- **Cross-platform**: Works seamlessly on Windows, macOS, and Linux
- **Extensible**: Easy-to-use extension system with development tools
- **Full Library Management**: Search, download, organize, and export your collection
- **Multiple Formats**: Export to EPUB, PDF, and other formats
- **Auto-Updates**: Keep your library current with new chapters

## Current Status

**Status**: MVP Ready

Quelle has reached MVP (Minimum Viable Product) status with a fully functional CLI, working extension system, and reliable core features.

**What Works Now:**
- **Complete CLI Interface**: All major commands implemented and stable
- **Extension System**: Build, install, and manage extensions with full tooling
- **Store Management**: Local and Git-based extension repositories
- **Novel Discovery**: Search and browse novels across multiple sources
- **Library Management**: Add, update, remove, and organize your collection
- **Chapter Reading**: Read chapters directly in your terminal
- **Export Functionality**: Export to EPUB and PDF formats
- **Development Tools**: Extension generator, dev server, testing tools
- **Three Working Extensions**: ScribbleHub, DragonTea, and RoyalRoad

**🔄 In Active Development:**
- Additional novel source extensions
- Enhanced export format options
- Improved search and filtering capabilities
- Cross-platform binary distribution
- Advanced library organization features

## Available Extensions

Quelle currently supports these novel sources:

- **ScribbleHub** (`scribblehub`): Original novels and translations from ScribbleHub.com
- **DragonTea** (`dragontea`): Light novels and web novels from DragonTea.ink
- **RoyalRoad** (`royalroad`): Original fiction and stories from RoyalRoad.com

*More extensions are available at the [official extension repository](https://github.com/nacht-org/extensions)*

## How It Works

Quelle's architecture consists of several key components:

1. **CLI Interface**: Command-line tool for all user interactions
2. **Extensions**: WebAssembly modules that handle website-specific scraping
3. **Store System**: Manages extension distribution and updates
4. **Storage Engine**: Handles novel metadata, chapters, and library organization
5. **Development Tools**: Complete toolkit for creating new extensions

## Quick Example

Here's what basic usage looks like:

```bash
# Search for novels across all sources
quelle search "cultivation fantasy"

# Add a novel to your library (downloads all chapters)
quelle add https://www.royalroad.com/fiction/12345/novel-title

# Update your library with new chapters
quelle update

# Read a chapter in your terminal
quelle read "Novel Title" 1

# Export to EPUB for your e-reader
quelle export "Novel Title" --format epub
```

## Extension Development

Quelle makes extension development straightforward with comprehensive tooling:

```bash
# Generate a new extension interactively
quelle dev generate

# Start development server with hot reload
quelle dev server myextension --watch

# Test your extension
quelle dev test myextension --url "https://example.com/novel"

# Validate before publishing
quelle dev validate myextension --extended
```

## Current Capabilities

Since reaching MVP status, Quelle offers:

**For Users:**
- Stable, feature-complete CLI interface
- Reliable novel downloading and management
- Multiple export formats (EPUB, PDF)
- Extension installation from official registry
- Library organization and chapter tracking

**For Developers:**
- Complete extension development toolkit
- Extension generator with templates
- Development server with hot reload
- Testing and validation tools
- Local and remote extension stores
- Automated publishing workflows

## Next Steps

Ready to get started with Quelle?

**For Users:**
1. [Installation](./installation.md) - Set up Quelle on your system
2. [Getting Started](./getting-started.md) - Your first novel and basic workflows
3. [Basic Usage](./basic-usage.md) - Complete guide to all features

**For Developers:**
4. [Extension Development](./development/extension-development.md) - Create scrapers for new sources
5. [Store System](./development/store-system.md) - Understand the extension distribution system

## Extension Registry

Quelle comes pre-configured with access to the [official extension registry](https://github.com/nacht-org/extensions), which provides:

- Curated, tested extensions
- Automatic updates and security patches  
- Easy installation with `quelle extensions install <name>`
- Community-contributed sources

You can also add custom extension stores for private or experimental extensions.

## Legal Notice

Quelle is a tool for accessing publicly available content. Users are responsible for:

- Complying with website terms of service
- Respecting content creators and copyright
- Following applicable laws in their jurisdiction
- Using rate limiting and respectful scraping practices

Always ensure your usage respects the policies of the websites you're accessing.