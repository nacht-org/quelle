# CLI Reference

> **⚠️ Note**: The CLI interface is under active development and commands may change significantly before the 1.0 release. This reference reflects the current implementation.

## Overview

Quelle provides a comprehensive command-line interface for managing stores and extensions. All commands follow a consistent pattern and provide helpful error messages and progress indicators.

## Global Options

```bash
quelle --help    # Show help for all commands
```

## Store Commands

### `quelle store`

Manage extension stores (repositories of extensions).

#### `quelle store add`

Add a new store to the configuration.

```bash
# Add local store
quelle store add local <path> [--name <name>]

# Examples
quelle store add local ./my-extensions
quelle store add local /usr/local/share/quelle/extensions --name "system"
```

**Options:**
- `--name <name>`: Custom name for the store (defaults to directory name)

#### `quelle store list`

List all configured stores.

```bash
quelle store list
```

Shows store name, type, trust status, and location.

#### `quelle store remove`

Remove a store from configuration.

```bash
quelle store remove <name>

# Example
quelle store remove "my-store"
```

#### `quelle store health`

Check health status of all configured stores.

```bash
quelle store health
```

Shows connectivity, response time, and extension count for each store.

#### `quelle store search`

Search for extensions across all configured stores.

```bash
quelle store search <query> [options]

# Examples
quelle store search "novel scraper"
quelle store search "chinese" --author "translator" --tags "webnovel"
```

**Options:**
- `--author <author>`: Filter by extension author
- `--tags <tags>`: Comma-separated list of tags to filter by
- `--sort <sort>`: Sort by (relevance, name, version, author, updated, downloads, size)
- `--limit <limit>`: Maximum number of results (default: 20)

#### `quelle store list-extensions`

List all available extensions from all stores.

```bash
quelle store list-extensions
```

## Extension Commands

### `quelle extension`

Manage installed extensions.

#### `quelle extension install`

Install an extension from available stores.

```bash
quelle extension install <name> [options]

# Examples
quelle extension install dragontea
quelle extension install dragontea --version 1.2.0
quelle extension install dragontea --force --no-deps
```

**Options:**
- `--version <version>`: Install specific version
- `--force`: Force reinstallation if already installed
- `--no-deps`: Skip dependency installation

#### `quelle extension update`

Update installed extensions.

```bash
# Update specific extension
quelle extension update <name> [options]

# Update all extensions
quelle extension update all [options]

# Examples
quelle extension update dragontea
quelle extension update all --prerelease
```

**Options:**
- `--prerelease`: Include pre-release versions
- `--force`: Force update even if no new version available

#### `quelle extension uninstall`

Remove an installed extension.

```bash
quelle extension uninstall <name> [options]

# Examples
quelle extension uninstall dragontea
quelle extension uninstall dragontea --remove-files
```

**Options:**
- `--remove-files`: Remove all files (not just registry entry)

#### `quelle extension list`

List all installed extensions.

```bash
quelle extension list
```

Shows extension name, version, source store, size, and installation date.

#### `quelle extension info`

Show detailed information about an extension.

```bash
quelle extension info <name>

# Example
quelle extension info dragontea
```

Shows metadata, dependencies, installation details, and available versions.

#### `quelle extension check-updates`

Check for available updates without installing.

```bash
quelle extension check-updates
```

Shows extensions with available updates, including security and breaking change indicators.

## Legacy Commands

These commands are for working with extensions directly (bypassing the store system):

### `quelle novel`

Fetch novel information using a specific extension.

```bash
quelle novel <url>
```

### `quelle chapter`

Fetch chapter content using a specific extension.

```bash
quelle chapter <url>
```

### `quelle search`

Search for novels using a specific extension.

```bash
quelle search <query>
```

## Output Indicators

The CLI uses various indicators to communicate status:

- `✓` Success indicator (green)
- `✗` Error indicator (red)
- `🔒` Security update available
- `⚠️` Breaking changes in update
- Progress bars for long-running operations

## Environment Variables

- `QUELLE_INSTALL_DIR`: Override default extension install directory
- `QUELLE_CACHE_DIR`: Override default cache directory
- `QUELLE_LOG_LEVEL`: Set logging level (error, warn, info, debug, trace)

## Configuration Files

Default locations:
- Extensions: `~/.local/share/quelle/extensions/`
- Cache: `~/.cache/quelle/`
- Config: `~/.config/quelle/`

## Exit Codes

- `0`: Success
- `1`: General error
- `2`: Invalid arguments
- `3`: Network error
- `4`: Permission error

## Examples

### Complete Setup Workflow

```bash
# Add a local store
quelle store add local ./extensions --name "dev"

# Check store health
quelle store health

# Search for extensions
quelle store search "novel"

# Install an extension
quelle extension install dragontea

# Check installation
quelle extension list

# Check for updates
quelle extension check-updates
```

### Maintenance Workflow

```bash
# Check store health
quelle store health

# Update all extensions
quelle extension update all

# Clean up unused extensions
quelle extension uninstall old-extension --remove-files
```

For more detailed usage examples, see the [Getting Started](../getting-started.md) guide.