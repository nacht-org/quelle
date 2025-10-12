//! Search command handlers for finding novels across installed extension sources.

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
    page: Option<u32>,
    advanced: bool,
    simple: bool,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!(
            "Would search for: {} (author: {:?}, tags: {:?}, categories: {:?}, limit: {:?}, page: {:?}, advanced: {}, simple: {})",
            query, author, tags, categories, limit, page, advanced, simple
        );
        return Ok(());
    }

    let use_complex_search = if simple {
        false
    } else {
        // Use complex search if we have any filters or advanced mode is requested
        advanced || !tags.is_empty() || !categories.is_empty() || author.is_some()
    };

    if use_complex_search {
        println!("Using complex search with filters");
    } else {
        println!("Using simple search");
    }
    let mut search_query = SearchQuery::new();

    if !query.is_empty() {
        search_query = search_query.with_text(query.clone());
    }

    if let Some(author) = author {
        search_query = search_query.with_author(author);
    }

    if !tags.is_empty() {
        search_query = search_query.with_tags(tags);
    }

    if let Some(limit) = limit {
        search_query = search_query.limit(limit);
    } else {
        search_query = search_query.limit(20);
    }

    if page.is_some() && page != Some(1) {
        println!("Pagination not yet fully supported, showing first page");
    }

    println!("Searching...");

    match store_manager
        .search_novels_with_installed_extensions(&search_query)
        .await
    {
        Ok(results) => {
            display_search_results(&results, &query);
        }
        Err(e) => {
            warn!("Search failed: {}", e);
            println!("Search failed: {}", e);
        }
    }

    Ok(())
}

fn display_search_results(
    results: &[quelle_engine::bindings::quelle::extension::novel::BasicNovel],
    query: &str,
) {
    if results.is_empty() {
        println!("No results found for: \"{}\"", query);
        return;
    }

    println!("Found {} result(s):", results.len());
    println!();

    for (i, result) in results.iter().enumerate() {
        println!("{}. {}", i + 1, result.title);
        println!("   {}", result.url);

        if let Some(cover) = &result.cover {
            println!("   Cover: {}", cover);
        }

        println!();
    }

    if results.len() >= 20 {
        println!(
            "Showing first {} results. Use --limit to adjust.",
            results.len()
        );
    }
}
