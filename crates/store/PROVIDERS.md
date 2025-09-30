# Store Providers Documentation

This document describes the new Store Provider system in Quelle, which allows for flexible syncing of extension stores from various sources (local filesystem, Git repositories, etc.) while using a common interface for reading the data.

## Overview

The provider system consists of three main components:

1. **StoreProvider trait** - Defines how to sync data from various sources
2. **LocallyCachedStore** - Wraps a provider and LocalStore for unified access
3. **Concrete Providers** - Implementations for specific source types (Local, Git)

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   StoreProvider │◄───│ LocallyCachedStore│────►   LocalStore   │
│                 │    │                  │    │                 │
│ - sync()        │    │ - provider       │    │ - extensions/   │
│ - needs_sync()  │    │ - local_store    │    │ - manifests     │
│ - sync_if_needed() │    │ - sync_dir       │    │ - validation    │
└─────────────────┘    └──────────────────┘    └─────────────────┘
        ▲                                               ▲
        │                                               │
┌───────┴────────┐                           ┌─────────┴─────────┐
│ LocalProvider  │                           │ All existing     │
│ GitProvider    │                           │ LocalStore        │
│ HttpProvider   │                           │ functionality     │
│ S3Provider     │                           │                   │
└────────────────┘                           └───────────────────┘
```

## Core Concepts

### StoreProvider Trait

The `StoreProvider` trait defines the interface for syncing data from various sources:

```rust
#[async_trait]
pub trait StoreProvider: Send + Sync {
    /// Sync/update the local store from the source to the given directory
    async fn sync(&self, sync_dir: &Path) -> Result<SyncResult>;
    
    /// Check if sync is needed (based on time, changes, etc.)
    async fn needs_sync(&self, sync_dir: &Path) -> Result<bool>;
    
    /// Sync only if needed - default implementation
    async fn sync_if_needed(&self, sync_dir: &Path) -> Result<Option<SyncResult>> {
        if self.needs_sync(sync_dir).await? {
            Ok(Some(self.sync(sync_dir).await?))
        } else {
            Ok(None)
        }
    }
    
    /// Get a human-readable description of this provider
    fn description(&self) -> String;
    
    /// Get the provider type identifier
    fn provider_type(&self) -> &'static str;
}
```

### LocallyCachedStore

`LocallyCachedStore<T: StoreProvider>` wraps a provider and a `LocalStore`, providing:

- Automatic syncing before read operations
- Unified interface implementing all store traits
- Efficient caching of synced data

### SyncResult

The `SyncResult` struct provides information about sync operations:

```rust
pub struct SyncResult {
    pub updated: bool,                    // Whether any changes were made
    pub changes: Vec<String>,             // List of changes made
    pub warnings: Vec<String>,            // Non-fatal warnings
    pub completed_at: DateTime<Utc>,      // When sync completed
    pub bytes_transferred: Option<u64>,   // Bytes transferred (if applicable)
}
```

## Provider Implementations



### GitProvider

For Git repository-based stores:

```rust
use quelle_store::{GitProvider, GitAuth, GitReference, LocallyCachedStore};

// Create a git provider
let provider = GitProvider::new(
    "https://github.com/user/extensions-repo.git".to_string(),
    cache_dir.clone(),
    GitReference::Branch("main".to_string()),
    GitAuth::Token { token: "ghp_xxxx".to_string() }
);

let store = LocallyCachedStore::new(
    provider,
    cache_dir,
    "my-git-store".to_string()
)?;

// First access will clone the repository
// Subsequent accesses will fetch updates if needed
```

**Characteristics:**
- `sync()` clones on first run, fetches updates on subsequent runs
- `needs_sync()` checks fetch interval and repository existence
- Supports branches, tags, commits, and various authentication methods
- Configurable fetch intervals and shallow/deep clones

### GitStore Type Alias

For convenience, `GitStore` is a type alias for `LocallyCachedStore<GitProvider>` with additional helper methods:

```rust
use quelle_store::GitStore;

// Simple creation
let store = GitStore::from_url(
    "my-store".to_string(),
    "https://github.com/user/repo.git".to_string(),
    cache_dir
)?;

// With authentication
let store = GitStore::with_auth(
    "private-store".to_string(),
    "https://github.com/user/private-repo.git".to_string(),
    cache_dir,
    GitAuth::Token { token: "ghp_xxxx".to_string() }
)?;

// With specific branch
let store = GitStore::with_branch(
    "dev-store".to_string(),
    "https://github.com/user/repo.git".to_string(),
    cache_dir,
    "develop".to_string()
)?;

// Full customization
let store = GitStore::with_config(
    "custom-store".to_string(),
    "https://github.com/user/repo.git".to_string(),
    cache_dir,
    GitReference::Tag("v2.0.0".to_string()),
    GitAuth::SshKey {
        private_key_path: PathBuf::from("~/.ssh/id_rsa"),
        public_key_path: None,
        passphrase: Some("password".to_string())
    },
    Duration::from_secs(1800), // 30 minute fetch interval
    false // Don't use shallow clone
)?;
```

## Git Authentication

GitProvider supports multiple authentication methods:

### No Authentication (Public Repos)
```rust
let auth = GitAuth::None;
```

### Personal Access Token
```rust
let auth = GitAuth::Token { 
    token: "ghp_xxxxxxxxxxxx".to_string() 
};
```

### SSH Key
```rust
let auth = GitAuth::SshKey {
    private_key_path: PathBuf::from("~/.ssh/id_rsa"),
    public_key_path: Some(PathBuf::from("~/.ssh/id_rsa.pub")), // Optional
    passphrase: Some("key_password".to_string()) // Optional
};
```

### Username/Password
```rust
let auth = GitAuth::UserPassword {
    username: "username".to_string(),
    password: "password".to_string()
};
```

## Git References

GitProvider can checkout specific references:

### Default Branch
```rust
let reference = GitReference::Default; // Uses repository's default branch
```

### Specific Branch
```rust
let reference = GitReference::Branch("develop".to_string());
```

### Specific Tag
```rust
let reference = GitReference::Tag("v1.0.0".to_string());
```

### Specific Commit
```rust
let reference = GitReference::Commit("abc123def456".to_string());
```

## Usage Patterns

### Local vs Git Stores

For local extension directories, use `LocalStore` directly:

```rust
// Local stores - use existing LocalStore directly
let local_store = LocalStore::new("/path/to/extensions")?;
```

For remote git repositories, use `GitStore` (which uses the provider system):

```rust
// Git stores - use GitStore (LocallyCachedStore<GitProvider>)
let git_store = GitStore::from_url(
    "my-store".to_string(),
    "https://github.com/user/extensions.git".to_string(),
    cache_dir
)?;
```

### Git-Based Extension Distribution

Set up a store that automatically syncs from a Git repository:

```rust
use quelle_store::{GitStore, GitAuth};

// Store will sync from Git repository
let store = GitStore::with_auth(
    "community-extensions".to_string(),
    "https://github.com/quelle-org/community-extensions.git".to_string(),
    dirs::cache_dir().unwrap().join("quelle/stores/community"),
    GitAuth::Token { token: env::var("GITHUB_TOKEN")? }
)?;

// First use will clone the repository
let extensions = store.list_extensions().await?;

// Subsequent uses will fetch updates if the fetch interval has passed
let more_extensions = store.list_extensions().await?;

// Force a refresh
store.refresh_cache().await?;
```

### Multiple Store Types

Mix different provider types in the same application:

```rust
// Local development store - use LocalStore directly
let dev_store = quelle_store::stores::local::LocalStore::new("./dev-extensions")?;

// Official extensions from Git
let official_store = GitStore::from_url(
    "official".to_string(),
    "https://github.com/quelle-org/official-extensions.git".to_string(),
    cache_dir.join("official")
)?;

// Community extensions from Git with auth
let community_store = GitStore::with_auth(
    "community".to_string(),
    "https://github.com/quelle-org/community-extensions.git".to_string(),
    cache_dir.join("community"),
    GitAuth::Token { token: github_token }
)?;

// All stores implement ReadableStore trait
// Note: You'd need to ensure dev_store is properly initialized first
```

## Performance Considerations

### Git Provider Optimization

- **Shallow Clones**: Default enabled for faster initial clones
- **Fetch Intervals**: Default 1 hour to avoid excessive network requests
- **Local Caching**: Repository cached locally for subsequent accesses
- **Incremental Updates**: Only fetches new commits, not full re-clone

### Memory Usage

- **Lazy Loading**: Providers only sync when data is accessed
- **Cache Management**: LocalStore handles extension caching efficiently  
- **Resource Cleanup**: Providers properly clean up resources

### Network Efficiency

- **Conditional Syncing**: Only sync when `needs_sync()` returns true
- **Configurable Intervals**: Adjust fetch frequency per use case
- **Error Resilience**: Failed syncs don't break existing cached data

## Error Handling

The provider system includes comprehensive error handling:

### Provider-Specific Errors

```rust
// Git errors
StoreError::GitError {
    operation: "clone repository".to_string(),
    url: "https://github.com/user/repo.git".to_string(),
    source: git_error,
}

// IO errors during sync
StoreError::IoOperation {
    operation: "create cache directory".to_string(), 
    path: cache_dir,
    source: io_error,
}
```

### Error Recovery

- **Graceful Degradation**: Store continues working with cached data if sync fails
- **Retry Logic**: Providers can implement retry strategies
- **User Feedback**: Clear error messages with actionable suggestions

## Testing

### Unit Tests

Test providers in isolation:

```rust
#[tokio::test]
async fn test_git_provider() {
    let temp_dir = TempDir::new().unwrap();
    let provider = GitProvider::new(
        "https://github.com/test/repo.git".to_string(),
        temp_dir.path().to_path_buf(),
        GitReference::Default,
        GitAuth::None
    );
    
    // Should need sync initially (repo doesn't exist)
    assert!(provider.needs_sync(temp_dir.path()).await.unwrap());
}
```

### Integration Tests

Test full store functionality:

```rust
#[tokio::test] 
async fn test_git_store() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create a git store (would normally clone from real repo)
    let store = GitStore::from_url(
        "test-store".to_string(),
        "https://github.com/test/repo.git".to_string(),
        temp_dir.path().to_path_buf()
    ).unwrap();
    
    // Test store operations (would work after successful clone)
    // let extensions = store.list_extensions().await.unwrap();
}
```

## Future Extensibility

The provider system is designed for easy extension:

### Adding New Providers

```rust
struct HttpProvider {
    base_url: String,
    cache_dir: PathBuf,
    // ...
}

#[async_trait]
impl StoreProvider for HttpProvider {
    async fn sync(&self, sync_dir: &Path) -> Result<SyncResult> {
        // Download extensions from HTTP API
        // Extract to sync_dir
        // Return sync result
    }
    
    async fn needs_sync(&self, sync_dir: &Path) -> Result<bool> {
        // Check last-modified headers, etc.
    }
    
    // ...
}

// Type alias for convenience
type HttpStore = LocallyCachedStore<HttpProvider>;
```

### Provider Configuration

Future providers can be configured through the store configuration system:

```rust
enum StoreConfig {
    Local { path: PathBuf },
    Git { url: String, auth: GitAuth, reference: GitReference },
    Http { base_url: String, auth: HttpAuth },
    S3 { bucket: String, region: String, credentials: S3Credentials },
}
```

This provider system provides a clean, extensible foundation for supporting multiple store sources while maintaining backward compatibility and consistent behavior.