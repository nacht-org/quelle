# Store Overview

The store system in Quelle manages extensions - the WASM modules that know how to scrape specific websites. Think of it like an app store for website scrapers.

## What is a Store?

A store is simply a place where extensions are kept. Currently, Quelle only supports **local stores** - directories on your computer that contain WASM extension files.

## How Stores Work

```bash
# Add a directory as a store
quelle store add local ./my-extensions --name "personal"

# Quelle now knows to look in ./my-extensions for extension files
```

When you add a local store, Quelle will:
1. Remember the directory path
2. Look for `.wasm` files in that directory
3. Make those extensions available for installation and use

## Current Store Features

### What Works
- ✅ **Local directories**: Point to any folder with WASM files
- ✅ **Multiple stores**: Add several directories  
- ✅ **Store health checks**: Verify directories are accessible
- ✅ **Extension discovery**: Find extensions across all stores
- ✅ **Basic search**: Search for extensions by name

### What's Coming
- 🔄 **Git repositories**: Use Git repos as stores
- 🔄 **HTTP endpoints**: Remote extension registries
- 🔄 **Automatic updates**: Keep extensions up to date
- 🔄 **Dependency management**: Handle extension dependencies

## Store Commands

### Basic Store Management
```bash
# Add a store
quelle store add local ./extensions --name "main"

# List all stores
quelle store list

# Remove a store
quelle store remove main

# Check if stores are working
quelle store health
```

### Finding Extensions
```bash
# List extensions in all stores
quelle store list-extensions

# Search for specific extensions
quelle store search "dragontea"

# Show publishing requirements
quelle store requirements
```

## Current Limitations

- **Local only**: Only local directories work right now
- **Manual setup**: You need to manually copy WASM files to store directories
- **Basic metadata**: Extension information is limited
- **No versioning**: Simple file-based system without version management

## Example Workflow

1. **Build an extension**:
   ```bash
   just build-extension dragontea
   ```

2. **Create a store directory**:
   ```bash
   mkdir ./my-extensions
   ```

3. **Copy the WASM file**:
   ```bash
   cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./my-extensions/
   ```

4. **Add the store**:
   ```bash
   quelle store add local ./my-extensions --name "dev"
   ```

5. **Use the extension**:
   ```bash
   quelle fetch novel https://dragontea.ink/novel/example
   ```

## Store Configuration

Stores are tracked in `./data/config.json`:
```json
{
  "stores": [
    {
      "name": "dev",
      "store_type": "local",
      "path": "./my-extensions"
    }
  ]
}
```

This file is created automatically when you add your first store.

## Best Practices

1. **Use descriptive names**: `--name "official"` instead of `--name "store1"`
2. **Organize by source**: Keep different types of extensions in separate stores
3. **Regular health checks**: Run `quelle store health` to catch issues early
4. **Backup important stores**: Keep copies of your extension directories

## Getting Help

- Check [Store Management](./management.md) for detailed commands
- See [Extension Management](./extensions.md) for working with extensions
- Review [CLI Reference](./cli-reference.md) for all available options

The store system is the foundation for managing extensions in Quelle. While it's currently simple, it provides a solid base for the more advanced features coming in future releases.