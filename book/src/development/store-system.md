# Store System

This section explains how Quelle's store system works and how to work with it as a developer.

## Overview

Quelle uses a store system to manage extensions. Think of stores like app stores - they contain extensions that users can install and use.

## Store Types

### Local Stores

Local stores are directories on your file system containing extension files.

**Creating a local store:**
```bash
# Create directory
mkdir my-extensions

# Add as store  
quelle store add local my-extensions --name "dev"
```

**Structure:**
```
my-extensions/
├── dragontea.wasm
├── scribblehub.wasm
└── manifest.json (optional)
```

### Git Stores

Git stores are Git repositories containing extensions. Users can add remote repositories as extension sources.

**Adding a git store:**
```bash
quelle store add git https://github.com/user/extensions.git --name "upstream"
```

**With authentication:**
```bash
quelle store add git https://github.com/user/extensions.git \
  --name "private" \
  --token "ghp_token_here"
```

## Working with Stores

### Adding Extensions to Local Stores

**Manual method:**
```bash
# Build extension
cargo component build --release -p extension_dragontea

# Copy to store
cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm \
   my-extensions/dragontea.wasm
```

**Publishing method:**
```bash
# Publish to store (when implemented)
quelle publish extension ./extension.wasm --store dev
```

### Store Management Commands

```bash
# List all stores
quelle store list

# Add stores
quelle store add local ./path --name "local"
quelle store add git https://github.com/user/repo.git --name "remote"

# Remove store
quelle store remove local --force

# Update store data
quelle store update remote

# Check store health
quelle store info local
```

## Extension Discovery

When you run commands like `quelle add <url>`, here's what happens:

1. **URL Analysis**: Quelle examines the URL to determine which extension is needed
2. **Extension Search**: Searches through all configured stores for the extension
3. **Auto-Install**: If found, installs the extension automatically
4. **Execution**: Uses the extension to perform the requested operation

## Store Configuration

Stores are configured in your Quelle config file:

**Example config:**
```toml
[[stores]]
name = "local-dev"
type = "local"
path = "/home/user/quelle-extensions"
priority = 1

[[stores]]
name = "official"
type = "git"
url = "https://github.com/nacht-org/quelle-extensions.git"
priority = 2
branch = "main"
```

**Priority**: Lower numbers = higher priority. Quelle searches stores in priority order.

## Development Workflow

### Setting Up Development Environment

1. **Create local store for testing:**
   ```bash
   mkdir ./dev-extensions
   quelle store add local ./dev-extensions --name "dev" --priority 1
   ```

2. **Build and test extensions:**
   ```bash
   # Build extension
   just build-extension mysite
   
   # Copy to dev store
   cp target/wasm32-unknown-unknown/release/extension_mysite.wasm \
      ./dev-extensions/mysite.wasm
   
   # Test it
   quelle fetch novel https://mysite.com/novel/test
   ```

3. **Add remote stores for stable extensions:**
   ```bash
   quelle store add git https://github.com/example/extensions.git \
     --name "stable" --priority 2
   ```

### Extension Publishing

**Current method (manual):**
```bash
# Build extension
cargo component build --release -p extension_mysite

# Copy to store directory
cp target/wasm32-unknown-unknown/release/extension_mysite.wasm \
   /path/to/store/mysite.wasm
```

**Future publishing API:**
```bash
# Will be available in future versions
quelle publish extension ./extension.wasm --store upstream
quelle publish extension ./extension-dir/ --store upstream --dev
```

## Store Implementation Details

### Local Store Provider

- **Location**: File system directories
- **Format**: WASM files with optional manifest
- **Performance**: Fast access, no network required
- **Use case**: Development, private extensions

### Git Store Provider

- **Location**: Git repositories (local or remote)
- **Format**: Standard Git repo with WASM files
- **Performance**: Network dependent, cached locally
- **Use case**: Shared extensions, version control

### Store Manager

The store manager coordinates between multiple stores:

```rust
// Simplified example
pub struct StoreManager {
    stores: Vec<Box<dyn Store>>,
    registry: Box<dyn RegistryStore>,
}

impl StoreManager {
    pub async fn find_extension(&self, id: &str) -> Option<Extension> {
        // Search stores by priority order
        for store in &self.stores {
            if let Some(ext) = store.get_extension(id).await? {
                return Some(ext);
            }
        }
        None
    }
}
```

## File Formats

### Extension Files

Extensions are WebAssembly components with `.wasm` extension:

```
extension_dragontea.wasm  # WebAssembly component
extension_scribblehub.wasm
```

### Manifest Format (Optional)

Store manifests provide metadata:

```json
{
  "version": "1.0",
  "extensions": [
    {
      "id": "dragontea",
      "name": "DragonTea Scraper",
      "version": "1.0.0",
      "file": "dragontea.wasm",
      "description": "Scraper for DragonTea novels",
      "author": "Quelle Team",
      "supported_domains": ["dragontea.ink"]
    }
  ]
}
```

## Best Practices

### For Store Operators

1. **Organize extensions clearly:**
   ```
   extensions/
   ├── dragontea.wasm
   ├── scribblehub.wasm
   ├── royalroad.wasm
   └── manifest.json
   ```

2. **Use semantic versioning for Git stores:**
   ```bash
   git tag v1.0.0
   git tag v1.1.0
   ```

3. **Test extensions before publishing:**
   ```bash
   quelle fetch novel https://example.com/test-novel
   ```

### For Developers

1. **Use local stores for development:**
   - Fast iteration cycle
   - No network dependencies
   - Easy debugging

2. **Test with multiple stores:**
   ```bash
   # Set up both local and remote
   quelle store add local ./dev --name "dev" --priority 1
   quelle store add git https://github.com/example/extensions.git \
     --name "upstream" --priority 2
   ```

3. **Keep extensions focused:**
   - One extension per website/domain
   - Clear, descriptive names
   - Proper error handling

## Troubleshooting

### Extension Not Found

**Problem:** `No extension found for URL: https://example.com/novel`

**Solutions:**
1. Check if extension exists: `quelle extensions list`
2. Verify store configuration: `quelle store list`
3. Update stores: `quelle store update all`
4. Check extension file exists in store directory

### Store Connection Issues

**Problem:** Git store not accessible

**Solutions:**
1. Check network connectivity
2. Verify credentials: `quelle store info store-name`
3. Check repository permissions
4. Update store: `quelle store update store-name`

### Build Issues

**Problem:** Extension doesn't build properly

**Solutions:**
1. Check Rust and WASM target installation
2. Verify `cargo-component` is installed
3. Clean and rebuild: `cargo clean && just build-extension name`
4. Check for compilation errors in extension code

## Future Plans

The store system will be enhanced with:

- **Registry API**: Centralized extension registry
- **Signing**: Cryptographic verification of extensions
- **Dependencies**: Extension dependency management
- **Automatic Updates**: Background extension updates
- **Publishing Tools**: Streamlined publishing workflow

For now, the system provides a solid foundation for extension distribution and management.