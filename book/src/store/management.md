# Store Management

> **⚠️ Note**: The store system is under active development and APIs may change significantly before the 1.0 release. This documentation reflects the current implementation but should be considered preliminary.

## Overview

Store management in Quelle involves adding, configuring, and maintaining the repositories from which extensions can be discovered and installed. The store system is designed to be flexible and extensible, supporting multiple backend types.

## Basic Store Operations

### Adding Stores

Currently, only local file system stores are fully implemented:

```bash
# Add a local store
quelle store add local ./my-extensions --name "my-store"

# Add with default name (uses directory name)
quelle store add local /path/to/extensions
```

### Listing Stores

View all configured stores:

```bash
quelle store list
```

Output shows store name, type, trust level, and location.

### Removing Stores

Remove a store from configuration:

```bash
quelle store remove "store-name"
```

Note: This only removes the store from Quelle's configuration; it doesn't delete the actual files.

### Health Checking

Check if stores are accessible and functioning:

```bash
quelle store health
```

This command verifies each store's availability and reports any issues.

## Store Configuration

Stores can be configured with various options:

- **Priority**: Determines resolution order when extensions exist in multiple stores
- **Trust Level**: Trusted stores are preferred during conflict resolution
- **Custom Layouts**: Different file organization schemes per store

## Programmatic API

For developers building on Quelle:

```rust,no_run
use quelle_store::{StoreManager, local::LocalStore};
use std::path::PathBuf;

// Initialize manager
let install_dir = PathBuf::from("./extensions");
let cache_dir = PathBuf::from("./cache");
let mut manager = StoreManager::new(install_dir, cache_dir).await?;

// Add stores
let local_store = LocalStore::new("./extensions")?;
manager.add_store(local_store);

// List stores
let stores = manager.list_stores();
```

## Future Store Types

The following store types are planned but not yet implemented:

- **Git Stores**: Git repository-based extension storage
- **HTTP Stores**: Web-based registries with API access
- **S3 Stores**: Cloud storage backends

## Best Practices

1. **Use descriptive names** for stores to make management easier
2. **Mark trusted sources** appropriately for security
3. **Set priorities** based on preference and trust level
4. **Regular health checks** to ensure store availability
5. **Backup configurations** before making significant changes

## Troubleshooting

Common issues and solutions:

- **Store not found**: Check path and permissions
- **Health check failures**: Verify network connectivity for remote stores
- **Permission errors**: Ensure read/write access to store directories

For more detailed troubleshooting, see the [Troubleshooting Guide](../advanced/troubleshooting.md).