# Basic Usage

This guide covers the main commands you'll use with Quelle. Remember, Quelle is still in early development, so some features are limited.

## Prerequisites

Before using these commands, make sure you've:
1. Built Quelle from source (see [Getting Started](./getting-started.md))
2. Built at least one extension with `just build-extension <name>`
3. Set up a local store

## Main Commands

### Check Status

See what's currently configured:

```bash
# Overall status
quelle status

# List configured stores
quelle store list

# List installed extensions
quelle extension list

# Check if stores are working
quelle store health
```

### Fetch Novel Information

Get basic info about a novel from its URL:

```bash
# Fetch novel info (DragonTea example)
quelle fetch novel https://dragontea.ink/novel/some-novel

# Fetch novel info (ScribbleHub example) 
quelle fetch novel https://scribblehub.com/series/123456/novel-title/
```

This shows:
- Novel title
- Author(s)
- Description
- Cover image URL
- Total chapters
- Publication status

### Fetch Chapter Content

Get the text content of a specific chapter:

```bash
# Fetch chapter content
quelle fetch chapter https://dragontea.ink/novel/some-novel/chapter-1

# ScribbleHub chapter
quelle fetch chapter https://scribblehub.com/read/123456-novel-title/chapter/1/
```

This shows:
- Chapter content length
- Preview of the text content

### Search for Novels

Search across available extensions:

```bash
# Simple search
quelle search "cultivation"

# Search with author filter
quelle search "romance" --author "Author Name"

# Search with tags (if supported)
quelle search "fantasy" --tags "magic,adventure"

# Limit results
quelle search "novel" --limit 5
```

### List Available Extensions

See what extensions are available in your stores:

```bash
# List extensions in all stores
quelle list

# This shows each store and its extensions
```

## Store Management

### Add a Store

```bash
# Add a local directory as a store
quelle store add local ./path/to/extensions --name "my-store"

# Add with auto-generated name
quelle store add local ./extensions
```

### Manage Stores

```bash
# List all stores
quelle store list

# Remove a store
quelle store remove my-store

# Check store health
quelle store health

# Search across all stores
quelle store search "novel title"

# List extensions in all stores  
quelle store list-extensions
```

## Extension Management

### Install Extensions

```bash
# Install an extension (if available in a store)
quelle extension install dragontea

# Install specific version
quelle extension install dragontea --version 1.0.0

# Force reinstall
quelle extension install dragontea --force
```

### Manage Extensions

```bash
# List installed extensions
quelle extension list

# Get extension info
quelle extension info dragontea

# Update an extension
quelle extension update dragontea

# Update all extensions
quelle extension update all

# Uninstall an extension
quelle extension uninstall dragontea
```

### Publish Extensions (Development)

If you're developing extensions:

```bash
# Publish to a local store
quelle extension publish ./extension.wasm --store local

# Publish with overwrite
quelle extension publish ./extension.wasm --store local --overwrite
```

## Common Workflows

### Daily Usage

```bash
# 1. Check status
quelle status

# 2. Search for something new
quelle search "new novel"

# 3. Get novel info from a URL you found
quelle fetch novel https://example.com/novel

# 4. Read a chapter
quelle fetch chapter https://example.com/novel/chapter-1
```

### Setting Up New Extensions

```bash
# 1. Build the extension
just build-extension new-site

# 2. Add to your local store (manual file copy for now)
cp target/wasm32-unknown-unknown/release/extension_new_site.wasm ./my-extensions/

# 3. Test it
quelle fetch novel https://new-site.com/novel-url
```

### Managing Multiple Extensions

```bash
# Check what you have
quelle extension list

# Update everything
quelle extension update all

# Check for issues
quelle store health
```

## Output Examples

### Novel Fetch Success
```text
Found extension with ID: dragontea
📖 Fetching novel info from: https://dragontea.ink/novel/example
✅ Successfully fetched novel information:
  Title: Example Novel
  Authors: Author Name
  Description: A great story about...
  Cover URL: https://example.com/cover.jpg
  Total chapters: 150
  Status: Ongoing
```

### Chapter Fetch Success
```text
Found extension with ID: dragontea
📄 Fetching chapter from: https://dragontea.ink/novel/example/chapter-1
✅ Successfully fetched chapter:
  Content length: 2847 characters
  Preview: Chapter 1: The Beginning...
```

### Search Results
```text
🔍 Using simple search...
Found 3 results:
1. Cultivation Master by Great Author
   A young cultivator begins his journey...
   Store: my-store
2. Magic Academy by Another Author
   Students learn magic in this academy...
   Store: my-store
```

## Current Limitations

### What Works
- ✅ Fetching novel info from URLs
- ✅ Fetching chapter content
- ✅ Basic search (if extension supports it)
- ✅ Store and extension management
- ✅ Two working extensions (DragonTea, ScribbleHub)

### What's Missing
- ❌ Downloading complete novels
- ❌ Exporting to EPUB/PDF
- ❌ Batch operations
- ❌ Resume interrupted downloads
- ❌ Advanced search across multiple sites
- ❌ Built-in extension repository

## Tips

1. **Always check status first** with `quelle status` to see what's configured
2. **URLs matter** - make sure you're using URLs that your extensions support
3. **Extensions auto-install** - if you fetch from a URL and the extension exists in a store, it installs automatically
4. **Local stores are simple** - just directories with WASM files for now
5. **Build before use** - remember to build extensions with `just build-extension <name>`

## Troubleshooting

### "No extension found for URL"
- Make sure you have extensions built and available in stores
- Check that the URL matches what the extension supports
- Run `quelle list` to see available extensions

### Extension errors
- Check that the extension was built successfully
- Verify the website URL is correct and accessible
- Some sites may block automated access

### Store issues
- Run `quelle store health` to check connectivity
- Make sure store directories exist and have proper permissions
- Check `quelle store list` to see configured stores

For more help, check the other sections of this book or look at the source code in the project repository.