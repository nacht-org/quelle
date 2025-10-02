# API Reference

This section provides essential API information for developers working with Quelle.

## Extension Development

### WIT Interfaces

Extensions must implement the WebAssembly Interface Types (WIT) defined in the `wit/` directory.

**Core Interface:**
```wit
interface novel-scraper {
  // Get novel metadata from a URL
  get-novel-info: func(url: string) -> result<novel-info, string>
  
  // Get chapter content from a URL
  get-chapter-content: func(url: string) -> result<chapter-content, string>
  
  // Search for novels (optional)
  search: func(query: string) -> result<list<novel-info>, string>
}
```

**Data Types:**
```wit
record novel-info {
  title: string,
  author: string,
  description: string,
  cover-url: option<string>,
  chapter-urls: list<string>,
  status: string,
}

record chapter-content {
  title: string,
  content: string,
  images: list<string>,
}
```

### Extension Structure

**Cargo.toml:**
```toml
[package]
name = "extension_mysite"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.24.0"
# Add other dependencies as needed
```

**lib.rs:**
```rust
use wit_bindgen::generate;

generate!({
    world: "novel-scraper",
});

// Export the interface implementation
export!(Component);

struct Component;

impl Guest for Component {
    fn get_novel_info(url: String) -> Result<NovelInfo, String> {
        // Implementation here
        todo!()
    }
    
    fn get_chapter_content(url: String) -> Result<ChapterContent, String> {
        // Implementation here
        todo!()
    }
    
    fn search(query: String) -> Result<Vec<NovelInfo>, String> {
        // Optional: implement search
        Err("Search not supported".to_string())
    }
}
```

## CLI Integration

### Adding New Commands

To add new CLI commands, modify `crates/cli/src/cli.rs`:

```rust
#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    // Existing commands...
    
    /// Your new command
    MyCommand {
        /// Command argument
        arg: String,
        /// Command option
        #[arg(long)]
        option: bool,
    },
}
```

Then handle it in `crates/cli/src/main.rs`:

```rust
match cli.command {
    // Existing matches...
    
    Commands::MyCommand { arg, option } => {
        handle_my_command(arg, option).await
    }
}
```

### Command Handler Pattern

Create command handlers in `crates/cli/src/commands/`:

```rust
use eyre::Result;

pub async fn handle_my_command(arg: String, option: bool) -> Result<()> {
    // Implementation here
    println!("Handling command with arg: {}, option: {}", arg, option);
    Ok(())
}
```

## Store System API

### Custom Store Providers

Implement the `StoreProvider` trait for custom stores:

```rust
use async_trait::async_trait;
use quelle_store::StoreProvider;

pub struct MyStoreProvider {
    // Your fields
}

#[async_trait]
impl StoreProvider for MyStoreProvider {
    async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>> {
        // Return available extensions
        todo!()
    }
    
    async fn get_extension(&self, id: &str) -> Result<Option<ExtensionData>> {
        // Return specific extension
        todo!()
    }
    
    async fn install_extension(&self, id: &str) -> Result<InstalledExtension> {
        // Install extension
        todo!()
    }
}
```

### Extension Metadata

Extensions should provide metadata in a standard format:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub supported_domains: Vec<String>,
    pub file_size: u64,
    pub checksum: String,
}
```

## Configuration API

### Config Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub stores: Vec<StoreConfig>,
    pub export: ExportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    pub name: String,
    pub store_type: String,
    pub path: Option<String>,
    pub url: Option<String>,
    pub priority: u32,
}
```

### Adding Config Options

To add new configuration options:

1. Update the config struct in `crates/cli/src/config.rs`
2. Add default values in the `Default` implementation
3. Update config validation logic
4. Add CLI commands to get/set the new options

## Error Handling

### Standard Error Types

Use the project's error types for consistency:

```rust
use quelle_store::error::StoreError;
use eyre::Result;

// For recoverable errors
fn my_function() -> Result<String, StoreError> {
    // Implementation
    Ok("success".to_string())
}

// For CLI commands
fn cli_function() -> eyre::Result<()> {
    // Implementation
    Ok(())
}
```

### Custom Error Messages

Provide helpful error messages:

```rust
if url.is_empty() {
    return Err(eyre::eyre!("URL cannot be empty"));
}

if !is_supported_domain(&url) {
    return Err(eyre::eyre!(
        "Domain not supported. Supported domains: {}",
        SUPPORTED_DOMAINS.join(", ")
    ));
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_my_function() {
        let result = my_function("test input");
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_async_function() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Integration Tests

Create integration tests in `tests/` directory:

```rust
use quelle_cli::commands::*;
use tempfile::TempDir;

#[tokio::test]
async fn test_add_novel() {
    let temp_dir = TempDir::new().unwrap();
    let storage = create_test_storage(temp_dir.path()).await;
    
    let result = handle_add_command(
        "https://example.com/novel".parse().unwrap(),
        false,
        None,
        &mut create_test_store_manager().await,
        &storage,
        false,
    ).await;
    
    assert!(result.is_ok());
}
```

## Build System

### Justfile Commands

Add new build commands to the `justfile`:

```just
# Build a specific extension
build-extension name:
    cargo component build --release -p extension_{{name}}

# Run tests
test:
    cargo test --all

# Format code
fmt:
    cargo fmt --all

# Check code quality
check:
    cargo clippy --all -- -D warnings
```

### CI/CD Integration

The project uses GitHub Actions. Add new workflows in `.github/workflows/`:

```yaml
name: Test Extensions
on: [push, pull_request]

jobs:
  test-extensions:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - run: cargo install just cargo-component
      - run: just build-extension dragontea
      - run: just build-extension scribblehub
```

## Development Workflow

### Setting Up Development Environment

1. **Clone and build:**
   ```bash
   git clone https://github.com/nacht-org/quelle
   cd quelle
   cargo build --release
   ```

2. **Install tools:**
   ```bash
   rustup target add wasm32-unknown-unknown
   cargo install just cargo-component
   ```

3. **Build extensions:**
   ```bash
   just build-extension dragontea
   just build-extension scribblehub
   ```

4. **Run tests:**
   ```bash
   cargo test --all
   ```

### Extension Development Cycle

1. **Create extension:**
   ```bash
   cargo new --lib extensions/mysite
   cd extensions/mysite
   # Set up Cargo.toml and implementation
   ```

2. **Build and test:**
   ```bash
   just build-extension mysite
   quelle fetch novel https://mysite.com/test-novel
   ```

3. **Debug issues:**
   ```bash
   quelle --verbose fetch novel https://mysite.com/test-novel
   ```

### Contributing Guidelines

1. **Code style:** Run `cargo fmt` before committing
2. **Testing:** Add tests for new functionality
3. **Documentation:** Update relevant documentation
4. **Error handling:** Use proper error types and messages
5. **Extensions:** Test thoroughly with real websites

## Future API Considerations

As Quelle evolves, these APIs may be added:

- **Plugin system** for custom export formats
- **Advanced search APIs** with filtering and ranking
- **Batch processing APIs** for handling multiple novels
- **Extension management APIs** for automatic updates
- **User preferences APIs** for customization

This API reference will be updated as the project develops and stabilizes.