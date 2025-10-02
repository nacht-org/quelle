# Git Store Authentication Guide

This guide explains how to configure authentication for git-backed stores in Quelle to enable publishing operations.

## Overview

While **reading** from public git repositories doesn't require authentication, **writing** (publishing/unpublishing extensions) to any git repository always requires proper authentication credentials.

**Good News:** If you don't explicitly configure authentication, Quelle will automatically attempt to use your system's git credentials including:
- SSH keys loaded in your SSH agent
- Git credential managers (Git Credential Manager, macOS Keychain, etc.)
- Stored credentials from previous `git` command usage

This means if you can already push to the repository using `git push` from your command line, Quelle should work automatically without additional configuration.

## Authentication Methods

### 0. System Default (Automatic)

By default, Quelle will attempt to use your system's existing git authentication:

```rust
use quelle_store::stores::providers::git::{GitAuth, GitProvider, GitReference};
use quelle_store::stores::locally_cached::LocallyCachedStore;

// GitAuth::None will use your system's git credentials
let provider = GitProvider::new(
    "https://github.com/username/store.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::None, // Uses system credentials automatically
);

let store = LocallyCachedStore::new(provider, sync_dir, "my-store".to_string())?;
```

This works if:
- You have SSH keys loaded in your SSH agent (`ssh-add -l` shows your keys)
- You have credentials stored in Git Credential Manager or similar
- You've previously authenticated with the repository using `git push`

**Test it first:** Try `git clone` and `git push` with the repository URL from your command line to verify your system authentication works.

### 1. Personal Access Token (Recommended)

The easiest and most secure method for GitHub, GitLab, and similar platforms.

```rust
use quelle_store::stores::providers::git::{GitAuth, GitProvider, GitReference};
use quelle_store::stores::locally_cached::LocallyCachedStore;

let provider = GitProvider::new(
    "https://github.com/username/store.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::Token {
        token: "ghp_your_personal_access_token_here".to_string(),
    },
);

let store = LocallyCachedStore::new(provider, sync_dir, "my-store".to_string())?;
```

#### Creating Personal Access Tokens:

**GitHub:**
1. Go to Settings → Developer settings → Personal access tokens
2. Generate new token (classic)
3. Select scopes: `repo` (for private repos) or `public_repo` (for public repos)

**GitLab:**
1. Go to User Settings → Access Tokens
2. Create personal access token with `write_repository` scope

### 2. SSH Key Authentication

Best for automated systems and when you prefer key-based authentication.

```rust
use std::path::PathBuf;

let provider = GitProvider::new(
    "git@github.com:username/store.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::SshKey {
        private_key_path: PathBuf::from("/home/user/.ssh/id_rsa"),
        public_key_path: Some(PathBuf::from("/home/user/.ssh/id_rsa.pub")),
        passphrase: None, // or Some("passphrase".to_string()) if key is encrypted
    },
);
```

### 3. Username/Password

Less secure but sometimes necessary for basic authentication.

```rust
let provider = GitProvider::new(
    "https://github.com/username/store.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::UserPassword {
        username: "your_username".to_string(),
        password: "your_password_or_token".to_string(),
    },
);
```

## Write Configuration

To enable automatic pushing after publish operations, configure the git store with write settings:

```rust
use quelle_store::stores::providers::git::{GitProvider, GitWriteConfig, GitAuthor};

let write_config = GitWriteConfig {
    author: GitAuthor {
        name: "Your Name".to_string(),
        email: "your.email@example.com".to_string(),
    },
    commit_message_template: "{action} extension {extension_id} v{version}".to_string(),
    auto_push: true, // Set to false to only commit locally
    write_auth: None, // Use provider's auth, or specify different auth for writing
};

let provider = GitProvider::new(
    "https://github.com/my-org/extension-store.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::Token { token: "your_token".to_string() },
)
.with_write_config(write_config);
```

**Note:** When you call `initialize_store()` on a writable git store, it will automatically:
1. Create the `store.json` manifest file
2. Add it to git staging area
3. Commit it with a message like "Initialize git store: Your Store Name"
4. Push to remote (if `auto_push: true` and authentication is configured)

This ensures your store initialization is properly recorded in git history.

## Disabling Auto-Push

If you want to create commits locally but not push automatically:

```rust
let write_config = GitWriteConfig {
    author: GitAuthor {
        name: "Your Name".to_string(),
        email: "your.email@example.com".to_string(),
    },
    commit_message_template: "{action} extension {extension_id} v{version}".to_string(),
    auto_push: false, // Only commit locally, don't push
    write_auth: None,
};
```

**Important:** Store initialization will automatically commit and push the initial `store.json` file when:
- The store has write configuration (`GitWriteConfig`)
- Auto-push is enabled (`auto_push: true`)
- Valid authentication is configured

## Environment Variables

For security, avoid hardcoding tokens in your code:

```rust
use std::env;

let token = env::var("GITHUB_TOKEN")
    .expect("GITHUB_TOKEN environment variable not set");

let provider = GitProvider::new(
    "https://github.com/username/store.git".to_string(),
    cache_dir,
    GitReference::Default,
    GitAuth::Token { token },
);
```

## Common Issues

### "Push rejected: remote authentication required but no callback set"

This error occurs when:
- Your system doesn't have valid git credentials configured
- SSH keys aren't loaded in the SSH agent
- Git credential manager isn't set up properly
- Invalid or expired authentication credentials

**Solutions:**
1. **Test system auth:** Run `git push` to the same repository from command line
2. **Load SSH keys:** Run `ssh-add ~/.ssh/id_rsa` (or your key path)
3. **Configure explicit auth:** Use one of the explicit authentication methods above

### "Push rejected: insufficient permissions"

Your authentication credentials don't have write access to the repository.

**Solutions:**
- Ensure your personal access token has the correct scopes
- Verify you have push permissions to the repository
- Check that the repository URL is correct

### SSH Key Issues

**Common problems:**
- SSH key not added to your GitHub/GitLab account
- Incorrect path to SSH key files
- SSH agent not running

**Solutions:**
- Add your public key to your git provider account
- Verify SSH key paths are correct
- Test SSH connection: `ssh -T git@github.com`

## Repository URLs

**HTTPS format (for tokens/username-password):**
```
https://github.com/username/repository.git
https://gitlab.com/username/repository.git
```

**SSH format (for SSH keys):**
```
git@github.com:username/repository.git
git@gitlab.com:username/repository.git
```

## Security Best Practices

1. **Use Personal Access Tokens** instead of passwords
2. **Limit token scopes** to minimum required permissions
3. **Store credentials securely** (environment variables, secret management)
4. **Rotate tokens regularly**
5. **Use SSH keys** for automated systems
6. **Never commit credentials** to source code

## Testing Authentication

To test if your authentication is working:

```rust
// This will fail early if authentication is misconfigured
let result = store.publish(package, options).await;
match result {
    Ok(_) => println!("Publish successful!"),
    Err(e) if e.to_string().contains("authentication") => {
        println!("Authentication error: {}", e);
        // Check your credentials
    },
    Err(e) => println!("Other error: {}", e),
}
```

## Complete Example

```rust
use quelle_store::stores::{LocallyCachedStore, WritableStore};
use quelle_store::stores::providers::git::{
    GitAuth, GitProvider, GitReference, GitWriteConfig, GitAuthor
};
use std::path::PathBuf;
use std::env;

async fn setup_git_store() -> Result<LocallyCachedStore<GitProvider>, Box<dyn std::error::Error>> {
    // Option 1: Use system credentials (recommended for development)
    let auth = if let Ok(token) = env::var("GITHUB_TOKEN") {
        GitAuth::Token { token }
    } else {
        GitAuth::None // Use system git credentials
    };
    
    let write_config = GitWriteConfig {
        author: GitAuthor {
            name: "Extension Publisher".to_string(),
            email: "publisher@example.com".to_string(),
        },
        commit_message_template: "{action} extension {extension_id} v{version}".to_string(),
        auto_push: true,
        write_auth: None,
    };

    let provider = GitProvider::new(
        "https://github.com/my-org/extension-store.git".to_string(),
        PathBuf::from("./cache/git-store"),
        GitReference::Default,
        auth,
    )
    .with_write_config(write_config);

    let store = LocallyCachedStore::new(
        provider,
        PathBuf::from("./cache/git-store"),
        "my-extension-store".to_string(),
    )?;

    // Initialize store if needed
    // This will create store.json, commit it, and push to remote if configured
    store.initialize_store(
        "My Extension Store".to_string(),
        Some("A git-backed extension store".to_string()),
    ).await?;

    Ok(store)
}
```
