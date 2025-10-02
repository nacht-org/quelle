# Getting Started

This guide walks you through your first steps with Quelle after installation.

## Prerequisites

- Quelle installed and built (see [Installation](./installation.md))
- Command line terminal
- Internet connection

## First Steps

### 1. Check Installation

Verify Quelle is working:

```bash
quelle --help
```

You should see the main help menu with available commands.

### 2. Check Available Extensions

See what extensions are installed:

```bash
quelle extensions list
```

By default, you should have:
- `dragontea` - For DragonTea novels
- `scribblehub` - For ScribbleHub stories

### 3. Add Your First Novel

Let's add a novel to your library. You need a URL from a supported site:

```bash
# Example with a DragonTea URL
quelle add https://dragontea.ink/novel/example-novel

# Example with a ScribbleHub URL  
quelle add https://scribblehub.com/series/12345/example-story
```

This will:
- Fetch novel metadata (title, author, description)
- Download all available chapters
- Store everything in your local library

### 4. View Your Library

See what novels you have:

```bash
quelle library list
```

This shows all novels in your library with their status.

### 5. Read a Chapter

Read from your library:

```bash
# List chapters for a novel
quelle read "Novel Title" --list

# Read a specific chapter
quelle read "Novel Title" 1
```

## Common Workflows

### Adding Novels

```bash
# Add novel with all chapters
quelle add https://example.com/novel

# Add only metadata, no chapters
quelle add https://example.com/novel --no-chapters

# Add with chapter limit (useful for testing)
quelle add https://example.com/novel --max-chapters 5
```

### Managing Your Library

```bash
# Check for new chapters
quelle update

# Update specific novel
quelle update "Novel Title"

# Show library statistics
quelle library stats

# Remove a novel
quelle remove "Novel Title"
```

### Searching

```bash
# Search for novels
quelle search "cultivation"

# Search with filters
quelle search "magic" --author "AuthorName"
```

## Configuration

### Storage Location

By default, Quelle stores data in:
- **Linux/macOS**: `~/.local/share/quelle/`
- **Windows**: `%APPDATA%\quelle\`

You can override this:

```bash
# Use custom storage location
quelle --storage-path /path/to/storage library list
```

### Basic Settings

View current configuration:

```bash
quelle config show
```

Set configuration values:

```bash
# Set default export format
quelle config set export.format epub

# Set storage directory
quelle config set data_dir /path/to/quelle/data
```

## Getting Help

### Command Help

Get help for any command:

```bash
quelle add --help
quelle library --help
quelle search --help
```

### Verbose Output

See what Quelle is doing:

```bash
quelle --verbose add https://example.com/novel
```

### Dry Run

Test commands without making changes:

```bash
quelle --dry-run add https://example.com/novel
```

## Troubleshooting

### Common Issues

1. **"Extension not found"**
   - Check `quelle extensions list`
   - The URL might not be supported yet

2. **"Network timeout"**
   - Check internet connection
   - Some sites may be slow or blocking requests

3. **"No chapters found"**
   - The novel might not have published chapters
   - Try adding with `--no-chapters` first

### Getting More Help

- Use `quelle status` to check system health
- Check [Troubleshooting](../reference/troubleshooting.md) for detailed solutions
- Look at verbose output with `--verbose` flag

## Next Steps

Once you're comfortable with the basics:

1. [Basic Usage](./basic-usage.md) - Learn more advanced workflows
2. [CLI Commands](../reference/cli-commands.md) - Complete command reference
3. [Extension Development](../development/extension-development.md) - Create your own scrapers

## Tips

- Start with small novels to test functionality
- Use `--dry-run` to preview actions
- Check `quelle status` if something seems wrong
- Extensions are sandboxed - they can't harm your system