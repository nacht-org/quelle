# Store Management

This guide covers how to manage extension stores in Quelle. Stores are directories where extensions (WASM files) are kept and organized.

## What Are Stores?

A store is simply a directory that contains extension files. Quelle looks in these directories to find available extensions that can scrape different websites.

Currently, Quelle only supports **local stores** - directories on your computer.

## Adding Stores

### Add a Local Directory

```bash
# Add a directory as a store
quelle store add local ./my-extensions --name "personal"

# Add without a custom name (auto-generated)
quelle store add local ./extensions

# Add with absolute path
quelle store add local /home/user/quelle-extensions --name "main"
```

The directory doesn't need to exist yet - Quelle will create it if needed.

## Listing Stores

```bash
# Show all configured stores
quelle store list
```

Example output:
```text
Configured stores:
  📦 personal (local) - ./my-extensions
  📦 main (local) - /home/user/quelle-extensions
```

## Removing Stores

```bash
# Remove a store by name
quelle store remove personal

# This only removes it from Quelle's configuration
# The directory and files are not deleted
```

## Checking Store Health

```bash
# Check if all stores are accessible
quelle store health
```

Example output:
```text
Registry Status:
  Configured stores: 2
  personal (local): ✅ Healthy
    Extensions: 2
  main (local): ❌ Unhealthy
    Error: Directory not found: /home/user/quelle-extensions
```

Common health issues:
- **Directory not found**: Create the directory or fix the path
- **Permission denied**: Check directory permissions with `ls -la`
- **No extensions**: The directory exists but has no WASM files

## Working with Store Contents

### List Extensions in Stores

```bash
# Show extensions available in all stores
quelle store list-extensions

# or use the shorter command
quelle list
```

This shows extensions found across all your configured stores.

### Search for Extensions

```bash
# Search by name
quelle store search "dragontea"

# Search with basic filters
quelle store search "novel" --author "author-name"
```

## Managing Store Contents

### Adding Extensions to Stores

Currently, you need to manually copy WASM files:

```bash
# Build an extension first
just build-extension dragontea

# Copy to your store directory
cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./my-extensions/
```

### Store Directory Structure

A typical store directory looks like:
```text
my-extensions/
├── extension_dragontea.wasm
├── extension_scribblehub.wasm
└── extension_custom_site.wasm
```

Just put WASM files directly in the directory. No subdirectories or special organization is needed.

## Store Configuration

Stores are saved in `./data/config.json`:

```json
{
  "stores": [
    {
      "name": "personal", 
      "store_type": "local",
      "path": "./my-extensions"
    }
  ]
}
```

This file is created automatically when you add your first store.

## Common Tasks

### Set Up Your First Store

```bash
# 1. Create a directory for extensions
mkdir ./my-extensions

# 2. Add it as a store
quelle store add local ./my-extensions --name "dev"

# 3. Build and copy an extension
just build-extension dragontea
cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./my-extensions/

# 4. Verify everything works
quelle store health
quelle list
```

### Organize Multiple Stores

```bash
# Separate stores for different purposes
quelle store add local ./official-extensions --name "official"
quelle store add local ./community-extensions --name "community"  
quelle store add local ./dev-extensions --name "dev"
```

### Clean Up Stores

```bash
# Remove stores you no longer need
quelle store remove old-store

# Check what's left
quelle store list
```

## Troubleshooting

### Store Not Found
```bash
# Check if the directory exists
ls -la ./my-extensions

# Create it if missing
mkdir -p ./my-extensions

# Re-add the store
quelle store add local ./my-extensions --name "dev"
```

### Permission Denied
```bash
# Check permissions
ls -la ./my-extensions

# Fix permissions if needed
chmod 755 ./my-extensions
```

### No Extensions Found
```bash
# Make sure WASM files are in the directory
ls -la ./my-extensions/*.wasm

# Build and copy extensions if missing
just build-extension dragontea
cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./my-extensions/
```

### Store Health Fails
```bash
# Run health check to see specific errors
quelle store health

# Common fixes:
# - Create missing directories
# - Fix file permissions
# - Check paths in config.json
```

## Best Practices

1. **Use descriptive names**: `--name "official"` instead of `--name "store1"`
2. **Organize by purpose**: Keep different types of extensions in separate stores
3. **Regular health checks**: Run `quelle store health` periodically
4. **Backup important stores**: Keep copies of directories with custom extensions
5. **Document your setup**: Note which stores contain which extensions

## Future Features

Coming in later releases:
- **Git stores**: Use Git repositories as extension stores
- **HTTP stores**: Download extensions from web registries
- **Automatic publishing**: Easy way to add extensions to stores
- **Version management**: Handle different extension versions
- **Dependencies**: Manage extension dependencies

For now, the local store system provides a solid foundation for managing extensions manually.