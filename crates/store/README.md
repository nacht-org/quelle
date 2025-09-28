# Quelle Store

Extension package management system for the Quelle e-book scraper.

> **⚠️ This crate is under active development and APIs may change significantly.**

## Documentation

For comprehensive documentation, please see the [Quelle Book](../../book/):

- [Store Overview](../../book/src/store/overview.md)
- [Store Management](../../book/src/store/management.md) 
- [Extension Management](../../book/src/store/extensions.md)
- [CLI Reference](../../book/src/store/cli-reference.md)
- [Development Guide](../../book/src/development/store-implementation.md)

## Quick Example

```rust
use quelle_store::{StoreManager, local::LocalStore};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = StoreManager::new(
        PathBuf::from("./extensions"),
        PathBuf::from("./cache")
    ).await?;

    let local_store = LocalStore::new("./local-repo")?;
    manager.add_store(local_store);

    let installed = manager.install("extension-name", None, None).await?;
    println!("Installed: {}@{}", installed.name, installed.version);

    Ok(())
}
```

## Development Status

- ✅ Core Store trait and LocalStore implementation
- ✅ Extension installation and management
- ✅ CLI integration
- 🔄 Git and HTTP store backends (planned)
- 🔄 Advanced dependency resolution (planned)

For the latest information, see the project documentation.