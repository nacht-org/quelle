# Introduction

Quelle is an open-source e-book scraper that downloads novels from web sources using WebAssembly extensions.

## What is Quelle?

Quelle uses a modular extension system where each website gets its own WebAssembly (WASM) extension. This makes it secure and easy to add support for new sources.

**Key Features:**
- **Secure**: Extensions run in a WebAssembly sandbox
- **Fast**: Built with Rust for performance
- **Cross-platform**: Works on Windows, macOS, and Linux
- **Extensible**: Add new sources by creating extensions

## Current Status

🚧 **Status**: Pre-MVP Development

Quelle is actively being developed. Here's what currently works:

**✅ Available Now:**
- Core WASM runtime for extensions
- Command-line interface
- Extension store system
- Two working extensions: DragonTea and ScribbleHub
- Novel search, metadata fetching, and chapter downloading

**🔄 Coming Soon:**
- More extensions for popular novel sites
- EPUB and PDF export formats
- Improved search across multiple sources
- Pre-built installation packages

## How It Works

1. **Extensions**: WASM modules that know how to scrape specific websites
2. **Store System**: Manages and distributes extensions
3. **CLI Tool**: Command-line interface for all operations

## Quick Example

Once installed, basic usage looks like this:

```bash
# Add a novel to your library
quelle add https://example.com/novel-page

# Update your library with new chapters
quelle update

# Read a chapter
quelle read "Novel Title" 1

# Search for novels
quelle search "cultivation"
```

## Current Limitations

Since this is pre-MVP software:

- **Manual Setup**: No installer yet, build from source required
- **Limited Extensions**: Only 2 working extensions (DragonTea, ScribbleHub)
- **Developer-Focused**: Primarily for technical users right now
- **Local Only**: No online extension repositories yet

## Next Steps

Ready to try Quelle?

1. [Installation](./installation.md) - Build from source
2. [Getting Started](./getting-started.md) - First steps
3. [Basic Usage](./basic-usage.md) - Common workflows

For developers interested in contributing:

4. [Extension Development](./development/extension-development.md) - Create new scrapers
5. [Store System](./development/store-system.md) - Technical details

## Legal Notice

Quelle is a tool for accessing publicly available content. Users are responsible for complying with website terms of service and applicable laws. Always respect content creators and website policies.