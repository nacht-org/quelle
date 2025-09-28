# Introduction

Quelle is an open-source e-book scraper built with Rust and WebAssembly. It downloads novels from web sources using a modular extension system.

## What is Quelle?

Quelle uses WebAssembly (WASM) extensions to scrape different websites. Each website gets its own extension, making it easy to add new sources without changing the main program.

**Key Benefits:**
- **Secure**: Extensions run in a sandbox and can't access your system files
- **Fast**: Built with Rust for high performance  
- **Cross-platform**: Works on Windows, macOS, and Linux
- **Extensible**: Easy to add support for new websites

## Current Status

🚧 **Status**: Early Development

Quelle is still being built. Here's what works now:

**✅ Working:**
- Core WASM engine for running extensions
- CLI with basic commands
- Store system for managing extensions
- Two sample extensions (DragonTea, ScribbleHub)
- Fetching novel info and chapters

**🔄 Coming Soon:**
- More extensions for popular sites
- EPUB/PDF export
- Better search across multiple sites
- Pre-built downloads

## Basic Concepts

- **Extensions**: WASM modules that know how to scrape specific websites
- **Stores**: Places where extensions are kept (like app stores)
- **CLI**: Command-line tool to search, fetch, and manage everything

## Quick Example

Once set up, using Quelle looks like this:

```bash
# Search for a novel
quelle search "novel title"

# Get novel info from a URL  
quelle fetch novel https://example.com/novel-page

# Get a specific chapter
quelle fetch chapter https://example.com/chapter-1
```

## Current Limitations

Since this is early development:

- **Manual setup required**: No simple installer yet
- **Limited extensions**: Only 2 working extensions
- **Development-focused**: Mainly for developers right now
- **Local stores only**: No online extension repositories yet

## Getting Started

Ready to try it? Check out:

1. [Installation](./installation.md) - How to build from source
2. [Getting Started](./getting-started.md) - First steps and basic usage
3. [Basic Usage](./basic-usage.md) - Common commands and workflows

## Legal Note

Quelle is a tool for accessing publicly available content. You're responsible for following website terms of service and copyright laws. Always respect content creators and website policies.