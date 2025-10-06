# Quelle CLI

A command-line interface for the Quelle novel scraper and library manager.

## Features

- **Fetch novels and chapters** with automatic asset handling
- **Manage local library** with powerful organization tools
- **Export to multiple formats** (EPUB, PDF, HTML, TXT)
- **Search across sources** using extensions
- **Extension management** for adding new novel sources
- **Configuration management** for customizing behavior

## Installation

```bash
cargo install --path crates/cli
```

## Features

The CLI supports the following optional features:

- `pdf` - Enable PDF export support (includes Typst and HTML parsing dependencies, enabled by default)
- `git` - Enable git-based extension stores (enabled by default)

```bash
# Build with all features (default)
cargo build --release

# Build without PDF support (EPUB only, excludes Typst and scraper dependencies)
cargo build --release --no-default-features --features git

# Build with specific features
cargo build --release --features git,pdf
```

## Usage

### Basic Commands

```bash
# Show help
quelle --help

# Fetch a novel (includes cover image automatically)
quelle fetch novel https://example.com/novel

# Fetch a specific chapter (includes embedded images)
quelle fetch chapter https://example.com/novel/chapter-1

# Fetch all chapters for a novel
quelle fetch chapters novel-id

# Fetch everything (novel + all chapters + assets)
quelle fetch all https://example.com/novel
```

### Library Management

```bash
# List all novels in your library
quelle library list

# Show details for a specific novel
quelle library show novel-id

# List chapters for a novel
quelle library chapters novel-id

# Read a chapter
quelle library read novel-id chapter-number

# Check for new chapters
quelle library sync novel-id
quelle library sync all

# Update novels with new chapters
quelle library update novel-id
quelle library update all

# Remove a novel from library
quelle library remove novel-id

# Clean up orphaned data
quelle library cleanup

# Show library statistics
quelle library stats
```

### Export

```bash
# Export to EPUB (always available)
quelle export epub novel-id

# Export to PDF (requires pdf feature at compile time)
quelle export pdf novel-id

# Export specific chapters
quelle export epub novel-id --chapters 1-10

# Export to custom directory
quelle export epub novel-id --output ~/Books/

# Export all novels
quelle export epub all

# Other formats (future)
quelle export html novel-id
quelle export txt novel-id
```

### Search

```bash
# Simple search
quelle search "Solo Leveling"

# Filter by author
quelle search "novel title" --author "Author Name"

# Filter by tags
quelle search "fantasy" --tags "isekai,magic"

# Filter by source
quelle search "novel" --source webnovel
```

### Extension Management

```bash
# Install an extension
quelle extension install webnovel-scraper

# List installed extensions
quelle extension list

# Update an extension
quelle extension update webnovel-scraper
quelle extension update all

# Remove an extension
quelle extension remove webnovel-scraper

# Search for extensions
quelle extension search webnovel

# Show extension info
quelle extension info webnovel-scraper
```

### Configuration

```bash
# Set storage location
quelle config set storage.path ~/Documents/Novels

# Set default export format
quelle config set export.format epub

# Enable cover images by default
quelle config set export.include-covers true

# Show current configuration
quelle config show

# Reset to defaults
quelle config reset
```

## Global Options

- `--storage-path <path>` - Override default storage location
- `--config <file>` - Use custom config file
- `--verbose` / `-v` - Enable verbose output
- `--quiet` / `-q` - Suppress most output
- `--dry-run` - Show what would be done without executing

## Examples

### Complete Workflow

```bash
# Install an extension for your favorite source
quelle extension install webnovel-scraper

# Search for a novel
quelle search "Solo Leveling"

# Fetch the novel and all its content
quelle fetch all https://webnovel.com/book/solo-leveling

# Export to EPUB with cover
quelle export epub solo-leveling --output ~/Books/

# Check for updates later
quelle library sync all
quelle library update all
```

### Batch Operations

```bash
# Update all novels and export any that were updated
quelle library update all
quelle export epub all --updated --output ~/Books/

# Export multiple specific novels
quelle export epub novel1 novel2 novel3
```

### Using Dry Run

```bash
# See what would be fetched without actually doing it
quelle --dry-run fetch all https://example.com/novel

# Preview cleanup without making changes
quelle --dry-run library cleanup
```

## Storage

By default, Quelle stores data in `~/.quelle/`:

```
~/.quelle/
├── novels/              # Novel metadata and chapters
├── assets/             # Cover images and chapter assets
├── extensions/         # Installed extensions
└── config.json        # Configuration
```

You can change the storage location with:
- `--storage-path` flag
- `quelle config set storage.path /path/to/storage`
- `QUELLE_STORAGE_PATH` environment variable

## Asset Handling

Quelle automatically handles assets (images) when fetching content:

- **Novel covers** are fetched when you `fetch novel`
- **Chapter images** are fetched when you `fetch chapter`
- **Assets are stored separately** from metadata for efficiency
- **Export includes assets** automatically (e.g., EPUB with embedded images)

## Development Status

🟢 **Ready**: CLI structure, library management, storage integration, extension management (install/list/remove)  
🟡 **In Progress**: Actual content fetching, export functionality, search across extensions  
🔴 **Planned**: Configuration management, extension updates, advanced export options

### What Works Now

- ✅ **Extension Management**: Install, list, remove extensions
- ✅ **Library Management**: List novels, show details, read chapters, cleanup
- ✅ **Storage**: All library operations work with the filesystem backend
- ✅ **Asset Handling**: Infrastructure for automatic asset fetching
- ✅ **CLI Interface**: All commands parse correctly with helpful messages

### What's Coming Next

- 🔄 **Content Fetching**: Integration with extensions to actually fetch novels/chapters
- 🔄 **Export**: EPUB generation with cover images and assets
- 🔄 **Search**: Cross-extension novel search functionality
- 🔄 **Configuration**: Persistent settings management

## Contributing

See the main project README for contribution guidelines.