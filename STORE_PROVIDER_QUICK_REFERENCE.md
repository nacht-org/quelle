# StoreProvider Quick Reference

Quick reference for the streamlined StoreProvider trait and store initialization API.

## Creating Stores

### Simple Git Store (Read-Only)

```rust
use quelle_store::GitStore;

let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir("/path/to/cache")
    .name("my-store")
    .build()?;
```

### Git Store with Authentication

```rust
use quelle_store::{GitStore, GitAuth};

let store = GitStore::builder()
    .url("https://github.com/user/private-repo.git")
    .cache_dir("/path/to/cache")
    .name("private-store")
    .auth(GitAuth::Token { 
        token: env::var("GITHUB_TOKEN")? 
    })
    .build()?;
```

### Git Store with Specific Branch

```rust
let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir("/path/to/cache")
    .name("dev-store")
    .branch("develop")
    .build()?;
```

### Writable Git Store

```rust
use quelle_store::{GitStore, GitAuth, CommitStyle};

let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir("/path/to/cache")
    .name("writable-store")
    .auth(GitAuth::Token { token })
    .writable()
    .author("Bot", "bot@example.com")
    .commit_style(CommitStyle::Conventional)
    .build()?;
```

### Full Configuration

```rust
use quelle_store::{GitStore, GitAuth, GitReference, CommitStyle};
use std::time::Duration;

let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir("/path/to/cache")
    .name("custom-store")
    .branch("main")
    .auth(GitAuth::Token { token })
    .fetch_interval(Duration::from_secs(600))
    .shallow(false)
    .writable()
    .author("Bot", "bot@example.com")
    .commit_style(CommitStyle::Detailed)
    .no_auto_push()
    .build()?;
```

## Builder Methods

### Required Methods (must call before build)

| Method | Description | Example |
|--------|-------------|---------|
| `url()` | Git repository URL | `.url("https://github.com/user/repo.git")` |
| `cache_dir()` | Local cache directory | `.cache_dir("/path/to/cache")` |
| `name()` | Store name | `.name("my-store")` |

### Optional Methods

| Method | Description | Default | Example |
|--------|-------------|---------|---------|
| `auth()` | Authentication method | None | `.auth(GitAuth::Token { token })` |
| `branch()` | Use specific branch | Default branch | `.branch("develop")` |
| `tag()` | Use specific tag | - | `.tag("v1.0.0")` |
| `commit()` | Use specific commit | - | `.commit("abc123")` |
| `reference()` | Set git reference | Default | `.reference(GitReference::Branch("main".into()))` |
| `fetch_interval()` | Update check interval | 5 minutes | `.fetch_interval(Duration::from_secs(300))` |
| `shallow()` | Enable/disable shallow clone | true | `.shallow(false)` |
| `writable()` | Enable write operations | false | `.writable()` |
| `author()` | Set commit author | From git config | `.author("Name", "email@example.com")` |
| `commit_style()` | Commit message style | Simple | `.commit_style(CommitStyle::Conventional)` |
| `no_auto_push()` | Disable auto-push | Auto-push enabled | `.no_auto_push()` |
| `write_config()` | Custom write config | - | `.write_config(config)` |

## Authentication Types

### No Authentication (Public Repos)

```rust
// Default - no .auth() call needed
let store = GitStore::builder()
    .url("https://github.com/user/public-repo.git")
    .cache_dir(cache)
    .name("store")
    .build()?;
```

### Personal Access Token

```rust
use quelle_store::GitAuth;

let auth = GitAuth::Token { 
    token: "ghp_xxxxxxxxxxxx".to_string() 
};

let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir(cache)
    .name("store")
    .auth(auth)
    .build()?;
```

### SSH Key

```rust
use quelle_store::GitAuth;
use std::path::PathBuf;

let auth = GitAuth::SshKey {
    private_key_path: PathBuf::from("~/.ssh/id_rsa"),
    public_key_path: Some(PathBuf::from("~/.ssh/id_rsa.pub")),
    passphrase: Some("key_password".to_string()),
};

let store = GitStore::builder()
    .url("git@github.com:user/repo.git")
    .cache_dir(cache)
    .name("store")
    .auth(auth)
    .build()?;
```

### Username/Password

```rust
use quelle_store::GitAuth;

let auth = GitAuth::UserPassword {
    username: "username".to_string(),
    password: "password".to_string(),
};
```

## Git References

```rust
use quelle_store::GitReference;

// Default branch (main/master)
.reference(GitReference::Default)

// Specific branch
.branch("develop")
// or
.reference(GitReference::Branch("develop".into()))

// Specific tag
.tag("v1.0.0")
// or
.reference(GitReference::Tag("v1.0.0".into()))

// Specific commit
.commit("abc123def456")
// or
.reference(GitReference::Commit("abc123def456".into()))
```

## Commit Styles

```rust
use quelle_store::CommitStyle;

// Simple: "Publish extension-id@1.0.0"
.commit_style(CommitStyle::Simple)

// Conventional: "feat(extension-id): publish version 1.0.0"
.commit_style(CommitStyle::Conventional)

// Detailed: Multi-line with metadata
.commit_style(CommitStyle::Detailed)
```

## StoreProvider Trait

### For Provider Implementors

```rust
use quelle_store::stores::providers::traits::{StoreProvider, SyncResult};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

struct MyProvider {
    cache_dir: PathBuf,
    // ... other fields
}

#[async_trait]
impl StoreProvider for MyProvider {
    // Required: Return the sync directory
    fn sync_dir(&self) -> &Path {
        &self.cache_dir
    }
    
    // Required: Sync data from source to cache
    async fn sync(&self) -> Result<SyncResult> {
        // Sync logic here
        Ok(SyncResult::with_changes(vec!["synced".into()]))
    }
    
    // Required: Check if sync is needed
    async fn needs_sync(&self) -> Result<bool> {
        // Check logic here
        Ok(true)
    }
    
    // Required: Human-readable description
    fn description(&self) -> String {
        "My custom provider".to_string()
    }
    
    // Required: Provider type identifier
    fn provider_type(&self) -> &'static str {
        "my-provider"
    }
    
    // Optional: Sync only if needed (has default impl)
    async fn sync_if_needed(&self) -> Result<Option<SyncResult>> {
        if self.needs_sync().await? {
            Ok(Some(self.sync().await?))
        } else {
            Ok(None)
        }
    }
    
    // Optional: Check if writable (default: false)
    fn is_writable(&self) -> bool {
        false
    }
    
    // Optional: Post-publish hook (default: no-op)
    async fn post_publish(&self, extension_id: &str, version: &str) -> Result<()> {
        Ok(())
    }
    
    // Optional: Post-unpublish hook (default: no-op)
    async fn post_unpublish(&self, extension_id: &str, version: &str) -> Result<()> {
        Ok(())
    }
    
    // Optional: Check write status (default: checks is_writable)
    async fn check_write_status(&self) -> Result<()> {
        if !self.is_writable() {
            return Err(StoreError::InvalidPackage {
                reason: "Not writable".to_string(),
            });
        }
        Ok(())
    }
}
```

## Using Custom Providers

```rust
use quelle_store::stores::locally_cached::LocallyCachedStore;

let provider = MyProvider {
    cache_dir: PathBuf::from("/cache"),
    // ...
};

let store = LocallyCachedStore::new(provider, "my-store".to_string())?;

// Use store like any other
let extensions = store.list_extensions().await?;
```

## Migration from Old API

### Before (Old API)

```rust
// Old way - redundant sync_dir
let provider = GitProvider::new(url, cache.clone(), ref, auth);
let store = LocallyCachedStore::new(provider, cache, name)?;
```

### After (New API)

```rust
// New way - no redundancy
let store = GitStore::builder()
    .url(url)
    .cache_dir(cache)
    .name(name)
    .reference(ref)
    .auth(auth)
    .build()?;
```

## Common Patterns

### Local Development Store

```rust
use quelle_store::LocalStore;

let store = LocalStore::new("./extensions")?;
```

### Multiple Stores

```rust
// Local dev store
let dev = LocalStore::new("./dev-extensions")?;

// Official git store
let official = GitStore::builder()
    .url("https://github.com/org/official.git")
    .cache_dir(cache_dir.join("official"))
    .name("official")
    .build()?;

// Community git store with auth
let community = GitStore::builder()
    .url("https://github.com/org/community.git")
    .cache_dir(cache_dir.join("community"))
    .name("community")
    .auth(GitAuth::Token { token })
    .build()?;
```

### Error Handling

```rust
let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir(cache)
    .name("store")
    .build()
    .map_err(|e| {
        eprintln!("Failed to create store: {}", e);
        e
    })?;
```

### With Validation

```rust
// Builder validates at build time
let result = GitStore::builder()
    .url("https://github.com/user/repo.git")
    // Missing .cache_dir() and .name()
    .build();

assert!(result.is_err()); // Error: cache_dir must be set
```

## Best Practices

1. **Always set required fields**: `url`, `cache_dir`, and `name`
2. **Use builder for complex config**: More readable and maintainable
3. **Handle errors properly**: Builder can fail if required fields missing
4. **Use meaningful names**: Store names help with debugging and logging
5. **Consider fetch_interval**: Balance freshness vs. network usage
6. **Enable writes explicitly**: Prevents accidental modifications
7. **Set author for commits**: Better attribution in git history
8. **Use appropriate auth**: Token for HTTPS, SSH key for git protocol

## Troubleshooting

### Error: "cache_dir must be set"

```rust
// ❌ Missing cache_dir
let store = GitStore::builder()
    .url("...")
    .name("store")
    .build()?;

// ✅ Fixed
let store = GitStore::builder()
    .url("...")
    .cache_dir("/path/to/cache")
    .name("store")
    .build()?;
```

### Error: "name must be set"

```rust
// ❌ Missing name
let store = GitStore::builder()
    .url("...")
    .cache_dir(cache)
    .build()?;

// ✅ Fixed
let store = GitStore::builder()
    .url("...")
    .cache_dir(cache)
    .name("my-store")
    .build()?;
```

### Error: "Provider does not support write operations"

```rust
// ❌ Trying to publish to read-only store
let store = GitStore::builder()
    .url("...")
    .cache_dir(cache)
    .name("store")
    .build()?;
store.publish(package, options).await?; // Error!

// ✅ Fixed - enable writes
let store = GitStore::builder()
    .url("...")
    .cache_dir(cache)
    .name("store")
    .writable()
    .build()?;
```

## See Also

- `STORE_PROVIDER_REFACTORING.md` - Detailed refactoring documentation
- `GIT_STORE_REFACTORING.md` - Git store builder pattern details
- `LOCAL_STORE_REFACTORING.md` - Local store builder pattern details
- `BUILDER_QUICK_REFERENCE.md` - General builder pattern reference