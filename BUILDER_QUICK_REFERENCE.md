# Store Builder Quick Reference

Quick examples for using the new builder pattern in Quelle stores.

---

## Git Store

### Basic (Read-Only)
```rust
use quelle_store::GitStore;

let store = GitStore::builder("https://github.com/user/repo.git")
    .build(cache_dir, "my-store")?;
```

### With Authentication
```rust
use quelle_store::{GitStore, GitAuth};

let store = GitStore::builder("https://github.com/user/repo.git")
    .auth(GitAuth::Token { token: env::var("GITHUB_TOKEN")? })
    .build(cache_dir, "my-store")?;
```

### Writable (Auto-commit & Push)
```rust
let store = GitStore::builder("https://github.com/user/repo.git")
    .auth(GitAuth::Token { token })
    .writable()
    .build(cache_dir, "my-store")?;
```

### Writable with Custom Author
```rust
let store = GitStore::builder("https://github.com/user/repo.git")
    .auth(GitAuth::Token { token })
    .writable()
    .author("Bot Name", "bot@example.com")
    .build(cache_dir, "my-store")?;
```

### Specific Branch
```rust
let store = GitStore::builder("https://github.com/user/repo.git")
    .branch("develop")
    .build(cache_dir, "my-store")?;
```

### Specific Tag
```rust
let store = GitStore::builder("https://github.com/user/repo.git")
    .tag("v1.0.0")
    .build(cache_dir, "my-store")?;
```

### Specific Commit
```rust
let store = GitStore::builder("https://github.com/user/repo.git")
    .commit("abc123def")
    .build(cache_dir, "my-store")?;
```

### Custom Commit Style
```rust
use quelle_store::CommitStyle;

let store = GitStore::builder("https://github.com/user/repo.git")
    .auth(GitAuth::Token { token })
    .writable()
    .commit_style(CommitStyle::Detailed)  // or Minimal, Custom(fn)
    .build(cache_dir, "my-store")?;
```

### No Auto-Push (Local Commits Only)
```rust
let store = GitStore::builder("https://github.com/user/repo.git")
    .auth(GitAuth::Token { token })
    .writable()
    .no_auto_push()
    .build(cache_dir, "my-store")?;
```

### SSH Authentication
```rust
use quelle_store::GitAuth;
use std::path::PathBuf;

let store = GitStore::builder("git@github.com:user/repo.git")
    .auth(GitAuth::SshKey {
        private_key_path: PathBuf::from("/home/user/.ssh/id_rsa"),
        public_key_path: None,
        passphrase: None,
    })
    .writable()
    .build(cache_dir, "my-store")?;
```

### Full Configuration
```rust
use std::time::Duration;

let store = GitStore::builder("https://github.com/user/repo.git")
    .auth(GitAuth::Token { token })
    .branch("main")
    .writable()
    .author("CI Bot", "ci@example.com")
    .commit_style(CommitStyle::Detailed)
    .fetch_interval(Duration::from_secs(1800))
    .shallow(false)
    .build(cache_dir, "my-store")?;
```

---

## Local Store

### Basic
```rust
use quelle_store::stores::local::LocalStore;

let store = LocalStore::new("/path/to/store")?;
```

### With Custom Name
```rust
let store = LocalStore::builder("/path/to/store")
    .name("my-store")
    .build()?;
```

### Readonly
```rust
let store = LocalStore::builder("/path/to/store")
    .readonly()
    .build()?;
```

### No Cache
```rust
let store = LocalStore::builder("/path/to/store")
    .no_cache()
    .build()?;
```

### Enable/Disable Cache Explicitly
```rust
let store = LocalStore::builder("/path/to/store")
    .cache(false)  // or cache(true)
    .build()?;
```

### Full Configuration
```rust
use quelle_store::validation::create_strict_validator;

let store = LocalStore::builder("/path/to/store")
    .name("production-store")
    .readonly()
    .no_cache()
    .validator(create_strict_validator())
    .build()?;
```

---

## Authentication Options

### None (System Credentials)
```rust
GitAuth::None  // Uses SSH agent, git credential manager, etc.
```

### Personal Access Token
```rust
GitAuth::Token { 
    token: "ghp_xxxxxxxxxxxx".to_string() 
}
```

### SSH Key
```rust
GitAuth::SshKey {
    private_key_path: PathBuf::from("/home/user/.ssh/id_rsa"),
    public_key_path: Some(PathBuf::from("/home/user/.ssh/id_rsa.pub")),
    passphrase: Some("passphrase".to_string()),
}
```

### Username/Password
```rust
GitAuth::UserPassword {
    username: "user".to_string(),
    password: "password".to_string(),
}
```

---

## Commit Styles

### Default
```rust
CommitStyle::Default
// Output: "Publish extension_id v1.0.0"
```

### Detailed
```rust
CommitStyle::Detailed
// Output: "Publish extension extension_id version 1.0.0"
```

### Minimal
```rust
CommitStyle::Minimal
// Output: "Publish extension_id@1.0.0"
```

### Custom
```rust
CommitStyle::Custom(|action, ext_id, version| {
    format!("chore: {} {}@{}", action, ext_id, version)
})
// Output: "chore: Publish extension_id@1.0.0"
```

---

## Common Patterns

### Development Setup
```rust
// Local store for testing
let local = LocalStore::builder("./test-store")
    .name("dev-store")
    .build()?;

// Git store for reading published extensions
let git = GitStore::builder("https://github.com/org/extensions.git")
    .build(cache_dir, "upstream")?;
```

### Production Setup
```rust
// Readonly local store
let local = LocalStore::builder("/var/lib/extensions")
    .readonly()
    .build()?;

// Writable git store with CI credentials
let git = GitStore::builder("https://github.com/org/extensions.git")
    .auth(GitAuth::Token { token: env::var("CI_TOKEN")? })
    .writable()
    .author("CI Bot", "ci@company.com")
    .build(cache_dir, "upstream")?;
```

### CI/CD Setup
```rust
let auth = if env::var("CI").is_ok() {
    GitAuth::SshKey {
        private_key_path: PathBuf::from("/secrets/deploy_key"),
        public_key_path: None,
        passphrase: None,
    }
} else {
    GitAuth::Token { token: env::var("GITHUB_TOKEN")? }
};

let store = GitStore::builder(url)
    .auth(auth)
    .writable()
    .author("Deployment Bot", "deploy@company.com")
    .build(cache_dir, "production")?;
```

---

## Method Reference

### GitStoreBuilder

| Method | Description |
|--------|-------------|
| `.auth(GitAuth)` | Set authentication |
| `.branch(name)` | Checkout specific branch |
| `.tag(name)` | Checkout specific tag |
| `.commit(hash)` | Checkout specific commit |
| `.reference(GitReference)` | Set git reference directly |
| `.fetch_interval(Duration)` | How often to check for updates |
| `.shallow(bool)` | Enable/disable shallow cloning |
| `.writable()` | Enable write operations |
| `.author(name, email)` | Set commit author |
| `.commit_style(CommitStyle)` | Set commit message style |
| `.no_auto_push()` | Disable automatic pushing |
| `.build(cache, name)` | Build the store |

### LocalStoreBuilder

| Method | Description |
|--------|-------------|
| `.name(name)` | Set custom name |
| `.cache(bool)` | Enable/disable caching |
| `.no_cache()` | Disable caching |
| `.readonly()` | Set readonly mode |
| `.writable()` | Set writable mode |
| `.validator(engine)` | Set custom validator |
| `.build()` | Build the store |

---

## Tips

1. **Default behavior**: Most settings have sensible defaults
   - Caching: enabled
   - Auto-push: enabled (if writable)
   - Shallow clone: enabled
   - Branch: default branch

2. **Author fallback**: If no author specified, uses:
   1. Provided author
   2. Git config (`~/.gitconfig`)
   3. Default ("Quelle" <quelle@localhost>)

3. **System credentials**: `GitAuth::None` automatically uses:
   - SSH agent
   - Git credential manager
   - Stored credentials

4. **Backward compatibility**: `LocalStore::new()` still works

5. **Builder reuse**: Save builder for creating multiple stores:
   ```rust
   let base_builder = GitStore::builder(url)
       .auth(GitAuth::Token { token });
   
   let readonly = base_builder.clone().build(cache1, "readonly")?;
   let writable = base_builder.writable().build(cache2, "writable")?;
   ```

---

## Error Handling

```rust
use quelle_store::error::StoreError;

let store = GitStore::builder(url)
    .auth(auth)
    .build(cache_dir, name)
    .map_err(|e| match e {
        StoreError::InvalidConfiguration(msg) => {
            eprintln!("Invalid config: {}", msg);
            e
        }
        StoreError::IoError(io_err) => {
            eprintln!("IO error: {}", io_err);
            e
        }
        _ => e,
    })?;
```

---

## Migration from Old API

```rust
// OLD: GitStore::from_url()
GitStore::from_url("name".into(), url, cache)?
// NEW:
GitStore::builder(url).build(cache, "name")?

// OLD: GitStore::with_auth()
GitStore::with_auth("name".into(), url, cache, auth)?
// NEW:
GitStore::builder(url).auth(auth).build(cache, "name")?

// OLD: GitStore::with_branch()
GitStore::with_branch("name".into(), url, cache, "main".into())?
// NEW:
GitStore::builder(url).branch("main").build(cache, "name")?

// OLD: LocalStore::with_name()
LocalStore::with_name(path, "name".into())?
// NEW:
LocalStore::builder(path).name("name").build()?

// OLD: LocalStore with options
LocalStore::new(path)?.with_cache_disabled().with_readonly(true)
// NEW:
LocalStore::builder(path).no_cache().readonly().build()?
```

---

For detailed information, see:
- `GIT_STORE_REFACTORING.md`
- `LOCAL_STORE_REFACTORING.md`
- `STORE_REFACTORING_SUMMARY.md`
