//! Search command handlers for finding novels across extension sources.

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
                println!("No results found for: {}", query);
            } else {
                let display_limit = limit.unwrap_or(10);
                println!("Found {} results:", results.len());

                for (i, result) in results.iter().enumerate().take(display_limit) {
                    println!("{}. {} by {}", i + 1, result.name, result.author);
                    if let Some(homepage) = &result.homepage {
                        println!("   {}", homepage);
                    }
                }

                if results.len() > display_limit {
                    println!("... {} more results", results.len() - display_limit);
                }
            }
        }
        Err(e) => {
            warn!("Search failed: {}", e);
            println!("Search failed: {}", e);
        }
    }

    Ok(())
}
