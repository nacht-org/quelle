//! Store Manifest Demo
//!
//! This example demonstrates the StoreManifest functionality and URL matching
//! capabilities that were added to the Quelle store system.
//!
//! Run with: cargo run --example store_manifest_demo

use chrono::Utc;
use quelle_store::{ExtensionSummary, StoreManifest, UrlPattern};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Store Manifest Demo");
    println!("====================");

    // Create a new store manifest
    let mut manifest = StoreManifest::new(
        "example-store".to_string(),
        "local".to_string(),
        "1.0.0".to_string(),
    );

    println!("\n📦 Created store manifest for: {}", manifest.store_name);

    println!("✨ Created manifest with basic metadata");

    // Add URL patterns for fast matching
    println!("\n🔗 Adding URL patterns...");

    manifest.add_url_pattern(
        "https://novels.example.com".to_string(),
        vec!["novel-scraper".to_string(), "backup-novel-ext".to_string()],
        10, // High priority
    );

    manifest.add_url_pattern(
        "https://manga.site.com".to_string(),
        vec!["manga-reader".to_string()],
        8,
    );

    manifest.add_url_pattern(
        "https://webtoon.platform.net".to_string(),
        vec!["webtoon-scraper".to_string()],
        6,
    );

    // Add some extensions with base URLs for fallback matching
    println!("📚 Adding extension summaries...");

    let novel_ext = ExtensionSummary {
        name: "advanced-novel-scraper".to_string(),
        version: "2.1.0".to_string(),
        base_urls: vec![
            "https://lightnovels.world".to_string(),
            "https://webnovels.site".to_string(),
        ],
        langs: vec!["en".to_string(), "zh".to_string()],
        last_updated: Utc::now(),
    };
    manifest.add_extension(novel_ext);

    let manga_ext = ExtensionSummary {
        name: "universal-manga-reader".to_string(),
        version: "1.5.3".to_string(),
        base_urls: vec![
            "https://manga-hub.org".to_string(),
            "https://scanlation.net".to_string(),
        ],
        langs: vec!["en".to_string(), "ja".to_string(), "ko".to_string()],
        last_updated: Utc::now(),
    };
    manifest.add_extension(manga_ext);

    let webtoon_ext = ExtensionSummary {
        name: "mobile-webtoon-viewer".to_string(),
        version: "3.0.1".to_string(),
        base_urls: vec!["https://mobile-webtoons.app".to_string()],
        langs: vec!["en".to_string(), "kr".to_string()],
        last_updated: Utc::now(),
    };
    manifest.add_extension(webtoon_ext);

    println!("📊 Manifest stats:");
    println!("  - Extension count: {}", manifest.extension_count);
    println!("  - Supported domains: {:?}", manifest.supported_domains);
    println!("  - URL patterns: {}", manifest.url_patterns.len());

    // Demonstrate URL matching
    println!("\n🎯 Testing URL matching...");

    let test_urls = vec![
        "https://novels.example.com/book/12345",
        "https://manga.site.com/chapter/67890",
        "https://webtoon.platform.net/series/abcdef",
        "https://lightnovels.world/novel/fantasy-adventure",
        "https://scanlation.net/manga/action-series",
        "https://mobile-webtoons.app/webtoon/romance-story",
        "https://unknown-site.com/content/random",
    ];

    for test_url in &test_urls {
        let matches = manifest.find_extensions_for_url(test_url);
        if matches.is_empty() {
            println!("  ❌ {}: No extensions found", test_url);
        } else {
            println!("  ✅ {}: Found extensions: {:?}", test_url, matches);
        }
    }

    // Demonstrate domain-specific queries
    println!("\n🌐 Testing domain-specific queries...");

    let test_domains = vec![
        "lightnovels.world",
        "manga-hub.org",
        "mobile-webtoons.app",
        "unknown-domain.com",
    ];

    for domain in &test_domains {
        let extensions = manifest.extensions_for_domain(domain);
        if extensions.is_empty() {
            println!("  ❌ {}: No extensions support this domain", domain);
        } else {
            println!("  ✅ {}: Supported by extensions: {:?}", domain, extensions);
        }
    }

    // Demonstrate priority ordering
    println!("\n⭐ URL pattern priorities (sorted by priority):");
    for (i, pattern) in manifest.url_patterns.iter().enumerate() {
        println!(
            "  {}. {} (priority: {}) -> {:?}",
            i + 1,
            pattern.url_prefix,
            pattern.priority,
            pattern.extensions
        );
    }

    // Demonstrate serialization
    println!("\n💾 Serializing manifest to JSON...");
    let json = serde_json::to_string_pretty(&manifest)?;
    println!("JSON size: {} bytes", json.len());

    // Show a snippet of the JSON
    let lines: Vec<&str> = json.lines().take(15).collect();
    println!("First 15 lines of JSON:");
    for line in lines {
        println!("  {}", line);
    }
    if json.lines().count() > 15 {
        println!("  ... ({} more lines)", json.lines().count() - 15);
    }

    // Test deserialization
    println!("\n🔄 Testing deserialization...");
    let deserialized: StoreManifest = serde_json::from_str(&json)?;
    println!("✅ Successfully deserialized manifest");
    println!("  - Store name: {}", deserialized.store_name);
    println!("  - Extension count: {}", deserialized.extension_count);
    println!("  - URL patterns: {}", deserialized.url_patterns.len());

    // Performance comparison simulation
    println!("\n⚡ Performance comparison simulation:");
    println!("Without StoreManifest:");
    println!("  - Must iterate through ALL extensions in ALL stores");
    println!("  - Must load and parse each extension manifest");
    println!("  - O(n) complexity where n = total extensions");
    println!();
    println!("With StoreManifest:");
    println!("  - Check pre-computed URL patterns first (O(p) where p = patterns)");
    println!("  - Only fallback to extension manifests if no pattern matches");
    println!("  - Patterns are sorted by priority for optimal matching");
    println!("  - Significant performance improvement for common URLs");

    println!("\n🎉 Demo completed successfully!");

    Ok(())
}
