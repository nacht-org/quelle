//! Git Store Demo
//!
//! This example demonstrates how to use the new GitStore and LocalProvider
//! functionality to create stores that sync from different sources.

use quelle_store::{GitAuth, GitProvider, GitReference, StoreProvider};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging (commented out to avoid dependency)
    // tracing_subscriber::init();

    println!("🚀 Quelle Store Provider Demo");
    println!("==============================\n");

    // Create temporary directories for our examples
    let temp_dir = TempDir::new()?;
    let base_path = temp_dir.path();

    // Demo 1: Git Provider (using a public repository)
    println!("🔧 Demo 1: Git Provider");
    println!("------------------------");
    demo_git_provider(&base_path.join("git_demo")).await?;

    // Demo 2: GitStore convenience methods
    println!("\n🎯 Demo 2: GitStore Convenience Methods");
    println!("----------------------------------------");
    demo_git_store_methods(&base_path.join("git_store_demo")).await?;

    println!("\n✅ All demos completed successfully!");
    Ok(())
}

async fn demo_git_provider(demo_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    // Create cache directory
    std::fs::create_dir_all(demo_dir)?;

    // Create a GitProvider (using a small public repo as example)
    let git_url = "https://github.com/octocat/Hello-World.git".to_string();
    let provider = GitProvider::new(
        git_url.clone(),
        demo_dir.to_path_buf(),
        GitReference::Default,
        GitAuth::None,
    );

    println!("  🌐 Created GitProvider: {}", provider.description());
    println!("  🔄 Provider type: {}", provider.provider_type());
    println!("  📍 Repository URL: {}", provider.url());
    println!("  📂 Cache directory: {}", provider.cache_dir().display());

    // Check if sync is needed (should be true initially)
    let needs_sync = provider.needs_sync(demo_dir).await?;
    println!("  🔍 Needs sync: {}", needs_sync);

    if needs_sync {
        println!("  📥 Syncing repository (this may take a moment)...");

        // This would normally clone the repository, but we'll skip it in the demo
        // to avoid network dependencies. In a real scenario:
        // let sync_result = provider.sync(demo_dir).await?;
        // println!("  ✨ Sync result: updated={}, changes={}", sync_result.updated, sync_result.changes.len());

        println!("  ⏭️  Skipping actual git clone for demo purposes");
    }

    Ok(())
}

async fn demo_git_store_methods(
    demo_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(demo_dir)?;

    println!("  🎯 Creating GitStore with different methods:");

    // Method 1: Basic GitStore
    let _store1_dir = demo_dir.join("store1");
    println!("  📦 GitStore::from_url()");
    // let store1 = GitStore::from_url(
    //     "basic-store".to_string(),
    //     "https://github.com/example/repo.git".to_string(),
    //     store1_dir,
    // )?;
    println!("    ✅ Would create basic git store");

    // Method 2: GitStore with authentication
    let _store2_dir = demo_dir.join("store2");
    println!("  🔐 GitStore::with_auth()");
    let _auth = GitAuth::Token {
        token: "fake-token".to_string(),
    };
    // let store2 = GitStore::with_auth(
    //     "auth-store".to_string(),
    //     "https://github.com/private/repo.git".to_string(),
    //     store2_dir,
    //     auth,
    // )?;
    println!("    ✅ Would create git store with token auth");

    // Method 3: GitStore with specific branch
    let _store3_dir = demo_dir.join("store3");
    println!("  🌿 GitStore::with_branch()");
    // let store3 = GitStore::with_branch(
    //     "branch-store".to_string(),
    //     "https://github.com/example/repo.git".to_string(),
    //     store3_dir,
    //     "develop".to_string(),
    // )?;
    println!("    ✅ Would create git store tracking 'develop' branch");

    // Method 4: GitStore with specific tag
    let _store4_dir = demo_dir.join("store4");
    println!("  🏷️  GitStore::with_tag()");
    // let store4 = GitStore::with_tag(
    //     "tag-store".to_string(),
    //     "https://github.com/example/repo.git".to_string(),
    //     store4_dir,
    //     "v1.0.0".to_string(),
    // )?;
    println!("    ✅ Would create git store pinned to tag 'v1.0.0'");

    // Method 5: Fully customized GitStore
    println!("  ⚙️  GitStore::with_config() - Full customization");
    let _custom_auth = GitAuth::SshKey {
        private_key_path: PathBuf::from("~/.ssh/id_rsa"),
        public_key_path: None,
        passphrase: None,
    };
    let _custom_reference = GitReference::Commit("abc123def456".to_string());

    // let custom_store = GitStore::with_config(
    //     "custom-store".to_string(),
    //     "git@github.com:private/repo.git".to_string(),
    //     demo_dir.join("custom"),
    //     custom_reference,
    //     custom_auth,
    //     std::time::Duration::from_secs(1800), // 30 minutes
    //     false, // Not shallow
    // )?;
    println!("    ✅ Would create fully customized git store");

    println!("\n  🎉 All GitStore creation methods demonstrated!");

    Ok(())
}
