use eyre::Result;
use quelle_store::{SearchQuery, StoreManager};
use tracing::warn;

pub async fn handle_search_command(
    store_manager: &StoreManager,
    query: String,
    author: Option<String>,
    tags: Vec<String>,
    categories: Vec<String>,
    limit: Option<usize>,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!(
            "Would search for: {} (author: {:?}, tags: {:?}, categories: {:?}, limit: {:?})",
            query, author, tags, categories, limit
        );
        return Ok(());
    }

    // Determine if we should use simple or complex search
    let is_complex = !tags.is_empty() || !categories.is_empty() || author.is_some();

    if is_complex {
        println!("🔍 Using complex search for: {}", query);
    } else {
        println!("🔍 Using simple search for: {}", query);
    }

    // Build search query
    let mut search_query = SearchQuery::new().with_text(query.clone());

    if let Some(author) = author {
        search_query = search_query.with_author(author);
    }

    if !tags.is_empty() {
        search_query = search_query.with_tags(tags);
    }

    if let Some(limit) = limit {
        search_query = search_query.limit(limit);
    } else {
        search_query = search_query.limit(20); // Default limit
    }

    // Search across all stores
    match store_manager.search_all_stores(&search_query).await {
        Ok(results) => {
            if results.is_empty() {
                println!("❌ No results found for: {}", query);
                println!("💡 Try:");
                println!("  • Different search terms");
                println!("  • Adding more extension stores with 'quelle store add'");
                println!("  • Checking if your stores have extensions installed");
            } else {
                let display_limit = limit.unwrap_or(10);
                println!("📚 Found {} results:", results.len());

                for (i, result) in results.iter().enumerate().take(display_limit) {
                    println!("{}. 📖 {} by {}", i + 1, result.name, result.author);

                    if let Some(desc) = &result.description {
                        let short_desc = if desc.len() > 150 {
                            format!("{}...", &desc[..147])
                        } else {
                            desc.clone()
                        };
                        println!("   {}", short_desc);
                    }

                    println!("   📦 Store: {}", result.store_source);

                    if let Some(homepage) = &result.homepage {
                        println!("   🔗 Homepage: {}", homepage);
                    }
                    println!();
                }

                if results.len() > display_limit {
                    println!("... and {} more results", results.len() - display_limit);
                    println!("💡 Use --limit {} to see more results", results.len());
                }

                println!("💡 To fetch a novel, use:");
                println!("  quelle fetch novel <url>");
            }
        }
        Err(e) => {
            warn!("Search failed: {}", e);
            println!("❌ Search failed: {}", e);
            println!("💡 Try:");
            println!("  • Checking store status with 'quelle status'");
            println!("  • Updating stores with 'quelle store update all'");
            println!("  • Adding extension stores if none are configured");
        }
    }

    Ok(())
}
