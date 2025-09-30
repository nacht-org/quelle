# Git Store CLI Demo

This document demonstrates the new git store management functionality in the Quelle CLI. With git stores, you can now add extension repositories hosted on Git platforms like GitHub, GitLab, Gitea, etc.

## Overview

Git stores allow you to:
- Add remote git repositories as extension stores
- Use different authentication methods (tokens, SSH keys, username/password)
- Track specific branches, tags, or commits
- Automatically sync with remote changes
- Publish extensions to git-backed stores

## CLI Structure

The Quelle CLI uses subcommands for different store types to provide a clean and intuitive interface:

```bash
# General syntax
quelle store add <STORE_TYPE> <NAME> <LOCATION> [OPTIONS]

# Store types available
quelle store add local <name> [path] [--priority N]
quelle store add git <name> <url> [--branch|--tag|--commit] [--auth-options] [--priority N]
```

This structure makes it clear which options are available for each store type and prevents confusion between local and git-specific parameters.

## Basic Usage

### Adding Local Stores

```bash
# Add with default location (data_dir/stores/name)
quelle store add local my-local-store

# Add with custom path
quelle store add local my-local-store /path/to/extensions

# Add with priority
quelle store add local priority-local /path/to/ext --priority 50

# Add with default location and priority
quelle store add local high-priority --priority 25
```

### Adding Public Git Stores

```bash
# Add a public GitHub repository
quelle store add git my-extensions https://github.com/user/quelle-extensions.git

# Add with specific branch
quelle store add git dev-extensions https://github.com/user/quelle-extensions.git --branch develop

# Add with specific tag
quelle store add git stable-extensions https://github.com/user/quelle-extensions.git --tag v1.0.0

# Add with specific commit
quelle store add git pinned-extensions https://github.com/user/quelle-extensions.git --commit abc123def456
```

### Adding Private Git Stores

#### Using Personal Access Token (GitHub/GitLab)
```bash
quelle store add git private-extensions https://github.com/user/private-extensions.git \
  --token ghp_your_token_here
```

#### Using SSH Key Authentication
```bash
quelle store add git ssh-extensions git@github.com:user/extensions.git \
  --ssh-key ~/.ssh/id_rsa \
  --ssh-pub-key ~/.ssh/id_rsa.pub
```

#### Using SSH Key with Passphrase
```bash
quelle store add git secure-extensions git@gitlab.com:user/extensions.git \
  --ssh-key ~/.ssh/id_ed25519 \
  --ssh-passphrase "your-passphrase"
```

#### Using Username and Password
```bash
quelle store add git basic-auth-extensions https://git.example.com/user/extensions.git \
  --username your-username \
  --password your-password
```

### Custom Cache Directory

By default, git repositories are cached in `~/.local/share/quelle/stores/<store-name>`. You can specify a custom location:

```bash
quelle store add git custom-extensions https://github.com/user/extensions.git \
  --cache-dir /path/to/custom/cache
```

### Setting Priority

Control the order in which stores are searched for extensions:

```bash
# Higher priority (lower number = searched first)
quelle store add git priority-extensions https://github.com/user/extensions.git \
  --priority 50

# Lower priority (higher number = searched later)
quelle store add git fallback-extensions https://github.com/user/backup-extensions.git \
  --priority 200

# Local store with high priority
quelle store add local high-priority-local --priority 10
```

## Management Commands

### List All Stores

```bash
quelle store list
```

Example output:
```
📦 Configured extension stores (4):
  📍 high-priority-local (priority: 10)
     Type: Local { path: "/home/user/.local/share/quelle/stores/high-priority-local" }
     Path: /home/user/.local/share/quelle/stores/high-priority-local
     Status: ✅ Enabled

  📍 github-extensions (priority: 50)
     Type: Git { url: "https://github.com/user/extensions.git", ... }
     URL: https://github.com/user/extensions.git
     Cache Dir: /home/user/.local/share/quelle/stores/github-extensions
     Reference: Default
     Auth: Token
     Status: ✅ Enabled

  📍 local-extensions (priority: 100)
     Type: Local { path: "/home/user/my-extensions" }
     Path: /home/user/my-extensions
     Status: ✅ Enabled

  📍 gitlab-extensions (priority: 100)
     Type: Git { url: "git@gitlab.com:user/extensions.git", ... }
     URL: git@gitlab.com:user/extensions.git
     Cache Dir: /home/user/.local/share/quelle/stores/gitlab-extensions
     Reference: Branch("main")
     Auth: SSH Key
     Status: ✅ Enabled
```

### Get Detailed Store Information

```bash
quelle store info github-extensions
```

Example output:
```
📍 Store: github-extensions
Type: Git { url: "https://github.com/user/extensions.git", ... }
Priority: 50
Enabled: true
Trusted: false
Added: 2025-09-30 14:00:00 UTC
URL: https://github.com/user/extensions.git
Cache Dir: /home/user/.local/share/quelle/stores/github-extensions
Cache Exists: true
Reference: Default
Auth: Token authentication

Runtime Information:
Status: ✅ Healthy
Extensions: 15
Last checked: 2025-09-30 14:10:08 UTC
Sample Extensions:
  - webnovel-scraper v1.2.0 by user - Scrapes web novels from various sites
  - royalroad v0.8.1 by user - Royal Road specific scraper
  - wattpad v1.0.0 by user - Wattpad novel scraper
  ... and 12 more
```

### Update Store Data

```bash
# Update a specific store (fetches latest changes)
quelle store update github-extensions

# Update all stores
quelle store update all
```

### Remove a Store

```bash
# Remove with confirmation
quelle store remove github-extensions

# Force remove without confirmation
quelle store remove github-extensions --force
```

## Complete Examples

### Example 1: Company Internal Extensions

```bash
# Add company's internal extension repository with SSH key
quelle store add git company-extensions git@git.company.com:tools/quelle-extensions.git \
  --ssh-key ~/.ssh/company_rsa \
  --priority 10 \
  --branch production
```

### Example 2: Community Extensions with Token

```bash
# Add community extensions from GitHub with personal access token
quelle store add git community-extensions https://github.com/quelle-community/extensions.git \
  --token ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx \
  --priority 75
```

### Example 3: Development Setup

```bash
# Add development branch for testing new extensions
quelle store add git dev-extensions https://github.com/user/quelle-extensions.git \
  --branch develop \
  --priority 150 \
  --cache-dir ~/.quelle/dev-cache

# Add local development store with high priority
quelle store add local dev-local ~/.local/dev-extensions --priority 5

# Add default local store for personal extensions
quelle store add local personal --priority 25
```

### Example 4: Multi-Environment Setup

```bash
# Production stores (highest priority)
quelle store add local prod-local /opt/quelle/prod-extensions --priority 10
quelle store add git prod-git https://github.com/company/prod-extensions.git --priority 15 --tag stable

# Staging stores
quelle store add local staging-local /opt/quelle/staging-extensions --priority 50
quelle store add git staging-git https://github.com/company/extensions.git --priority 55 --branch staging

# Development stores (lowest priority)
quelle store add local dev-local --priority 100
quelle store add git dev-git https://github.com/company/extensions.git --priority 105 --branch develop
```

## Authentication Setup

### GitHub Personal Access Token

1. Go to GitHub Settings > Developer settings > Personal access tokens
2. Generate a new token with `repo` scope for private repos
3. Use the token with `--token` flag

### GitLab Personal Access Token

1. Go to GitLab User Settings > Access Tokens
2. Create token with `read_repository` scope
3. Use the token with `--token` flag

### SSH Key Setup

1. Generate SSH key: `ssh-keygen -t ed25519 -C "your_email@example.com"`
2. Add public key to your Git platform
3. Use private key path with `--ssh-key` flag

## Best Practices

### Security
- Use SSH keys when possible for better security
- Store tokens securely, don't hardcode them in scripts
- Use read-only tokens/keys when you only need to fetch extensions
- Consider using separate keys for different purposes

### Organization
- Use descriptive store names that indicate their purpose
- Set appropriate priorities based on trust and update frequency
- Use specific branches/tags for production environments
- Use development branches for testing new extensions

### Local Store Management
- Use default paths (`quelle store add local <name>`) for simple setups
- Specify custom paths when you need to share stores or use existing directories
- Use priorities to control search order (lower numbers = higher priority)
- Consider using separate local stores for different types of extensions

### Performance
- Lower priority numbers for frequently used stores
- Consider using specific tags for stable releases
- Use shallow clones (default) for faster syncing
- Clean up unused stores to save disk space

## Troubleshooting

### Authentication Issues
```bash
# Check store status
quelle store info problematic-store

# Common issues:
# - Expired tokens: regenerate and update store
# - SSH key not added to platform
# - Wrong repository URL or permissions
```

### Network Issues
```bash
# Retry updating store
quelle store update problematic-store

# Check cache directory exists and is writable
ls -la ~/.local/share/quelle/stores/
```

### Store Not Loading
```bash
# Check for manifest file
ls ~/.local/share/quelle/stores/store-name/store.json

# If missing, the repository might not be a valid extension store
# Remove and re-add with correct repository
quelle store remove invalid-store --force
```

### Local Store Issues
```bash
# Check if default directory was created
ls -la ~/.local/share/quelle/stores/

# Check permissions
ls -ld ~/.local/share/quelle/stores/store-name

# Reinitialize if needed
quelle store remove broken-local --force
quelle store add local fixed-local /path/to/directory
```

## Integration with Publishing

Git stores also support publishing extensions (when configured with write access):

```bash
# Publish to git store (will commit and push changes)
quelle publish extension ./my-extension --store github-extensions

# Unpublish from git store
quelle publish unpublish my-extension 1.0.0 --store github-extensions
```

For publishing to work, the store needs:
- Write permissions (push access to repository)
- Appropriate authentication configured
- Git author information configured in the store

## Migration from Local Stores

If you have local extension stores that you want to move to git:

1. Initialize a git repository in your local store directory
2. Commit and push to your git platform
3. Remove the local store and add as git store
4. Verify extensions are still accessible

```bash
# Example migration
cd ~/.local/share/quelle/stores/my-local-store
git init
git add .
git commit -m "Initial commit of extension store"
git remote add origin https://github.com/user/my-extensions.git
git push -u origin main

# Back to quelle CLI
quelle store remove my-local-store --force
quelle store add git my-extensions https://github.com/user/my-extensions.git
```

## Default Directory Structure

When using default paths for local stores, Quelle creates the following structure:

```
~/.local/share/quelle/
├── stores/
│   ├── my-store/
│   │   ├── store.json          # Store manifest
│   │   └── extensions/         # Extension packages
│   ├── another-store/
│   │   ├── store.json
│   │   └── extensions/
│   └── git-cache-store/        # Git store cache
│       ├── .git/               # Git repository
│       ├── store.json          # Store manifest
│       └── extensions/         # Extension packages
└── config.json                 # Quelle configuration
```

This structure keeps everything organized and makes it easy to back up or migrate your extension stores.