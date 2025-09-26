# Basic Usage

> **⚠️ Note**: Quelle is currently in pre-MVP development. The usage patterns described here may change as the project evolves toward its 1.0 release.

## Overview

This guide covers the fundamental usage patterns for Quelle, focusing on the store system and extension management. Since Quelle is still in active development, this guide emphasizes what currently works while noting planned features.

## Core Concepts

Before diving into usage, it's helpful to understand Quelle's key concepts:

- **Extensions**: WebAssembly modules that handle specific e-book sources
- **Stores**: Repositories containing extensions (like package repositories)
- **Store Manager**: Coordinates multiple stores and manages installations
- **Local Registry**: Tracks what extensions are installed on your system

## Basic Workflow

### 1. Setting Up Stores

First, configure where Quelle should look for extensions:

```bash
# Add a local directory as a store
quelle store add local ./my-extensions --name "personal"

# Check that the store was added
quelle store list

# Verify store accessibility
quelle store health
```

### 2. Discovering Extensions

Find available extensions across your configured stores:

```bash
# Search for extensions
quelle store search "novel"

# List all available extensions
quelle store list-extensions

# Search with filters
quelle store search "chinese" --tags "translation" --author "team"
```

### 3. Installing Extensions

Install extensions for the sources you want to use:

```bash
# Install the latest version
quelle extension install dragontea

# Install a specific version
quelle extension install scribblehub --version 1.0.0

# Force reinstall if needed
quelle extension install dragontea --force
```

### 4. Managing Extensions

Keep track of and maintain your installed extensions:

```bash
# List installed extensions
quelle extension list

# Get detailed information about an extension
quelle extension info dragontea

# Check for available updates
quelle extension check-updates

# Update a specific extension
quelle extension update dragontea

# Update all extensions
quelle extension update all
```

### 5. Using Extensions (Limited)

Currently, extension usage is limited to direct testing:

```bash
# Test an extension directly (requires WASM file)
quelle novel https://example.com/novel-url
quelle chapter https://example.com/chapter-url
quelle search "novel title"
```

## Common Tasks

### Setting Up a Development Environment

```bash
# Create and configure a local development store
mkdir ./dev-extensions
quelle store add local ./dev-extensions --name "dev"

# Build sample extensions (requires source)
just build-extension dragontea
just build-extension scribblehub

# Verify everything works
quelle store health
quelle extension list
```

### Regular Maintenance

```bash
# Check store connectivity
quelle store health

# Look for extension updates
quelle extension check-updates

# Apply updates if available
quelle extension update all

# Clean up unused extensions
quelle extension uninstall old-extension --remove-files
```

### Managing Multiple Stores

```bash
# Add multiple stores with different priorities
quelle store add local ./official-extensions --name "official"
quelle store add local ./community-extensions --name "community"
quelle store add local ./dev-extensions --name "dev"

# List stores to see configuration
quelle store list

# Search across all stores
quelle store search "webnovel"
```

## Current Limitations

Since Quelle is in early development, be aware of these limitations:

### Store System
- **Local stores only**: Git and HTTP stores are planned but not implemented
- **Basic search**: Advanced filtering and sorting are limited
- **Manual setup**: Store configuration requires manual CLI commands

### Extension System
- **Limited metadata**: Rich extension information is partially implemented
- **Basic dependencies**: Dependency resolution is simplified
- **Manual installation**: Some extension setup may require manual steps

### Integration
- **Direct WASM usage**: Full integration between stores and engine is in progress
- **Limited automation**: Many tasks require manual intervention
- **Development focus**: The system is optimized for developers, not end users

## Best Practices

### Store Organization
- Use descriptive names for stores
- Organize stores by trust level (official, community, personal)
- Regular health checks to ensure store availability

### Extension Management
- Keep extensions updated for security and features
- Remove unused extensions to save space
- Use specific versions for critical workflows

### Development Workflow
- Test extensions in isolation before deploying
- Use local stores for development and testing
- Maintain backups of important extension configurations

## Troubleshooting

### Common Issues

**Store not accessible:**
```bash
# Check store health
quelle store health

# Verify path exists and permissions are correct
ls -la /path/to/store
```

**Extension not found:**
```bash
# Check which stores are configured
quelle store list

# Search across all stores
quelle store search "extension-name"
```

**Installation failures:**
```bash
# Check available disk space and permissions
# Try force reinstall
quelle extension install extension-name --force
```

### Getting Help

- Check the [CLI Reference](./store/cli-reference.md) for detailed command information
- Review the [Troubleshooting Guide](./advanced/troubleshooting.md) for specific issues
- See the [Development](./development/) section if you're contributing to the project

## What's Next

As Quelle develops toward its MVP, expect improvements in:

- **Simplified installation**: Pre-built binaries and package manager support
- **Enhanced store system**: Git repositories and HTTP registries
- **Better integration**: Seamless connection between stores and the engine
- **User-friendly interface**: Improved CLI and potential GUI
- **Automated workflows**: Batch operations and scripting support

For the latest updates, follow the project development and check the documentation regularly.