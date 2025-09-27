//! Publishing with Store Manifest Demo
//!
//! This example demonstrates how the StoreManifest is automatically updated
//! when extensions are published to and unpublished from a store.
//!
//! Run with: cargo run --example publish_with_manifest

use quelle_store::{
    local::LocalStore,
    manifest::{
        checksum::{Checksum, ChecksumAlgorithm},
        ExtensionManifest,
    },
    models::ExtensionPackage,
    publish::{PublishOptions, PublishableStore, UnpublishOptions},
    Store,
};

use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📚 Publishing with Store Manifest Demo");
    println!("====================================");

    // Create a temporary directory for our store
    let temp_dir = TempDir::new()?;
    let mut store = LocalStore::new(temp_dir.path())?;

    println!("\n📦 Created local store at: {}", temp_dir.path().display());

    // Check initial manifest state
    println!("\n🔍 Initial store manifest:");
    let manifest = store.get_store_manifest().await?;
    println!("  - Extension count: {}", manifest.extension_count);
    println!("  - Supported domains: {:?}", manifest.supported_domains);
    println!("  - URL patterns: {}", manifest.url_patterns.len());

    // Create a sample extension package
    println!("\n📝 Creating extension package...");

    let manifest = ExtensionManifest {
        name: "novel-scraper".to_string(),
        version: "1.0.0".to_string(),
        author: "Demo Author".to_string(),
        langs: vec!["en".to_string()],
        base_urls: vec![
            "https://novels.example.com".to_string(),
            "https://lightnovels.world".to_string(),
        ],
        rds: vec![],
        attrs: vec![],
        checksum: Checksum {
            algorithm: ChecksumAlgorithm::Blake3,
            value: "demo-checksum-value".to_string(),
        },
        signature: None,
    };

    let package = ExtensionPackage::new(manifest, b"fake-wasm-data".to_vec(), "local".to_string());

    println!(
        "  ✅ Created package: {}@{}",
        package.manifest.name, package.manifest.version
    );
    println!("  📍 Base URLs: {:?}", package.manifest.base_urls);

    // Publish the extension
    println!("\n🚀 Publishing extension...");
    let mut publish_options = PublishOptions::default();
    publish_options.skip_validation = true; // Skip validation for demo
    let result = store.publish_extension(package, &publish_options).await?;

    println!("  ✅ Published successfully!");
    println!("  📝 Publication ID: {}", result.publication_id);
    println!("  📊 Package size: {} bytes", result.package_size);

    // Check manifest after publishing
    println!("\n📊 Store manifest after publishing:");
    let manifest = store.get_store_manifest().await?;
    println!("  - Extension count: {}", manifest.extension_count);
    println!("  - Supported domains: {:?}", manifest.supported_domains);
    println!("  - Extensions:");
    for ext in &manifest.extensions {
        println!(
            "    • {}@{} (langs: {:?})",
            ext.name, ext.version, ext.langs
        );
        println!("      Base URLs: {:?}", ext.base_urls);
    }

    // Test URL matching
    println!("\n🎯 Testing URL matching after publish:");
    let test_urls = vec![
        "https://novels.example.com/book/123",
        "https://lightnovels.world/novel/456",
        "https://unknown-site.com/content",
    ];

    for url in &test_urls {
        let matches = manifest.find_extensions_for_url(url);
        if matches.is_empty() {
            println!("    ❌ {}: No extensions found", url);
        } else {
            println!("    ✅ {}: Found {:?}", url, matches);
        }
    }

    // Create and publish a second extension
    println!("\n📝 Creating second extension package...");

    let manifest2 = ExtensionManifest {
        name: "manga-reader".to_string(),
        version: "2.1.0".to_string(),
        author: "Manga Dev".to_string(),
        langs: vec!["en".to_string(), "ja".to_string()],
        base_urls: vec![
            "https://manga.site.com".to_string(),
            "https://scanlation.net".to_string(),
        ],
        rds: vec![],
        attrs: vec![],
        checksum: Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: "manga-checksum-value".to_string(),
        },
        signature: None,
    };

    let package2 = ExtensionPackage::new(
        manifest2,
        b"fake-manga-wasm-data".to_vec(),
        "local".to_string(),
    );

    println!(
        "  ✅ Created package: {}@{}",
        package2.manifest.name, package2.manifest.version
    );

    let _result2 = store.publish_extension(package2, &publish_options).await?;
    println!("  ✅ Published second extension!");

    // Check manifest with multiple extensions
    println!("\n📊 Store manifest with multiple extensions:");
    let manifest = store.get_store_manifest().await?;
    println!("  - Extension count: {}", manifest.extension_count);
    println!("  - Supported domains: {:?}", manifest.supported_domains);

    println!("\n🎯 Testing URL matching with multiple extensions:");
    let test_urls = vec![
        "https://novels.example.com/book/123",
        "https://manga.site.com/chapter/456",
        "https://scanlation.net/manga/789",
        "https://random-site.com/content",
    ];

    for url in &test_urls {
        let matches = manifest.find_extensions_for_url(url);
        if matches.is_empty() {
            println!("    ❌ {}: No extensions found", url);
        } else {
            println!("    ✅ {}: Found {:?}", url, matches);
        }
    }

    // Demonstrate unpublishing
    println!("\n🗑️ Unpublishing first extension...");
    let unpublish_options = UnpublishOptions {
        access_token: None,
        reason: Some("Demo unpublish".to_string()),
        keep_record: false,
        notify_users: false,
    };
    let unpublish_result = store
        .unpublish_extension("novel-scraper", "1.0.0", &unpublish_options)
        .await?;

    println!("  ✅ Unpublished at: {}", unpublish_result.unpublished_at);

    // Check manifest after unpublishing
    println!("\n📊 Store manifest after unpublishing:");
    let manifest = store.get_store_manifest().await?;
    println!("  - Extension count: {}", manifest.extension_count);
    println!("  - Supported domains: {:?}", manifest.supported_domains);
    println!("  - Remaining extensions:");
    for ext in &manifest.extensions {
        println!("    • {}@{}", ext.name, ext.version);
    }

    println!("\n🎯 URL matching after unpublish:");
    let test_urls = vec![
        "https://novels.example.com/book/123", // Should no longer match
        "https://manga.site.com/chapter/456",  // Should still match
    ];

    for url in &test_urls {
        let matches = manifest.find_extensions_for_url(url);
        if matches.is_empty() {
            println!("    ❌ {}: No extensions found", url);
        } else {
            println!("    ✅ {}: Found {:?}", url, matches);
        }
    }

    // Show the manifest file on disk
    let manifest_path = temp_dir.path().join("store.json");
    if manifest_path.exists() {
        println!(
            "\n💾 Store manifest file exists at: {}",
            manifest_path.display()
        );

        // Read and show the JSON content
        let content = std::fs::read_to_string(&manifest_path)?;
        let lines: Vec<&str> = content.lines().take(20).collect();
        println!("📄 First 20 lines of store.json:");
        for (i, line) in lines.iter().enumerate() {
            println!("  {:2}: {}", i + 1, line);
        }
        if content.lines().count() > 20 {
            println!("  ... ({} more lines)", content.lines().count() - 20);
        }
    } else {
        println!("\n❌ No manifest file found on disk");
    }

    println!("\n🎉 Publishing demo completed!");
    println!("\n💡 Key takeaways:");
    println!(
        "  • StoreManifest is automatically updated when extensions are published/unpublished"
    );
    println!("  • URL patterns are generated from extension base_urls");
    println!("  • Supported domains are automatically extracted and maintained");
    println!("  • The manifest provides fast URL-to-extension matching");
    println!("  • All changes are persisted to store.json on disk");

    Ok(())
}
