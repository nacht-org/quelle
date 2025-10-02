# Basic Usage

This guide covers common workflows and commands for using Quelle effectively.

## Core Workflows

### Adding Novels to Your Library

The `add` command is your primary way to build your library:

```bash
# Add a complete novel (fetches all chapters)
quelle add https://dragontea.ink/novel/example

# Add only metadata (no chapters)
quelle add https://example.com/novel --no-chapters

# Add with chapter limit (useful for testing large novels)
quelle add https://example.com/novel --max-chapters 10
```

**What happens when you add a novel:**
1. Quelle identifies the appropriate extension
2. Fetches novel metadata (title, author, description, cover)
3. Downloads all available chapters (unless `--no-chapters`)
4. Stores everything in your local library

### Managing Your Library

**View your library:**
```bash
# List all novels
quelle library list

# Show detailed information for a specific novel
quelle library show "Novel Title"

# View chapters for a novel
quelle library chapters "Novel Title"

# Show library statistics
quelle library stats
```

**Update with new content:**
```bash
# Update all novels with new chapters
quelle update

# Update specific novel
quelle update "Novel Title"

# Check for updates without downloading
quelle update --check-only
```

**Remove content:**
```bash
# Remove a novel (prompts for confirmation)
quelle remove "Novel Title"

# Remove without confirmation
quelle remove "Novel Title" --force
```

### Reading Content

**Read chapters:**
```bash
# List available chapters
quelle read "Novel Title" --list

# Read a specific chapter
quelle read "Novel Title" 1
quelle read "Novel Title" "Chapter Title"

# Read using novel ID (shown in library list)
quelle read novel-id-123 5
```

### Searching for Novels

**Basic search:**
```bash
# Simple text search
quelle search "cultivation"
quelle search "magic academy"

# Search with author filter
quelle search "reincarnation" --author "AuthorName"
```

**Advanced search:**
```bash
# Search with multiple filters
quelle search "fantasy" --tags "magic,adventure" --limit 10

# Filter by categories
quelle search "novel" --categories "fantasy,action"
```

## Extension Management

### Working with Extensions

**List extensions:**
```bash
# Show installed extensions
quelle extensions list

# Show detailed extension information
quelle extensions info dragontea
```

**Search for new extensions:**
```bash
# Search available extensions
quelle extensions search "webnovel"

# Install an extension
quelle extensions install new-extension-id

# Install specific version
quelle extensions install dragontea --version 1.2.0
```

**Update extensions:**
```bash
# Update all extensions
quelle extensions update all

# Update specific extension
quelle extensions update dragontea

# Force update even if no new version
quelle extensions update dragontea --force
```

## Advanced Operations

### Export Content

**Export to different formats:**
```bash
# Export to EPUB (default format)
quelle export "Novel Title"

# Export to PDF
quelle export "Novel Title" --format pdf

# Export to specific directory
quelle export "Novel Title" --output /path/to/exports

# Export with images included
quelle export "Novel Title" --include-images

# Export all novels
quelle export all
```

### Fetch Operations (Advanced)

For debugging or testing individual operations:

```bash
# Fetch only novel metadata
quelle fetch novel https://example.com/novel

# Fetch specific chapter
quelle fetch chapter https://example.com/novel/chapter-1

# Fetch all chapters for existing novel
quelle fetch chapters novel-id-123
```

### Configuration Management

**View and modify settings:**
```bash
# Show all configuration
quelle config show

# Set specific values
quelle config set export.format epub
quelle config set data_dir /custom/path

# Get specific value
quelle config get export.format

# Reset to defaults
quelle config reset --force
```

## Working with Multiple Sources

### Supported Sites

Currently supported extensions:
- **DragonTea** (`dragontea`): Dragons Tea novels
- **ScribbleHub** (`scribblehub`): ScribbleHub stories

### URL Patterns

Each extension supports specific URL patterns:

**DragonTea:**
- Novel: `https://dragontea.ink/novel/novel-name`
- Chapter: `https://dragontea.ink/novel/novel-name/chapter-n`

**ScribbleHub:**
- Novel: `https://scribblehub.com/series/12345/novel-name`
- Chapter: `https://scribblehub.com/read/12345-novel-name/chapter/67890`

## Tips and Best Practices

### Performance Tips

1. **Use chapter limits for testing:**
   ```bash
   quelle add https://example.com/novel --max-chapters 5
   ```

2. **Update regularly but efficiently:**
   ```bash
   # Check first, then update
   quelle update --check-only
   quelle update
   ```

3. **Use dry-run for testing:**
   ```bash
   quelle --dry-run add https://example.com/novel
   ```

### Organization Tips

1. **Use meaningful library organization:**
   - Let Quelle handle file organization automatically
   - Use library commands to browse content

2. **Regular maintenance:**
   ```bash
   # Clean up orphaned data
   quelle library cleanup
   
   # Check library health
   quelle status
   ```

### Troubleshooting Tips

1. **Use verbose output for debugging:**
   ```bash
   quelle --verbose add https://example.com/novel
   ```

2. **Check system status:**
   ```bash
   quelle status
   ```

3. **Verify extensions work:**
   ```bash
   quelle extensions list
   quelle fetch novel https://example.com/test-novel
   ```

## Common Issues and Solutions

### Novel Not Found
**Problem:** Extension can't find the novel at URL
**Solutions:**
- Verify the URL is correct and accessible in browser
- Check if the extension supports that specific site format
- Try fetching just metadata first: `quelle fetch novel <url>`

### Network Timeouts
**Problem:** Downloads fail due to network issues
**Solutions:**
- Retry the operation (Quelle will resume where it left off)
- Check internet connection
- Some sites may rate-limit requests

### Missing Chapters
**Problem:** Not all chapters were downloaded
**Solutions:**
- Run `quelle update "Novel Title"` to fetch missing chapters
- Check if chapters are actually published on the site
- Some sites may have access restrictions

### Storage Issues
**Problem:** Running out of disk space or permission errors
**Solutions:**
- Check available disk space: `quelle library stats`
- Verify write permissions to storage directory
- Change storage location: `quelle config set data_dir /new/path`

## Next Steps

- **For users:** Explore [CLI Commands](../reference/cli-commands.md) for complete command reference
- **For developers:** Learn [Extension Development](../development/extension-development.md) to add new sources
- **For troubleshooting:** See [Troubleshooting Guide](../reference/troubleshooting.md)

Remember: Quelle is in active development, so features and workflows may evolve. Check the documentation regularly for updates!