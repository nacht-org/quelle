# StoreProvider API Reference

Quick reference for implementing and using the streamlined `StoreProvider` trait.

## Core Trait

```rust
#[async_trait]
pub trait StoreProvider: Send + Sync {
    // Required: Core sync operations
    fn sync_dir(&self) -> &Path;
    async fn sync(&self) -> Result<SyncResult>;
    async fn needs_sync(&self) -> Result<bool>;
    
    // Required: Metadata
    fn description(&self) -> String;
    fn provider_type(&self) -> &'static str;
    
    // Required: Capability declaration
    fn supports_capability(&self, capability: Capability) -> bool;
    
    // Optional: Write operations (default implementations provided)
    async fn handle_event(&self, event: LifecycleEvent) -> Result<()>;
    async fn ensure_writable(&self) -> Result<()>;
}
```

## Capabilities

```rust
pub enum Capability {
    Write,              // Provider supports write operations
    IncrementalSync,    // Provider supports incremental syncing
    Authentication,     // Provider supports authentication
    RemotePush,         // Provider can push changes to remote
    Caching,            // Provider supports caching
    BackgroundSync,     // Provider supports background sync
}
```

## Lifecycle Events

```rust
pub enum LifecycleEvent {
    Published { extension_id: String, version: String },
    Unpublished { extension_id: String, version: String },
}

// Helper methods
event.extension_id()   // Get the extension ID
event.version()        // Get the version
event.is_publish()     // Check if this is a publish event
event.is_unpublish()   // Check if this is an unpublish event
```

## Implementing a Read-Only Provider

```rust
use async_trait::async_trait;
use quelle_store::stores::providers::traits::{Capability, StoreProvider, SyncResult};

pub struct MyProvider {
    cache_dir: PathBuf,
    // ... other fields
}

#[async_trait]
impl StoreProvider for MyProvider {
    fn sync_dir(&self) -> &Path {
        &self.cache_dir
    }
    
    async fn sync(&self) -> Result<SyncResult> {
        // Download/sync logic here
        Ok(SyncResult::with_changes(vec!["Synced".into()]))
    }
    
    async fn needs_sync(&self) -> Result<bool> {
        // Check if sync is needed
        Ok(true)
    }
    
    fn description(&self) -> String {
        format!("My provider at {}", self.url)
    }
    
    fn provider_type(&self) -> &'static str {
        "my-provider"
    }
    
    fn supports_capability(&self, _capability: Capability) -> bool {
        false // Read-only provider
    }
    
    // That's it! Default implementations handle the rest:
    // - handle_event() does nothing (no-op)
    // - ensure_writable() returns an error
}
```

## Implementing a Writable Provider

```rust
#[async_trait]
impl StoreProvider for MyWritableProvider {
    // ... sync_dir, sync, needs_sync, description, provider_type ...
    
    fn supports_capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::Write => self.write_config.is_some(),
            Capability::IncrementalSync => true,
            _ => false,
        }
    }
    
    async fn handle_event(&self, event: LifecycleEvent) -> Result<()> {
        // Only handle if writable
        let write_config = match &self.write_config {
            Some(c) => c,
            None => return Ok(()),
        };
        
        // Handle the event
        match &event {
            LifecycleEvent::Published { extension_id, version } => {
                // Commit/push/notify about publish
                self.commit_changes(&format!("Publish {extension_id} v{version}")).await?;
            }
            LifecycleEvent::Unpublished { extension_id, version } => {
                // Commit/push/notify about unpublish
                self.commit_changes(&format!("Unpublish {extension_id} v{version}")).await?;
            }
        }
        
        Ok(())
    }
    
    async fn ensure_writable(&self) -> Result<()> {
        // Check write capability first
        if !self.supports_capability(Capability::Write) {
            return Err(StoreError::InvalidPackage {
                reason: "Provider is read-only".to_string(),
            });
        }
        
        // Additional checks (authentication, state, etc.)
        if !self.is_authenticated() {
            return Err(StoreError::InvalidPackage {
                reason: "Not authenticated".to_string(),
            });
        }
        
        Ok(())
    }
}
```

## Using Providers

### Checking Capabilities

```rust
// Check if provider supports a capability
if provider.supports_capability(Capability::Write) {
    // Perform write operation
}

if provider.supports_capability(Capability::RemotePush) {
    // Provider can push to remote
}
```

### Syncing

```rust
// Check if sync is needed, then sync
if provider.needs_sync().await? {
    let result = provider.sync().await?;
    println!("Synced with {} changes", result.changes.len());
}
```

### Publishing (LocallyCachedStore)

```rust
// Lifecycle events are handled automatically
let store = LocallyCachedStore::new(provider, "my-store".to_string())?;

// This will:
// 1. Call ensure_writable() to validate state
// 2. Perform the publish operation
// 3. Call handle_event() with Published event
let result = store.publish(package, options).await?;
```

## Method Reference

### Required Methods

| Method | Purpose | Return Type |
|--------|---------|-------------|
| `sync_dir()` | Get the provider's sync directory | `&Path` |
| `sync()` | Sync data from source | `Result<SyncResult>` |
| `needs_sync()` | Check if sync is needed | `Result<bool>` |
| `description()` | Human-readable description | `String` |
| `provider_type()` | Provider type identifier | `&'static str` |
| `supports_capability()` | Check capability support | `bool` |

### Optional Methods (with defaults)

| Method | Purpose | Default | Override When |
|--------|---------|---------|---------------|
| `handle_event()` | Handle lifecycle events | No-op | Provider needs to react to publish/unpublish |
| `ensure_writable()` | Validate write state | Checks `Write` capability | Need additional validation (auth, state, etc.) |

## SyncResult

```rust
pub struct SyncResult {
    pub updated: bool,
    pub changes: Vec<String>,
    pub warnings: Vec<String>,
    pub completed_at: DateTime<Utc>,
    pub bytes_transferred: Option<u64>,
}

// Create results
SyncResult::no_changes()
SyncResult::with_changes(vec!["file1.txt".into()])
    .with_warning("Some warning".into())
    .with_bytes_transferred(1024)
```

## Common Patterns

### Time-Based Syncing

```rust
pub struct MyProvider {
    last_sync: RwLock<Option<Instant>>,
    sync_interval: Duration,
}

async fn needs_sync(&self) -> Result<bool> {
    match self.last_sync.read().unwrap().as_ref() {
        Some(last) => Ok(last.elapsed() >= self.sync_interval),
        None => Ok(true), // Never synced
    }
}
```

### Conditional Write Support

```rust
fn supports_capability(&self, capability: Capability) -> bool {
    match capability {
        Capability::Write => self.write_config.is_some(),
        Capability::RemotePush => {
            self.write_config.as_ref()
                .map(|c| c.auto_push)
                .unwrap_or(false)
        }
        _ => false,
    }
}
```

### Unified Event Handling

```rust
async fn handle_event(&self, event: LifecycleEvent) -> Result<()> {
    let (action, ext_id, version) = match &event {
        LifecycleEvent::Published { extension_id, version } => {
            ("Publish", extension_id, version)
        }
        LifecycleEvent::Unpublished { extension_id, version } => {
            ("Unpublish", extension_id, version)
        }
    };
    
    let message = format!("{} {} v{}", action, ext_id, version);
    self.commit_and_push(&message).await
}
```

## Migration from Old API

| Old Method | New Method | Notes |
|------------|------------|-------|
| `sync_if_needed()` | `needs_sync()` + `sync()` | More explicit |
| `is_writable()` | `supports_capability(Capability::Write)` | Use capability system |
| `post_publish()` | `handle_event(Published)` | Unified hook |
| `post_unpublish()` | `handle_event(Unpublished)` | Unified hook |
| `check_write_status()` | `ensure_writable()` | Renamed for clarity |

## Best Practices

1. **Always implement `supports_capability` explicitly** - Don't rely on defaults, make capabilities clear
2. **Keep `handle_event` idempotent** - It may be called multiple times
3. **Use `ensure_writable` for validation** - Don't perform validation in `handle_event`
4. **Return meaningful errors** - Include context about why operations fail
5. **Log warnings for non-fatal issues** - Use `SyncResult::with_warning()` for recoverable problems

## Examples

See full examples in:
- `crates/store/src/stores/providers/git.rs` - Full writable provider implementation
- `crates/store/examples/git_store_demo.rs` - Usage examples
- `STORE_PROVIDER_STREAMLINING.md` - Detailed migration guide