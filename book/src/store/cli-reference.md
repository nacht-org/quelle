# CLI Reference

This reference covers all available commands in Quelle's CLI. Commands are organized by functionality.

## Global Options

```bash
quelle --help     # Show help for all commands
quelle --version  # Show version information (when implemented)
```

## Main Commands

### Status and Information

```bash
# Show overall system status
quelle status

# List available extensions across all stores
quelle list
```

### Fetching Content

```bash
# Fetch novel information from URL
quelle fetch novel <URL>

# Fetch chapter content from URL  
quelle fetch chapter <URL>

# Examples:
quelle fetch novel https://dragontea.ink/novel/example
quelle fetch chapter https://scribblehub.com/read/123456/chapter/1/
```

### Search

```bash
# Simple search
quelle search "<query>"

# Search with filters
quelle search "<query>" --author "<author_name>"
quelle search "<query>" --tags "<tag1>,<tag2>"
quelle search "<query>" --categories "<cat1>,<cat2>"
quelle search "<query>" --limit <number>

# Examples:
quelle search "cultivation"
quelle search "romance" --author "Great Author"
quelle search "fantasy" --tags "magic,adventure" --limit 10
```

## Store Management

### Store Commands

```bash
# Add a local store
quelle store add local <path> --name "<name>"
quelle store add local <path>  # Auto-generated name

# List all configured stores
quelle store list

# Remove a store by name
quelle store remove <store_name>

# Check health of all stores
quelle store health

# Search across all stores
quelle store search "<query>" [options]

# List extensions in all stores
quelle store list-extensions

# Show publishing requirements
quelle store requirements [--store <store_name>]
```

### Store Search Options

```bash
quelle store search "<query>" \
  --author "<author>" \
  --tags "<tag1>,<tag2>" \
  --sort <option> \
  --limit <number>

# Sort options: relevance (default), name, date
```

### Store Examples

```bash
# Add local directories as stores
quelle store add local ./my-extensions --name "personal"
quelle store add local /home/user/extensions --name "main"

# Check store status
quelle store list
quelle store health

# Search for extensions
quelle store search "dragontea"
quelle store list-extensions

# Remove a store
quelle store remove personal
```

## Extension Management

### Extension Commands

```bash
# Install an extension
quelle extension install <extension_id>
quelle extension install <extension_id> --version <version>
quelle extension install <extension_id> --force

# Update extensions
quelle extension update <extension_id>
quelle extension update all
quelle extension update <extension_id> --prerelease --force

# List installed extensions
quelle extension list

# Get extension information
quelle extension info <extension_id>

# Uninstall an extension
quelle extension uninstall <extension_id>
quelle extension uninstall <extension_id> --remove-data

# Check for updates
quelle extension check-updates

# Publish extension to store (development)
quelle extension publish <wasm_file> --store <store_name>
quelle extension publish <wasm_file> --store <store_name> --overwrite
```

### Extension Examples

```bash
# Install extensions
quelle extension install dragontea
quelle extension install scribblehub --force

# Manage extensions
quelle extension list
quelle extension info dragontea
quelle extension update all

# Development workflow
quelle extension publish ./extension.wasm --store local --overwrite
```

## Command Examples

### Basic Workflow

```bash
# 1. Check current status
quelle status

# 2. Set up a store
quelle store add local ./my-extensions --name "dev"

# 3. Check store health
quelle store health

# 4. Look for available extensions
quelle list

# 5. Try fetching from a URL
quelle fetch novel https://example.com/novel-url
```

### Daily Usage

```bash
# Search for novels
quelle search "cultivation fantasy"

# Get novel information
quelle fetch novel https://dragontea.ink/novel/some-story

# Read a chapter
quelle fetch chapter https://dragontea.ink/novel/some-story/chapter-1

# Check for extension updates
quelle extension check-updates
```

### Development Workflow

```bash
# Build extension
just build-extension dragontea

# Set up development store
quelle store add local ./dev-extensions --name "dev"

# Copy extension to store
cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./dev-extensions/

# Test extension
quelle fetch novel https://dragontea.ink/test-url

# Publish to store (alternative)
quelle extension publish ./extension.wasm --store dev --overwrite
```

## Output Examples

### Successful Novel Fetch
```text
Found extension with ID: dragontea
📖 Fetching novel info from: https://dragontea.ink/novel/example
✅ Successfully fetched novel information:
  Title: Example Novel
  Authors: Author Name
  Description: A story about...
  Cover URL: https://example.com/cover.jpg
  Total chapters: 150
  Status: Ongoing
```

### Store List Output
```text
Available extension stores:
  📦 dev (local)
     - dragontea
     - scribblehub
  📦 personal (local)
     - custom_extension
```

### Extension List Output
```text
Installed extensions:
  ✅ dragontea v1.0.0 (dev store)
  ✅ scribblehub v1.0.0 (dev store)
```

### Search Results
```text
🔍 Using simple search...
Found 3 results:
1. Cultivation Master by Great Author
   A young cultivator begins his journey...
   Store: dev
2. Magic Academy by Another Author  
   Students learn magic at the academy...
   Store: personal
```

## Error Handling

### Common Error Messages

**No extension found:**
```text
❌ No extension found that can handle URL: https://example.com/novel
Try adding more extension stores with: quelle store add
```

**Store not accessible:**
```text
❌ Store 'personal' is not accessible
Error: Directory not found: ./my-extensions
```

**Extension installation failed:**
```text
❌ Failed to install dragontea: Extension not found in any store
```

### Exit Codes

- `0`: Success
- `1`: General error
- `2`: Configuration error
- `3`: Network error
- `4`: Extension error

## Configuration Files

### Store Configuration
Location: `./data/config.json`

```json
{
  "stores": [
    {
      "name": "dev",
      "store_type": "local", 
      "path": "./my-extensions"
    }
  ]
}
```

### Extension Registry  
Location: `./data/registry/`

Contains installed extension metadata and WASM files.

## Environment Variables

Currently, Quelle uses these working directories:
- `./data/` - Configuration and registry data
- `./target/` - Build artifacts (when building from source)

Future releases may support environment variables for custom locations.

## Tips

1. **Use `--help`**: Add `--help` to any command to see detailed options
2. **Tab completion**: May be available depending on your shell setup  
3. **Relative paths**: Store paths can be relative to current directory
4. **Auto-detection**: Extensions are automatically selected based on URL
5. **Manual setup**: For now, manually copying WASM files is often easier than automatic installation

## Limitations

### Current Implementation
- Only local stores supported
- Limited extension metadata
- Manual extension management often required
- No automatic dependency resolution
- Basic error messages

### Future Features
- Git and HTTP store support
- Enhanced search and filtering
- Automatic extension updates
- Better error reporting
- Extension dependency management
- Configuration profiles

This CLI interface is evolving rapidly. Check `quelle --help` for the most current information, and refer to other sections of this book for detailed usage guides.