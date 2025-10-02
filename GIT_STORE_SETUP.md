# Git Store Setup Guide

This guide shows you how to properly set up a git-backed store in Quelle with automatic commit and push functionality.

## Quick Setup

For a git store that automatically commits and pushes changes:

```rust
use quelle_store::stores::{LocallyCachedStore, WritableStore};
use quelle_store::stores::providers::git::{
    GitAuth, GitProvider, GitReference, GitWriteConfig, GitAuthor
};
use std::path::PathBuf;
use std::env;

async fn setup_writable_git_store() -> Result<LocallyCachedStore<GitProvider>, Box<dyn std::error::Error>> {
    // Get authentication token from environment
    let token = env::var("GITHUB_TOKEN")
        .expect("GITHUB_TOKEN environment variable not set");
    
    // Configure git write settings (REQUIRED for commits/pushes)
    let write_config = GitWriteConfig {
        author: GitAuthor {
            name: "Your Name".to_string(),
            email: "your.email@example.com".to_string(),
        },
        commit_message_template: "{action} extension {extension_id} v{version}".to_string(),
        auto_push: true,  // Set to false to only commit locally
        write_auth: None, // Use provider's auth, or specify different auth for writing
        write_branch: None, // Use default branch
    };

    // Create git provider with authentication and write config
    let provider = GitProvider::new(
        "https://github.com/my-org/extension-store.git".to_string(),
        PathBuf::from("./cache/git-store"),
        GitReference::Default,
        GitAuth::Token { token },
    )
    .with_write_config(write_config); // THIS IS ESSENTIAL!

    let store = LocallyCachedStore::new(
        provider,
        PathBuf::from("./cache/git-store"),
        "my-extension-store".to_string(),
    )?;

    Ok(store)
}
```

## Why Commits Don't Happen

If your git store isn't committing during initialization or publish operations, it's likely because:

### 1. Missing GitWriteConfig

**Problem:** Git store created without write configuration
```rust
// ❌ This WON'T commit or push
let provider = GitProvider::new(
    "https://github.com/user/repo.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::Token { token },
);
// Missing .with_write_config()!
```

**Solution:** Add write configuration
```rust
// ✅ This WILL commit and push
let write_config = GitWriteConfig {
    author: GitAuthor {
        name: "Bot User".to_string(),
        email: "bot@example.com".to_string(),
    },
    commit_message_template: "{action} extension {extension_id} v{version}".to_string(),
    auto_push: true,
    write_auth: None,
    write_branch: None,
};

let provider = GitProvider::new(
    "https://github.com/user/repo.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::Token { token },
)
.with_write_config(write_config); // ✅ Now it's writable!
```

### 2. Missing Authentication

**Problem:** No authentication configured for pushing
```rust
// ❌ This will fail to push (but may commit locally)
let provider = GitProvider::new(
    "https://github.com/user/repo.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::None, // No auth for pushing!
);
```

**Solution:** Configure authentication
```rust
// ✅ This will push successfully
GitAuth::Token { token: "your_github_token".to_string() }
// or
GitAuth::SshKey { 
    private_key_path: PathBuf::from("/home/user/.ssh/id_rsa"),
    public_key_path: None,
    passphrase: None,
}
```

## Complete Working Example

Here's a complete example that demonstrates all the pieces working together:

```rust
use quelle_store::stores::{LocallyCachedStore, WritableStore, BaseStore};
use quelle_store::stores::providers::git::{
    GitAuth, GitProvider, GitReference, GitWriteConfig, GitAuthor
};
use std::path::PathBuf;
use std::env;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging to see what's happening
    env_logger::init();

    // 1. Get authentication
    let github_token = env::var("GITHUB_TOKEN")
        .expect("Please set GITHUB_TOKEN environment variable");

    // 2. Configure git write settings
    let write_config = GitWriteConfig {
        author: GitAuthor {
            name: "Extension Publisher".to_string(),
            email: "publisher@myorg.com".to_string(),
        },
        commit_message_template: "{action} extension {extension_id} v{version}".to_string(),
        auto_push: true,
        write_auth: None, // Use the provider's main auth
        write_branch: None, // Use default branch
    };

    // 3. Create git provider
    let provider = GitProvider::new(
        "https://github.com/my-org/extension-store.git".to_string(),
        PathBuf::from("./data/stores/git-store"),
        GitReference::Default,
        GitAuth::Token { token: github_token },
    )
    .with_write_config(write_config);

    // 4. Create store
    let store = LocallyCachedStore::new(
        provider,
        PathBuf::from("./data/stores/git-store"),
        "my-git-store".to_string(),
    )?;

    // 5. Initialize store (this will commit and push!)
    println!("Initializing git store...");
    store.initialize_store(
        "My Extension Store".to_string(),
        Some("A git-backed extension store with automatic versioning".to_string()),
    ).await?;

    println!("✅ Git store initialized successfully!");
    println!("Check your repository - you should see a new commit with store.json");

    // 6. Test health
    let health = store.health_check().await?;
    println!("Store health: {:?}", health);

    Ok(())
}
```

## Environment Setup

Create a `.env` file or set environment variables:

```bash
# GitHub Personal Access Token
export GITHUB_TOKEN="ghp_your_token_here"

# Or for GitLab
export GITLAB_TOKEN="glpat-your_token_here"
```

### Creating GitHub Personal Access Token

1. Go to GitHub Settings → Developer settings → Personal access tokens
2. Generate new token (classic)
3. Select scopes:
   - `repo` (full repository access) - for private repos
   - `public_repo` - for public repos only
4. Copy the token and set it as `GITHUB_TOKEN`

## Configuration Options

### GitWriteConfig Fields

```rust
GitWriteConfig {
    // Author info for commits (REQUIRED)
    author: GitAuthor {
        name: "Commit Author Name".to_string(),
        email: "author@example.com".to_string(),
    },
    
    // Template for commit messages (REQUIRED)
    // Available placeholders: {action}, {extension_id}, {version}
    commit_message_template: "{action} extension {extension_id} v{version}".to_string(),
    
    // Whether to automatically push after commits (default: false)
    auto_push: true,
    
    // Override authentication for write operations (optional)
    // If None, uses the provider's main auth
    write_auth: Some(GitAuth::Token { token: "different_token".to_string() }),
    
    // Target branch for commits (optional)
    // If None, uses the default branch
    write_branch: Some("main".to_string()),
}
```

### Authentication Options

```rust
// 1. GitHub/GitLab Personal Access Token (recommended)
GitAuth::Token { 
    token: "ghp_your_token_here".to_string() 
}

// 2. SSH Key (for automated systems)
GitAuth::SshKey {
    private_key_path: PathBuf::from("/home/user/.ssh/id_rsa"),
    public_key_path: Some(PathBuf::from("/home/user/.ssh/id_rsa.pub")),
    passphrase: None, // or Some("passphrase".to_string())
}

// 3. Username/Password (less secure)
GitAuth::UserPassword {
    username: "your_username".to_string(),
    password: "your_password_or_token".to_string(),
}

// 4. System credentials (uses git credential manager, SSH agent, etc.)
GitAuth::None
```

## Troubleshooting

### "Git workflow failed after successful initialization"

This warning appears when:
- Authentication is invalid or expired
- Repository doesn't exist or you don't have push permissions
- Network connectivity issues

**Solutions:**
1. Verify your token has the correct permissions
2. Test with `git push` from command line using the same credentials
3. Check repository URL is correct
4. Ensure repository exists and you have write access

### "Git provider is not writable, skipping git workflow"

This means you didn't configure `GitWriteConfig`. Add it:

```rust
let provider = GitProvider::new(/* ... */)
    .with_write_config(GitWriteConfig {
        author: GitAuthor { /* ... */ },
        commit_message_template: "...".to_string(),
        auto_push: true,
        write_auth: None,
        write_branch: None,
    });
```

### Commits happen but pushes fail

If you see commits in your local git history but they don't appear on GitHub/GitLab:

1. **Check authentication:** Your token might not have push permissions
2. **Check auto_push setting:** Make sure it's set to `true`
3. **Check branch:** You might be committing to a branch that doesn't exist on remote

### Repository not found errors

- Verify the repository URL is correct
- Ensure the repository exists
- Check you have access to the repository
- For private repos, ensure your token has appropriate permissions

## What Gets Committed

When you perform operations on a git store, the following gets committed:

### During Initialization
- `store.json` - Store manifest with metadata
- Commit message: "Initialize git store: {store_name}"

### During Publish
- `store.json` - Updated store manifest
- `extensions/{extension_id}/{version}/` - Extension files
- Commit message: "Add extension {extension_id} v{version}" (or your template)

### During Unpublish
- `store.json` - Updated store manifest  
- Removes `extensions/{extension_id}/{version}/` directory
- Commit message: "Remove extension {extension_id} v{version}" (or your template)

## Best Practices

1. **Use environment variables** for tokens - never hardcode them
2. **Test authentication** with `git push` from command line first
3. **Use descriptive commit messages** - customize the template for your needs
4. **Monitor git operations** - check logs for warnings about failed pushes
5. **Use SSH keys** for automated/CI environments
6. **Rotate tokens regularly** for security
7. **Use branch protection** rules on important repositories
8. **Backup your repositories** - git stores contain important data

## Integration with CI/CD

For automated environments:

```rust
// Use different auth for CI
let auth = if env::var("CI").is_ok() {
    // CI environment - use SSH key
    GitAuth::SshKey {
        private_key_path: PathBuf::from("/secrets/deploy_key"),
        public_key_path: None,
        passphrase: None,
    }
} else {
    // Development - use token
    GitAuth::Token { 
        token: env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN required") 
    }
};
```

This ensures your git store works both in development and production environments.