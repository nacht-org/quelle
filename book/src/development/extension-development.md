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
just generate
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
just dev mysite

# Test individual functions
just test mysite --url "https://mysite.com/novel/123"
just test mysite --query "fantasy"
```

### 5. Validate
```bash
just validate mysite
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

## Handling Dynamic Content

Many modern websites load content dynamically with JavaScript. Quelle supports element waiting to handle these cases:

### Element Waiting

For sites that load content after the initial page load, you can wait for specific elements:

```rust
use quelle_extension::prelude::*;

// Wait for dynamic content to load
let response = Request::get(&url)
    .wait_for_element(".novel-details")  // Wait for this CSS selector
    .wait_timeout(10000)                 // Optional: 10 second timeout (default: 30s)
    .send(&client)?
    .error_for_status()?;

let html = response.text()?.unwrap_or_default();
let document = scraper::Html::parse_document(&html);
// ... continue with normal scraping
```

### Browser Compatibility

- **Chrome Headless**: Full support - element waiting works as expected
- **Reqwest**: Graceful fallback - element waiting options are ignored, standard HTTP request is made

Your extension will work with both executors, but Chrome provides enhanced support for JavaScript-heavy sites.

### Common Use Cases

```rust
// Wait for main content area
.wait_for_element("#main-content")

// Wait for chapter content to load
.wait_for_element(".chapter-text")

// Wait for search results
.wait_for_element(".search-results .novel-item:first-child")

// Wait for pagination controls
.wait_for_element(".pagination")
```

### Best Practices

- Use specific CSS selectors that uniquely identify the content you need
- Set reasonable timeouts (5-15 seconds) to balance functionality with user experience
- Always test with both Chrome and Reqwest executors during development
- Handle cases where elements might not appear within the timeout

## Tips

- Study existing extensions in `extensions/` directory
- Test CSS selectors in browser developer tools first
- Use the development server for rapid iteration
- Start simple and add complexity gradually
- Handle missing elements gracefully
- Use element waiting only when necessary - static sites are faster with regular HTTP requests

## Getting Help

- Check existing extensions for examples
- Use `just --list` to see available development commands
- Test URLs in browser before implementing
- For advanced options, use the CLI directly: `cargo run -p quelle_cli -- dev --help`
- Report issues on GitHub