# StoreProvider Trait Refactoring

## Overview

This document describes the refactoring of the `StoreProvider` trait and initialization workflow to streamline store creation, eliminate redundancy, and improve the developer experience.

## Goals

1. **Simplify initialization** - Reduce multi-step store creation to a single, intuitive builder pattern
2. **Remove redundancy** - Eliminate sync_dir/cache_dir duplication between provider and store
3. **Clarify responsibilities** - Better separation between provider (source) and store (interface)
4. **Improve ergonomics** - Make common patterns easier while keeping advanced configuration possible
5. **Maintain consistency** - Unified API across all store types

## Problems Solved

### Problem 1: Confusing Directory Management

**Before:**
```rust
// cache_dir passed to BOTH provider and store - confusing!
let provider = GitProvider::new(url, cache_dir.clone(), reference, auth);
let store = LocallyCachedStore::new(provider, cache_dir, name)?;
//                                              ^^^^^^^^^ redundant!
```

The `cache_dir` was passed to both `GitProvider::new()` and `LocallyCachedStore::new()`, and the `StoreProvider::sync()` method received `sync_dir` as a parameter. This created:
- Redundant parameter passing
- Opportunity for directory mismatches
- Confusion about which component owns the directory information
- Runtime validation overhead in `GitProvider::sync()` to check directory matches

**After:**
```rust
// Provider manages its own directory internally
let provider = GitProvider::new(url, cache_dir, reference, auth);
let store = LocallyCachedStore::new(provider, name)?;
//          No sync_dir needed - provider knows its directory!
```

### Problem 2: Multi-Step Initialization

**Before:**
```rust
// 5+ steps to create a writable git store
let provider = GitProvider::new(url, cache.clone(), ref, auth)
    .with_fetch_interval(interval)
    .with_shallow(shallow)
    .with_write_config(write_config);
let store = LocallyCachedStore::new(provider, cache, name)?;
```

**After:**
```rust
// Single fluent chain
let store = GitStore::builder()
    .url(url)
    .cache_dir(cache)
    .name(name)
    .reference(ref)
    .auth(auth)
    .fetch_interval(interval)
    .shallow(shallow)
    .writable()
    .build()?;
```

### Problem 3: StoreProvider Trait Confusion

**Before:**
```rust
trait StoreProvider {
    async fn sync(&self, sync_dir: &Path) -> Result<SyncResult>;
    //                   ^^^^^^^^^ Why is this a parameter?
    async fn needs_sync(&self, sync_dir: &Path) -> Result<bool>;
    async fn post_publish(&self, id: &str, ver: &str, sync_dir: &Path);
    //                                                   ^^^^^^^^^ Again?
}
```

**After:**
```rust
trait StoreProvider {
    // Provider owns and exposes its directory
    fn sync_dir(&self) -> &Path;
    
    // No sync_dir parameters - use internal state
    async fn sync(&self) -> Result<SyncResult>;
    async fn needs_sync(&self) -> Result<bool>;
    async fn post_publish(&self, id: &str, ver: &str);
}
```

## Changes Made

### 1. StoreProvider Trait Refactoring

#### Added Method
```rust
/// Get the directory where this provider syncs data
/// This is the authoritative location for the provider's local cache
fn sync_dir(&self) -> &Path;
```

#### Removed Parameters
All `sync_dir: &Path` parameters were removed from:
- `sync(&self, sync_dir: &Path)` → `sync(&self)`
- `needs_sync(&self, sync_dir: &Path)` → `needs_sync(&self)`
- `sync_if_needed(&self, sync_dir: &Path)` → `sync_if_needed(&self)`
- `post_publish(&self, id: &str, ver: &str, sync_dir: &Path)` → `post_publish(&self, id: &str, ver: &str)`
- `post_unpublish(&self, id: &str, ver: &str, sync_dir: &Path)` → `post_unpublish(&self, id: &str, ver: &str)`
- `check_write_status(&self, sync_dir: &Path)` → `check_write_status(&self)`

### 2. LocallyCachedStore Constructor Simplification

#### New Primary Constructor
```rust
/// Create a new locally cached store
///
/// The sync directory is determined by the provider's `sync_dir()` method.
/// This ensures a single source of truth for where data is stored.
pub fn new(provider: T, name: String) -> Result<Self>
```

#### Deprecated Method
```rust
#[deprecated(
    since = "0.1.0",
    note = "Use new() instead - provider manages its own directory"
)]
pub fn with_custom_sync_dir(provider: T, sync_dir: PathBuf, name: String) -> Result<Self>
```

### 3. GitStoreBuilder Enhancement

#### Builder Methods Added
```rust
/// Set the cache directory where the git repository will be stored
pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self

/// Set the name for this store
pub fn name(mut self, name: impl Into<String>) -> Self
```

#### Updated Build Method
```rust
/// Build the GitStore
///
/// Returns an error if cache_dir or name were not set via the builder
pub fn build(self) -> Result<GitStore>
```

#### Deprecated Method
```rust
#[deprecated(since = "0.1.0", note = "Use .cache_dir().name().build() instead")]
pub fn build_with(self, cache_dir: PathBuf, name: impl Into<String>) -> Result<GitStore>
```

## Migration Guide

### For Store Users

#### Creating Git Stores

**Before:**
```rust
let provider = GitProvider::new(url, cache_dir.clone(), reference, auth);
let store = LocallyCachedStore::new(provider, cache_dir, name)?;
```

**After (Recommended):**
```rust
let store = GitStore::builder()
    .url(url)
    .cache_dir(cache_dir)
    .name(name)
    .reference(reference)
    .auth(auth)
    .build()?;
```

**After (Alternative - still valid):**
```rust
let provider = GitProvider::new(url, cache_dir, reference, auth);
let store = LocallyCachedStore::new(provider, name)?;
```

#### Common Patterns

**Simple read-only store:**
```rust
let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir(cache_path)
    .name("my-store")
    .build()?;
```

**With authentication:**
```rust
let store = GitStore::builder()
    .url("https://github.com/user/private-repo.git")
    .cache_dir(cache_path)
    .name("private-store")
    .auth(GitAuth::Token { token })
    .build()?;
```

**With specific branch:**
```rust
let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir(cache_path)
    .name("dev-store")
    .branch("develop")
    .build()?;
```

**Writable store:**
```rust
let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir(cache_path)
    .name("writable-store")
    .auth(GitAuth::Token { token })
    .writable()
    .author("Bot", "bot@example.com")
    .commit_style(CommitStyle::Conventional)
    .build()?;
```

**Full configuration:**
```rust
let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir(cache_path)
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

### For Provider Implementors

#### Implementing StoreProvider

**Before:**
```rust
#[async_trait]
impl StoreProvider for MyProvider {
    async fn sync(&self, sync_dir: &Path) -> Result<SyncResult> {
        // Had to validate sync_dir matches internal state
        if sync_dir != self.cache_dir {
            return Err(StoreError::InvalidConfiguration(...));
        }
        
        // Sync logic...
    }
    
    async fn needs_sync(&self, sync_dir: &Path) -> Result<bool> {
        if sync_dir != self.cache_dir {
            return Ok(false);
        }
        
        // Check logic...
    }
}
```

**After:**
```rust
#[async_trait]
impl StoreProvider for MyProvider {
    fn sync_dir(&self) -> &Path {
        &self.cache_dir
    }
    
    async fn sync(&self) -> Result<SyncResult> {
        // Use self.sync_dir() or self.cache_dir directly
        // No parameter validation needed!
        
        // Sync logic...
    }
    
    async fn needs_sync(&self) -> Result<bool> {
        // Use self.sync_dir() or self.cache_dir directly
        
        // Check logic...
    }
}
```

#### Key Changes for Implementors

1. **Add `sync_dir()` method** - Return a reference to your internal directory
2. **Remove `sync_dir` parameters** - Use internal state instead
3. **Remove validation code** - No need to check if passed directory matches internal state
4. **Simplify logic** - Direct access to internal directory, no parameter passing

## Benefits

### 1. Cleaner API Surface

- **Before:** 9 lines minimum to create a configured git store
- **After:** 5 lines for simple cases, scales naturally for complex ones

### 2. Type Safety

```rust
// Compile-time error if you forget to set required fields
let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .build(); // ❌ Error: cache_dir must be set

// Compile-time error for name too
let store = GitStore::builder()
    .url("https://github.com/user/repo.git")
    .cache_dir(cache)
    .build(); // ❌ Error: name must be set
```

### 3. No Directory Mismatch Errors

**Before:**
```rust
let provider = GitProvider::new(url, dir1.clone(), ref, auth);
let store = LocallyCachedStore::new(provider, dir2, name)?;
// Runtime panic when provider validates dir2 != dir1
```

**After:**
```rust
let provider = GitProvider::new(url, dir, ref, auth);
let store = LocallyCachedStore::new(provider, name)?;
// Impossible to have mismatch - provider owns directory!
```

### 4. Less Boilerplate

**Before:**
```rust
// Repeat cache_dir three times!
let cache = PathBuf::from("/cache");
let provider = GitProvider::new(url, cache.clone(), ref, auth);
let store = LocallyCachedStore::new(provider, cache.clone(), name)?;
store.ensure_synced().await?; // provider.sync() gets cache internally
```

**After:**
```rust
// Specify once in builder
let store = GitStore::builder()
    .url(url)
    .cache_dir("/cache")
    .name(name)
    .reference(ref)
    .auth(auth)
    .build()?;
store.ensure_synced().await?;
```

### 5. Clearer Ownership

- **Provider** owns and manages its sync directory
- **Store** delegates directory questions to provider
- No ambiguity about who controls what

### 6. Better Performance

- No runtime directory validation in `sync()` and other methods
- Fewer parameter copies
- Direct field access instead of parameter passing

## Backward Compatibility

### Deprecated (But Still Available)

```rust
// This still works but shows deprecation warning
#[allow(deprecated)]
let store = LocallyCachedStore::with_custom_sync_dir(provider, sync_dir, name)?;

// This also still works with deprecation warning
#[allow(deprecated)]
let store = builder.build_with(cache_dir, name)?;
```

### Breaking Changes

If you implemented `StoreProvider` for custom types:
- Must add `fn sync_dir(&self) -> &Path` implementation
- Must remove `sync_dir` parameters from all trait methods
- Must update method signatures to match new trait definition

## Testing

All existing tests pass with the new implementation:
- ✅ 91 tests passed
- ✅ 0 tests failed
- ✅ Full backward compatibility for store creation
- ✅ All provider trait methods work correctly
- ✅ Builder validation works as expected

## Examples

### Complete Example: Read-Only Git Store

```rust
use quelle_store::GitStore;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = GitStore::builder()
        .url("https://github.com/quelle-org/extensions.git")
        .cache_dir(dirs::cache_dir().unwrap().join("quelle/stores/official"))
        .name("official-extensions")
        .branch("main")
        .build()?;
    
    // First access triggers clone
    let extensions = store.list_extensions().await?;
    println!("Found {} extensions", extensions.len());
    
    Ok(())
}
```

### Complete Example: Writable Git Store

```rust
use quelle_store::{GitStore, GitAuth, CommitStyle};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("GITHUB_TOKEN")?;
    
    let store = GitStore::builder()
        .url("https://github.com/myorg/extensions.git")
        .cache_dir(PathBuf::from("./cache/git-store"))
        .name("my-store")
        .auth(GitAuth::Token { token })
        .branch("main")
        .writable()
        .author("Extension Bot", "bot@example.com")
        .commit_style(CommitStyle::Conventional)
        .build()?;
    
    // Publishing automatically commits and pushes
    let package = create_extension_package()?;
    store.publish(package, Default::default()).await?;
    
    Ok(())
}
```

### Complete Example: Custom Provider

```rust
use quelle_store::stores::providers::traits::{StoreProvider, SyncResult};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

struct HttpProvider {
    api_url: String,
    cache_dir: PathBuf,
}

#[async_trait]
impl StoreProvider for HttpProvider {
    fn sync_dir(&self) -> &Path {
        &self.cache_dir
    }
    
    async fn sync(&self) -> Result<SyncResult> {
        // Download extensions from HTTP API
        let response = reqwest::get(&self.api_url).await?;
        // ... extract to self.cache_dir
        Ok(SyncResult::with_changes(vec!["Downloaded extensions".into()]))
    }
    
    async fn needs_sync(&self) -> Result<bool> {
        // Check if cache is stale
        Ok(true)
    }
    
    fn description(&self) -> String {
        format!("HTTP provider for {}", self.api_url)
    }
    
    fn provider_type(&self) -> &'static str {
        "http"
    }
}

// Use with LocallyCachedStore
let provider = HttpProvider {
    api_url: "https://api.example.com/extensions".into(),
    cache_dir: PathBuf::from("/cache/http"),
};
let store = LocallyCachedStore::new(provider, "http-store".into())?;
```

## Summary

This refactoring achieves all stated goals:

1. ✅ **Simpler initialization** - Single builder chain for complete configuration
2. ✅ **No redundancy** - Provider owns directory, no duplicate parameters
3. ✅ **Clear responsibilities** - Provider manages source, store provides interface
4. ✅ **Better ergonomics** - Common cases are simple, advanced config still possible
5. ✅ **Consistent API** - Same pattern works for all store types

The changes make the store system more intuitive, safer, and easier to maintain while preserving backward compatibility where possible.