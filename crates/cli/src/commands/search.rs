use eyre::Result;
use quelle_engine::ExtensionEngine;
use quelle_store::StoreManager;
use tracing::{info, warn};

pub async fn handle_search_command(
    query: String,
    author: Option<String>,
    tags: Option<String>,
    source: Option<String>,
    limit: usize,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!(
            "Would search for: {} (author: {:?}, tags: {:?}, source: {:?}, limit: {})",
            query, author, tags, source, limit
        );
        return Ok(());
    }

    // Initialize the extension engine and store manager
    let storage_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".quelle");

    let registry_dir = storage_path.join("extensions");
    let registry_store =
        Box::new(quelle_store::registry::LocalRegistryStore::new(&registry_dir).await?);
    let store_manager = StoreManager::new(registry_store).await?;

    let http_executor = std::sync::Arc::new(quelle_engine::http::ReqwestExecutor::new());
    let engine = ExtensionEngine::new(http_executor)?;

    println!("🔍 Searching for novels: {}", query);

    // Get all installed extensions
    let installed_extensions = store_manager.list_installed().await?;

    if installed_extensions.is_empty() {
        println!("❌ No extensions installed");
        println!("💡 Install extensions first with: quelle extension install <id>");
        return Ok(());
    }

    println!(
        "📦 Searching across {} installed extensions...",
        installed_extensions.len()
    );

    let mut total_results = 0;
    let mut searched_extensions = 0;
    let mut failed_extensions = 0;

    // Search each extension
    for extension in &installed_extensions {
        // Filter by source if specified
        if let Some(ref source_filter) = source {
            if !extension
                .name
                .to_lowercase()
                .contains(&source_filter.to_lowercase())
                && !extension
                    .id
                    .to_lowercase()
                    .contains(&source_filter.to_lowercase())
            {
                continue;
            }
        }

        info!("🔍 Searching extension: {}", extension.name);

        match search_extension_for_novels(
            &extension,
            &query,
            author.as_ref(),
            tags.as_ref(),
            limit,
            &engine,
        )
        .await
        {
            Ok(results) => {
                if !results.is_empty() {
                    println!("\n📚 Results from {} ({}):", extension.name, extension.id);
                    for (_i, novel) in results.iter().enumerate() {
                        if total_results >= limit {
                            break;
                        }
                        println!("  {}. 📖 {}", total_results + 1, novel.title);
                        println!("     🔗 {}", novel.url);
                        if let Some(ref cover) = novel.cover {
                            println!("     🎨 Cover: {}", cover);
                        }
                        total_results += 1;
                    }
                }
                searched_extensions += 1;
            }
            Err(e) => {
                warn!("❌ Failed to search {}: {}", extension.name, e);
                failed_extensions += 1;
            }
        }

        if total_results >= limit {
            break;
        }
    }

    // Summary
    println!("\n📊 Search Summary:");
    println!("  🔍 Query: {}", query);
    if let Some(ref author_filter) = author {
        println!("  👤 Author filter: {}", author_filter);
    }
    if let Some(ref tags_filter) = tags {
        println!("  🏷️  Tags filter: {}", tags_filter);
    }
    if let Some(ref source_filter) = source {
        println!("  📦 Source filter: {}", source_filter);
    }
    println!("  📚 Total results: {}", total_results);
    println!("  📦 Extensions searched: {}", searched_extensions);
    if failed_extensions > 0 {
        println!("  ❌ Extensions failed: {}", failed_extensions);
    }

    if total_results == 0 && searched_extensions > 0 {
        println!("\n💡 No results found. Try:");
        println!("  • Different search terms");
        println!("  • Checking if extensions support your query");
        println!("  • Installing more extensions");
    }

    if total_results > 0 {
        println!("\n💡 To fetch a novel, use:");
        println!("  quelle fetch novel <url>");
    }

    Ok(())
}

async fn search_extension_for_novels(
    extension: &quelle_store::models::InstalledExtension,
    query: &str,
    author: Option<&String>,
    tags: Option<&String>,
    limit: usize,
    engine: &ExtensionEngine,
) -> Result<Vec<quelle_engine::bindings::quelle::extension::novel::BasicNovel>> {
    use quelle_engine::bindings::{ComplexSearchQuery, SimpleSearchQuery};

    // Get WASM component bytes
    let wasm_bytes = extension.get_wasm_bytes();

    // Create runner
    let runner = engine.new_runner_from_bytes(wasm_bytes).await?;

    // Try simple search first
    let simple_query = SimpleSearchQuery {
        query: query.to_string(),
        page: Some(1),
        limit: Some(limit as u32),
    };

    let (runner, result) = runner.simple_search(&simple_query).await?;

    match result {
        Ok(search_result) => {
            // Filter results by author if specified
            let mut novels = search_result.novels;

            if let Some(author_filter) = author {
                novels.retain(|novel| {
                    // Check if any of the novel's metadata contains the author
                    // For BasicNovel, we only have title, cover, and url
                    // So we'll do a simple check against the URL or title
                    novel
                        .title
                        .to_lowercase()
                        .contains(&author_filter.to_lowercase())
                        || novel
                            .url
                            .to_lowercase()
                            .contains(&author_filter.to_lowercase())
                });
            }

            // Filter by tags if specified (limited without full novel info)
            if let Some(tags_filter) = tags {
                let tag_list: Vec<String> = tags_filter
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .collect();
                novels.retain(|novel| {
                    // Simple tag matching against title and URL
                    tag_list.iter().any(|tag| {
                        novel.title.to_lowercase().contains(tag)
                            || novel.url.to_lowercase().contains(tag)
                    })
                });
            }

            Ok(novels)
        }
        Err(wit_error) => {
            // If simple search fails, try complex search if we have filters
            if author.is_some() || tags.is_some() {
                let complex_query = ComplexSearchQuery {
                    filters: vec![], // TODO: Implement proper filter construction
                    page: Some(1),
                    limit: Some(limit as u32),
                    sort_by: None,
                    sort_order: None,
                };

                let (_, result) = runner.complex_search(&complex_query).await?;
                match result {
                    Ok(search_result) => Ok(search_result.novels),
                    Err(_) => Err(eyre::eyre!(
                        "Both simple and complex search failed: {:?}",
                        wit_error
                    )),
                }
            } else {
                Err(eyre::eyre!("Search failed: {:?}", wit_error))
            }
        }
    }
}
