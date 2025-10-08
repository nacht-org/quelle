# Getting Started

This guide walks you through your first steps with Quelle after installation. By the end, you'll have added your first novel and understand the basic workflows.

## Prerequisites

- Quelle installed and built (see [Installation](./installation.md))
- At least one extension installed (ScribbleHub recommended for testing)
- Internet connection for downloading novels

## Quick Start

### 1. Verify Your Installation

First, make sure everything is working correctly:

```bash
# Check system status
quelle status

# List installed extensions
quelle extensions list
```

You should see something like:
```
✅ Quelle is ready
📦 Extensions: 1 installed
🏪 Stores: 1 configured (local)
```

If you see any errors, return to the [Installation](./installation.md) guide.

### 2. Your First Search

Let's search for novels to get familiar with the interface:

```bash
# Search across all available sources
quelle search "cultivation" --limit 5

# Search with more specific terms
quelle search "fantasy adventure" --limit 3
```

This shows you available novels from all configured extensions. You'll see results with titles, authors, and URLs.

### 3. Add Your First Novel

Pick a novel from your search results or use a direct URL:

```bash
# Add a novel (example with ScribbleHub URL)
quelle add "https://www.scribblehub.com/series/123456/example-novel/"

# This will:
# - Fetch novel metadata (title, author, description, tags)
# - Download all available chapters
# - Add it to your library
```

**What happens during download:**
- Novel metadata is extracted and stored
- All chapters are downloaded in order
- Progress is displayed as chapters are fetched
- The novel becomes available in your library

### 4. Explore Your Library

Now that you have a novel, explore your library:

```bash
# List all novels in your library
quelle library list

# Show detailed information for a specific novel
quelle library show "Novel Title"

# List chapters for a novel
quelle read "Novel Title" --list
```

### 5. Read Your First Chapter

Read content directly in your terminal:

```bash
# Read the first chapter
quelle read "Novel Title" 1

# Read by chapter title
quelle read "Novel Title" "Prologue"
```

The chapter content will be displayed in a clean, readable format in your terminal.

## Essential Workflows

### Managing Your Collection

**Adding novels:**
```bash
# Add with all chapters (default)
quelle add https://example.com/novel

# Add only metadata, download chapters later
quelle add https://example.com/novel --no-chapters

# Add with a chapter limit (useful for testing large novels)
quelle add https://example.com/novel --max-chapters 10
```

**Keeping up to date:**
```bash
# Update all novels with new chapters
quelle update

# Update a specific novel
quelle update "Novel Title"

# Check for updates without downloading
quelle update --check-only
```

**Library maintenance:**
```bash
# View library statistics
quelle library stats

# Remove a novel you no longer want
quelle remove "Novel Title"

# Clean up any orphaned data
quelle library cleanup
```

### Discovery and Search

**Basic search:**
```bash
# Simple keyword search
quelle search "magic academy"

# Search with filters
quelle search "isekai" --limit 10
```

**Advanced search (when supported by extensions):**
```bash
# Search by author
quelle search "reincarnation" --author "AuthorName"

# Search with tags
quelle search "fantasy" --tags "magic,adventure"
```

### Reading and Export

**Reading options:**
```bash
# List all chapters
quelle read "Novel Title" --list

# Read specific chapters
quelle read "Novel Title" 1      # Chapter by number
quelle read "Novel Title" "Chapter One"  # By title
```

**Export for external reading:**
```bash
# Export to EPUB (default)
quelle export "Novel Title"

# Export to PDF
quelle export "Novel Title" --format pdf

# Export to a specific directory
quelle export "Novel Title" --output ./my-books/

# Export all novels
quelle export all
```

## Working with Extensions

### Managing Extensions

Extensions are what make Quelle work - each one handles a different website:

```bash
# List installed extensions
quelle extensions list

# Get detailed info about an extension
quelle extensions info scribblehub

# Search for available extensions (if connected to registry)
quelle extensions search "royal"
```

### Installing New Extensions

If you have access to the official registry or custom stores:

```bash
# Install from official registry
quelle extensions install en.royalroad

# Update all extensions
quelle extensions update all

# Remove an extension you no longer need
quelle extensions remove dragontea
```

## Configuration and Customization

### Basic Configuration

```bash
# View current configuration
quelle config show

# Set default export format
quelle config set export.format epub

# Change storage location
quelle config set data_dir /path/to/quelle/data
```

### Store Management

Stores are repositories where extensions are distributed:

```bash
# List configured stores
quelle store list

# Add a Git-based store
quelle store add git upstream https://github.com/user/extensions.git

# Update store data
quelle store update local
```

## Tips for New Users

### Best Practices

1. **Start small**: Add 1-2 novels initially to test the system
2. **Use chapter limits**: For large novels (500+ chapters), consider `--max-chapters 50` initially
3. **Regular updates**: Run `quelle update` weekly to get new chapters
4. **Export regularly**: Export novels to EPUB for backup and offline reading

### Common Workflows

**Daily usage:**
```bash
quelle update                    # Check for new chapters
quelle read "Current Novel" 42   # Continue reading
quelle search "new genre"        # Discover new content
```

**Weekly maintenance:**
```bash
quelle library stats             # Check library health
quelle update --check-only       # See what has updates
quelle library cleanup           # Clean up any issues
```

**Adding new sources:**
```bash
quelle search "site name"        # Look for extensions
quelle extensions search "site"  # Search available extensions
```

### Understanding Output

When adding novels, you'll see output like:
```
✅ Novel: "Example Novel" by Author Name
📝 Description: A fantastic story about...
📊 Chapters: Found 245 chapters
⬇️  Downloading chapters... [42/245]
✅ Added successfully!
```

When updating:
```
📚 Checking "Novel Title"...
🆕 Found 3 new chapters
⬇️  Downloading chapters 246-248...
✅ Updated successfully!
```

## Troubleshooting Common Issues

### "Extension not found"
**Problem**: The URL isn't supported by any installed extension  
**Solution**: Check `quelle extensions list` and install the appropriate extension, or verify the URL is correct

### "No chapters found"
**Problem**: Extension found the novel but no chapters  
**Solution**: The novel might not have published chapters yet, or the website structure may have changed

### Network timeouts
**Problem**: Downloads fail due to connection issues  
**Solution**: Retry the operation - Quelle resumes where it left off. Some sites may rate-limit requests.

### Storage errors
**Problem**: Permission denied or disk full errors  
**Solution**: Check disk space with `df -h` (Linux/macOS) and ensure the storage directory is writable

### Getting More Help

- **Verbose output**: Add `-v` to any command for detailed logging
- **System check**: Use `quelle status` to diagnose issues
- **Command help**: Add `--help` to any command for usage information
- **Dry run**: Add `--dry-run` to preview actions without executing them

## What's Next?

Now that you're comfortable with the basics:

1. **Power User Guide**: [Basic Usage](./basic-usage.md) covers advanced features and workflows
2. **CLI Reference**: [CLI Commands](./reference/cli-commands.md) provides complete command documentation  
3. **Extension Development**: [Extension Development](./development/extension-development.md) if you want to add support for new sites
4. **Troubleshooting**: [Troubleshooting Guide](./reference/troubleshooting.md) for detailed problem-solving

## Example: Complete First Session

Here's a complete example of a first session with Quelle:

```bash
# 1. Verify installation
quelle status

# 2. Search for something interesting
quelle search "progression fantasy" --limit 5

# 3. Add a novel from the results
quelle add "https://www.scribblehub.com/series/123456/example-novel/"

# 4. Check your library
quelle library list

# 5. Read the first chapter
quelle read "Example Novel" 1

# 6. Export for your e-reader
quelle export "Example Novel" --format epub

# 7. Set up regular updates
quelle update
```

Welcome to Quelle! You're now ready to build and manage your digital novel library.