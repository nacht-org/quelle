# Extension Management

> **⚠️ Note**: The extension management system is under active development and may change significantly before the 1.0 release. This documentation reflects the current implementation.

## Overview

Extension management covers the complete lifecycle of extensions in Quelle: discovering, installing, updating, and removing extensions from configured stores.

## Basic Extension Operations

### Installing Extensions

Install the latest version of an extension:

```bash
quelle extension install dragontea
```

Install a specific version:

```bash
quelle extension install dragontea --version 1.2.0
```

Install with options:

```bash
# Force reinstall
quelle extension install dragontea --force

# Skip dependencies
quelle extension install dragontea --no-deps
```

### Listing Installed Extensions

View all installed extensions:

```bash
quelle extension list
```

Shows extension name, version, source store, size, and installation date.

### Updating Extensions

Update a specific extension:

```bash
quelle extension update dragontea
```

Update all extensions:

```bash
quelle extension update all
```

Check for available updates without installing:

```bash
quelle extension check-updates
```

### Removing Extensions

Uninstall an extension:

```bash
# Remove from registry only
quelle extension uninstall dragontea

# Remove files completely
quelle extension uninstall dragontea --remove-files
```

### Extension Information

Get detailed information about an extension:

```bash
quelle extension info dragontea
```

Shows metadata, dependencies, installation details, and available versions.

## Extension Structure

Extensions consist of:

- **WASM Component**: The compiled WebAssembly module
- **Manifest**: Metadata and configuration (manifest.json)
- **Assets**: Optional additional files (documentation, examples)

## Installation Process

1. **Discovery**: Find extension in configured stores
2. **Download**: Retrieve package with integrity verification
3. **Validation**: Check checksums and compatibility
4. **Installation**: Extract to local directory
5. **Registry**: Update local extension registry

## Dependency Management

Currently simplified but will expand:

- Extensions can declare dependencies
- Dependencies are resolved during installation
- Circular dependencies are detected and prevented

## Programmatic API

```rust,no_run
use quelle_store::{StoreManager, InstallOptions};
use std::path::PathBuf;

// Initialize manager (example)
let mut manager = StoreManager::new(
    PathBuf::from("./extensions"),
    PathBuf::from("./cache")
).await?;

// Install extension
let options = InstallOptions::default();
let installed = manager.install("dragontea", None, Some(options)).await?;

// Check updates
let updates = manager.check_all_updates().await?;

// List installed
let installed = manager.list_installed();
```

## Version Management

- **Semantic Versioning**: Extensions use semver (1.2.3)
- **Conflict Resolution**: Newer versions preferred unless explicitly specified
- **Downgrade Protection**: Prevents accidental downgrades
- **Update Notifications**: Shows available updates with changelog links

## Security Considerations

- **Checksum Verification**: All packages verified with SHA256
- **Trusted Sources**: Extensions from trusted stores preferred
- **Sandboxing**: WASM provides natural security boundaries
- **Integrity Checks**: Regular validation of installed extensions

## Common Workflows

### Setting Up a New Environment

```bash
# Add a store
quelle store add local ./extensions

# Install essential extensions
quelle extension install dragontea
quelle extension install scribblehub

# Check everything is working
quelle extension list
```

### Regular Maintenance

```bash
# Check for updates
quelle extension check-updates

# Update all extensions
quelle extension update all

# Verify store health
quelle store health
```

## Limitations and Future Plans

**Current Limitations:**
- Only local stores fully supported
- Basic dependency resolution
- Limited metadata support

**Planned Features:**
- Advanced dependency resolution
- Extension signing and verification
- Rollback capabilities
- Extension configuration management
- Batch operations

For development information, see [Extension Development](../development/extension-development.md).