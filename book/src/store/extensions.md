# Extension Management

This guide covers how to install, update, and manage extensions in Quelle. Extensions are WASM modules that know how to scrape specific websites.

## What Are Extensions?

Extensions are WebAssembly (WASM) files that contain the logic for scraping specific websites. Each website typically needs its own extension because they all have different page structures and APIs.

Currently available extensions:
- **dragontea**: Scrapes novels from DragonTea sites
- **scribblehub**: Scrapes novels from ScribbleHub

## Extension Commands

### List Extensions

```bash
# List installed extensions
quelle extension list

# List all available extensions in stores
quelle store list-extensions
# or shorter:
quelle list
```

### Install Extensions

```bash
# Install an extension (if available in a store)
quelle extension install dragontea

# Install specific version (when versioning is implemented)
quelle extension install dragontea --version 1.0.0

# Force reinstall
quelle extension install dragontea --force
```

**Note**: Currently, automatic installation has limitations. You may need to build and copy extensions manually.

### Get Extension Information

```bash
# Show details about an installed extension
quelle extension info dragontea

# Show general extension status
quelle extension list
```

### Update Extensions

```bash
# Update a specific extension
quelle extension update dragontea

# Update all extensions
quelle extension update all

# Include pre-release versions
quelle extension update dragontea --prerelease
```

### Uninstall Extensions

```bash
# Remove an extension
quelle extension uninstall dragontea

# Remove with cleanup
quelle extension uninstall dragontea --remove-data
```

## Manual Extension Setup

Currently, the easiest way to set up extensions is manually:

### 1. Build Extensions

```bash
# Build DragonTea extension
just build-extension dragontea

# Build ScribbleHub extension
just build-extension scribblehub

# This creates WASM files in target/wasm32-unknown-unknown/release/
```

### 2. Set Up Store

```bash
# Create store directory
mkdir ./my-extensions

# Add as store
quelle store add local ./my-extensions --name "dev"
```

### 3. Copy Extensions

```bash
# Copy built extensions to store
cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./my-extensions/
cp target/wasm32-unknown-unknown/release/extension_scribblehub.wasm ./my-extensions/
```

### 4. Verify Setup

```bash
# Check that extensions are found
quelle list

# Test extension
quelle fetch novel https://dragontea.ink/novel/example
```

## Using Extensions

Once extensions are installed, they work automatically when you fetch from supported URLs:

### Automatic Detection

```bash
# Quelle automatically finds the right extension for each URL
quelle fetch novel https://dragontea.ink/novel/some-novel
quelle fetch novel https://scribblehub.com/series/123456/title/
```

### Search with Extensions

```bash
# Search across extensions that support search
quelle search "cultivation novel"

# Search with filters
quelle search "romance" --author "author name"
```

### Chapter Fetching

```bash
# Fetch individual chapters
quelle fetch chapter https://dragontea.ink/novel/example/chapter-1
quelle fetch chapter https://scribblehub.com/read/123456/chapter/1/
```

## Extension Development

### For Developers

If you want to create your own extensions:

1. **Study existing extensions**: Look at `extensions/dragontea/` and `extensions/scribblehub/`
2. **Understand WIT interfaces**: Check `wit/` directory for the interface definitions
3. **Use the extension framework**: Import `quelle_extension` crate
4. **Build as WASM component**: Use `cargo component build`

### Building Custom Extensions

```bash
# Create new extension (copy existing as template)
cp -r extensions/dragontea extensions/mysite

# Modify the extension code for your target site
# Edit extensions/mysite/src/lib.rs

# Build the extension
just build-extension mysite

# Add to your store
cp target/wasm32-unknown-unknown/release/extension_mysite.wasm ./my-extensions/
```

## Publishing Extensions

For development purposes, you can publish extensions to local stores:

```bash
# Publish to local store
quelle extension publish ./target/wasm32-unknown-unknown/release/extension_mysite.wasm --store dev

# Publish with overwrite
quelle extension publish ./extension.wasm --store dev --overwrite
```

## Extension Status and Health

### Check Extension Status

```bash
# See what extensions are installed
quelle extension list

# Check store health (includes extensions)
quelle store health

# Overall system status
quelle status
```

### Extension Metadata

Currently limited, but extensions can provide:
- Name and description
- Supported domains
- Version information
- Author information

## Current Limitations

### What Works
- ✅ Loading WASM extensions
- ✅ Automatic URL-to-extension matching
- ✅ Basic extension management
- ✅ Manual installation process

### What's Coming
- 🔄 Automatic installation from stores
- 🔄 Version management and updates
- 🔄 Dependency resolution
- 🔄 Extension metadata and descriptions
- 🔄 Built-in extension registry
- 🔄 Extension compatibility checking

## Troubleshooting

### Extension Not Found

```bash
# Check if extension exists in stores
quelle list

# Build and add extension manually
just build-extension dragontea
cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./my-extensions/
```

### Extension Fails to Load

```bash
# Check WASM file exists
ls -la ./my-extensions/*.wasm

# Rebuild extension
just build-extension dragontea

# Check extension compatibility (future feature)
```

### URL Not Supported

```bash
# Check which extensions are available
quelle list

# Verify URL format matches what extension expects
# Some extensions may only support specific URL patterns
```

### Installation Fails

```bash
# Try manual installation instead
just build-extension extension-name
cp target/wasm32-unknown-unknown/release/extension_*.wasm ./my-extensions/

# Check store configuration
quelle store list
quelle store health
```

## Best Practices

1. **Keep extensions updated**: Rebuild when the main project updates
2. **Test extensions**: Verify they work with real URLs before relying on them
3. **Organize by source**: Group related extensions in the same stores
4. **Backup custom extensions**: Keep copies of any extensions you modify
5. **Document supported sites**: Note which URLs work with each extension

## Extension Registry

Currently, there's no central registry for extensions. You need to:
- Build from source
- Copy WASM files manually
- Manage versions yourself

Future releases will include:
- Official extension registry
- Community extension sharing
- Automatic updates
- Dependency management

## Getting Help

For extension-related issues:
1. Check this troubleshooting section
2. Verify extensions are built and copied correctly
3. Test URLs in a web browser first
4. Check the project's GitHub issues for known problems
5. Look at existing extension code for examples

Extension management will become much simpler as Quelle develops, but the current manual process gives you full control over which extensions are available and how they're organized.