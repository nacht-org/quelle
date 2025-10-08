# Extension Development

Quelle uses WebAssembly extensions to scrape different novel sites. This guide covers how to create new extensions.

## Prerequisites

Make sure you have the required tools:

```bash
# Install Rust WebAssembly target
rustup target add wasm32-unknown-unknown

# Install cargo-component for building WASM extensions
cargo install cargo-component
```

## Quick Start

Generate a new extension:

```bash
# Interactive generation (recommended)
quelle dev generate

# Or specify details directly
quelle dev generate mysite --display-name "My Site" --base-url "https://mysite.com"
```

## Development Workflow

### 1. Generate Extension
The generator creates the basic structure in `extensions/mysite/`.

### 2. Customize Code
Edit `extensions/mysite/src/lib.rs` to match your target site's HTML structure. Look at existing extensions like `scribblehub` or `dragontea` for examples.

### 3. Build Extension
```bash
cargo component build -r -p extension_mysite --target wasm32-unknown-unknown
```

### 4. Test Extension
```bash
# Test with development server
quelle dev server mysite --watch

# Test individual functions
quelle dev test mysite --url "https://mysite.com/novel/123"
quelle dev test mysite --query "fantasy"
```

### 5. Validate
```bash
quelle dev validate mysite
```

## Publishing

Once your extension works:

```bash
# Set up local store (first time only)
quelle store add local local ./data/stores/local

# Publish extension
quelle publish extension \
  ./target/wasm32-unknown-unknown/release/extension_mysite.wasm \
  --store local --overwrite

# Install extension
quelle extensions install en.mysite
```

## Extension Structure

Each extension implements these functions:

- `info()` - Extension metadata
- `search()` - Search for novels
- `get_novel_info()` - Extract novel metadata from URL
- `get_chapter_list()` - Get list of chapters
- `get_chapter_content()` - Extract chapter content

## Tips

- Study existing extensions in `extensions/` directory
- Test CSS selectors in browser developer tools first
- Use the development server for rapid iteration
- Start simple and add complexity gradually
- Handle missing elements gracefully

## Getting Help

- Check existing extensions for examples
- Use `quelle dev --help` for available commands
- Test URLs in browser before implementing
- Report issues on GitHub