# CLI Commands Reference

This section provides a complete reference for all Quelle CLI commands.

## Global Options

These options work with any command:

```bash
--storage-path <PATH>     # Override default storage location
--config <FILE>           # Use custom config file
--verbose, -v             # Show detailed output
--quiet, -q               # Show minimal output
--dry-run                 # Show what would be done without executing
--help, -h                # Show help information
--version, -V             # Show version information
```

## Core Commands

### `quelle add`

Add a novel to your library.

```bash
quelle add <URL> [OPTIONS]
```

**Arguments:**
- `<URL>` - Novel URL to add

**Options:**
- `--no-chapters` - Only fetch metadata, skip chapters
- `--max-chapters <NUM>` - Limit number of chapters to fetch

**Examples:**
```bash
quelle add https://dragontea.ink/novel/example
quelle add https://scribblehub.com/series/12345/story --no-chapters
quelle add https://example.com/novel --max-chapters 10
```

### `quelle update`

Update novels with new chapters.

```bash
quelle update [NOVEL] [OPTIONS]
```

**Arguments:**
- `[NOVEL]` - Novel ID, title, or "all" (default: "all")

**Options:**
- `--check-only` - Check for updates without downloading

**Examples:**
```bash
quelle update                          # Update all novels
quelle update "Novel Title"            # Update specific novel
quelle update novel-id-123             # Update by ID
quelle update --check-only             # Check all for updates
```

### `quelle read`

Read chapters from your library.

```bash
quelle read <NOVEL> [CHAPTER] [OPTIONS]
```

**Arguments:**
- `<NOVEL>` - Novel ID, title, or URL
- `[CHAPTER]` - Chapter number or title (optional)

**Options:**
- `--list` - Show chapter list instead of reading

**Examples:**
```bash
quelle read "Novel Title" --list       # List chapters
quelle read "Novel Title" 1            # Read chapter 1
quelle read novel-id-123 "Chapter One" # Read by title
```

### `quelle search`

Search for novels.

```bash
quelle search <QUERY> [OPTIONS]
```

**Arguments:**
- `<QUERY>` - Search terms

**Options:**
- `--author <NAME>` - Filter by author
- `--tags <TAG,TAG>` - Filter by tags (comma-separated)
- `--categories <CAT,CAT>` - Filter by categories
- `--limit <NUM>` - Maximum results to show

**Examples:**
```bash
quelle search "cultivation"
quelle search "magic" --author "AuthorName"
quelle search "fantasy" --tags "magic,adventure" --limit 5
```

### `quelle remove`

Remove a novel from your library.

```bash
quelle remove <NOVEL> [OPTIONS]
```

**Arguments:**
- `<NOVEL>` - Novel ID, title, or URL

**Options:**
- `--force` - Skip confirmation prompt

**Examples:**
```bash
quelle remove "Novel Title"
quelle remove novel-id-123 --force
```

## Library Management

### `quelle library`

Manage your local library.

```bash
quelle library <SUBCOMMAND>
```

#### `quelle library list`

List all novels in your library.

```bash
quelle library list [OPTIONS]
```

**Options:**
- `--source <SOURCE>` - Filter by source (e.g., "dragontea")

**Examples:**
```bash
quelle library list
quelle library list --source dragontea
```

#### `quelle library show`

Show detailed information for a novel.

```bash
quelle library show <NOVEL>
```

**Arguments:**
- `<NOVEL>` - Novel ID, title, or URL

#### `quelle library chapters`

List chapters for a novel.

```bash
quelle library chapters <NOVEL> [OPTIONS]
```

**Arguments:**
- `<NOVEL>` - Novel ID, title, or URL

**Options:**
- `--downloaded-only` - Show only downloaded chapters

#### `quelle library stats`

Show library statistics.

```bash
quelle library stats
```

#### `quelle library cleanup`

Clean up orphaned data and fix inconsistencies.

```bash
quelle library cleanup
```

## Extension Management

### `quelle extensions`

Manage extensions.

```bash
quelle extensions <SUBCOMMAND>
```

#### `quelle extensions list`

List installed extensions.

```bash
quelle extensions list [OPTIONS]
```

**Options:**
- `--detailed` - Show detailed information

#### `quelle extensions install`

Install an extension.

```bash
quelle extensions install <ID> [OPTIONS]
```

**Arguments:**
- `<ID>` - Extension ID

**Options:**
- `--version <VERSION>` - Install specific version
- `--force` - Force reinstallation

**Examples:**
```bash
quelle extensions install dragontea
quelle extensions install scribblehub --version 1.2.0
quelle extensions install myextension --force
```

#### `quelle extensions update`

Update extensions.

```bash
quelle extensions update <ID> [OPTIONS]
```

**Arguments:**
- `<ID>` - Extension ID or "all"

**Options:**
- `--prerelease` - Include pre-release versions
- `--force` - Force update even if no new version

**Examples:**
```bash
quelle extensions update all
quelle extensions update dragontea --force
```

#### `quelle extensions remove`

Remove an extension.

```bash
quelle extensions remove <ID> [OPTIONS]
```

**Arguments:**
- `<ID>` - Extension ID

**Options:**
- `--force` - Skip confirmation

#### `quelle extensions search`

Search for available extensions.

```bash
quelle extensions search <QUERY> [OPTIONS]
```

**Arguments:**
- `<QUERY>` - Search terms

**Options:**
- `--author <NAME>` - Filter by author
- `--limit <NUM>` - Maximum results (default: 20)

#### `quelle extensions info`

Show extension information.

```bash
quelle extensions info <ID>
```

**Arguments:**
- `<ID>` - Extension ID

## Store Management

### `quelle store`

Manage extension stores.

```bash
quelle store <SUBCOMMAND>
```

#### `quelle store add`

Add a new extension store.

```bash
quelle store add <TYPE> [OPTIONS]
```

**Local Store:**
```bash
quelle store add local <NAME> [PATH] [OPTIONS]
```

**Options:**
- `--priority <NUM>` - Store priority (default: 100)

**Git Store:**
```bash
quelle store add git <NAME> <URL> [OPTIONS]
```

**Options:**
- `--priority <NUM>` - Store priority (default: 100)
- `--branch <BRANCH>` - Git branch to track
- `--tag <TAG>` - Git tag to track
- `--commit <HASH>` - Git commit to pin to
- `--token <TOKEN>` - Authentication token
- `--ssh-key <PATH>` - SSH private key path
- `--username <USER>` - Username for basic auth
- `--password <PASS>` - Password for basic auth

**Examples:**
```bash
quelle store add local dev ./my-extensions --priority 1
quelle store add git upstream https://github.com/user/extensions.git
quelle store add git private https://github.com/user/private.git --token ghp_xxx
```

#### `quelle store remove`

Remove an extension store.

```bash
quelle store remove <NAME> [OPTIONS]
```

**Arguments:**
- `<NAME>` - Store name

**Options:**
- `--force` - Skip confirmation

#### `quelle store list`

List configured stores.

```bash
quelle store list
```

#### `quelle store update`

Update store data.

```bash
quelle store update <NAME>
```

**Arguments:**
- `<NAME>` - Store name or "all"

#### `quelle store info`

Show store information.

```bash
quelle store info <NAME>
```

**Arguments:**
- `<NAME>` - Store name

## Export Commands

### `quelle export`

Export novels to various formats.

```bash
quelle export <NOVEL> [OPTIONS]
```

**Arguments:**
- `<NOVEL>` - Novel ID, title, or "all"

**Options:**
- `--format <FORMAT>` - Output format (default: "epub")
- `--output <PATH>` - Output directory
- `--include-images` - Include images in export

**Examples:**
```bash
quelle export "Novel Title"
quelle export "Novel Title" --format pdf
quelle export all --output /path/to/exports
quelle export "Novel Title" --include-images
```

## Configuration

### `quelle config`

Manage configuration.

```bash
quelle config <SUBCOMMAND>
```

#### `quelle config set`

Set a configuration value.

```bash
quelle config set <KEY> <VALUE>
```

**Examples:**
```bash
quelle config set export.format epub
quelle config set data_dir /custom/path
```

#### `quelle config get`

Get a configuration value.

```bash
quelle config get <KEY>
```

#### `quelle config show`

Show all configuration.

```bash
quelle config show
```

#### `quelle config reset`

Reset configuration to defaults.

```bash
quelle config reset [OPTIONS]
```

**Options:**
- `--force` - Skip confirmation

## Advanced Commands

### `quelle status`

Show system status and health.

```bash
quelle status
```

### `quelle fetch`

Advanced fetch operations (for debugging).

```bash
quelle fetch <SUBCOMMAND>
```

#### `quelle fetch novel`

Fetch novel metadata only.

```bash
quelle fetch novel <URL>
```

#### `quelle fetch chapter`

Fetch specific chapter content.

```bash
quelle fetch chapter <URL>
```

#### `quelle fetch chapters`

Fetch all chapters for a novel.

```bash
quelle fetch chapters <NOVEL>
```

#### `quelle fetch all`

Fetch everything (novel + chapters).

```bash
quelle fetch all <URL>
```

## Publishing Commands

### `quelle publish`

Publish and manage extensions (for developers).

```bash
quelle publish <SUBCOMMAND>
```

#### `quelle publish extension`

Publish an extension.

```bash
quelle publish extension <PATH> [OPTIONS]
```

**Arguments:**
- `<PATH>` - Path to extension package or directory

**Options:**
- `--store <NAME>` - Target store name
- `--pre-release` - Mark as pre-release
- `--visibility <VIS>` - Extension visibility (public/private/unlisted)
- `--overwrite` - Overwrite existing version
- `--skip-validation` - Skip validation checks
- `--notes <TEXT>` - Release notes
- `--tags <TAGS>` - Tags (comma-separated)
- `--token <TOKEN>` - Authentication token
- `--timeout <SECS>` - Timeout in seconds (default: 300)
- `--dev` - Use development defaults

#### `quelle publish unpublish`

Remove a published extension version.

```bash
quelle publish unpublish <ID> <VERSION> [OPTIONS]
```

**Arguments:**
- `<ID>` - Extension ID
- `<VERSION>` - Version to unpublish

**Options:**
- `--store <NAME>` - Target store name
- `--reason <TEXT>` - Reason for unpublishing
- `--keep-record` - Keep tombstone record
- `--notify-users` - Notify users who installed this version
- `--token <TOKEN>` - Authentication token

#### `quelle publish validate`

Validate an extension package.

```bash
quelle publish validate <PATH> [OPTIONS]
```

**Arguments:**
- `<PATH>` - Path to extension package or directory

**Options:**
- `--store <NAME>` - Target store name
- `--strict` - Use strict validation rules
- `--verbose` - Show detailed validation results

#### `quelle publish requirements`

Show publishing requirements for stores.

```bash
quelle publish requirements [OPTIONS]
```

**Options:**
- `--store <NAME>` - Show requirements for specific store

## Exit Codes

- `0` - Success
- `1` - General error
- `2` - Invalid arguments
- `3` - Configuration error
- `4` - Network error
- `5` - File system error
- `6` - Extension error

## Environment Variables

- `QUELLE_CONFIG_DIR` - Override config directory
- `QUELLE_DATA_DIR` - Override data directory
- `QUELLE_CACHE_DIR` - Override cache directory
- `GITHUB_TOKEN` - GitHub authentication token
- `RUST_LOG` - Set logging level

## Examples

### Basic Workflow

```bash
# Check status
quelle status

# Add a novel
quelle add https://dragontea.ink/novel/example

# Read first chapter
quelle read "Example Novel" 1

# Update library
quelle update

# Export to EPUB
quelle export "Example Novel"
```

### Development Workflow

```bash
# Set up local store
quelle store add local dev ./my-extensions --priority 1

# Build and test extension
just build-extension mysite
cp target/wasm32-unknown-unknown/release/extension_mysite.wasm ./my-extensions/
quelle fetch novel https://mysite.com/test

# Publish when ready
quelle publish extension ./extension.wasm --store upstream
```

### Multi-Source Setup

```bash
# Add multiple stores
quelle store add local dev ./dev-extensions --priority 1
quelle store add git official https://github.com/nacht-org/extensions.git --priority 2

# Search across all sources
quelle search "cultivation"

# Install extensions from different stores
quelle extensions install new-extension
```
