//! Novel identifier resolution helpers.

use eyre::Result;
use quelle_storage::{
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};
use url::Url;

/// Resolve a user-supplied input string to a `NovelId` in storage.
///
/// Accepts:
/// - `"all"` → returns `None` (caller interprets as "every novel")
/// - An exact novel ID
/// - A URL (looks up by URL, then maps to stored ID)
/// - A case-insensitive exact title match
/// - A partial title match (only when unambiguous)
///
/// When multiple novels match, prints the candidates and returns `None`.
pub async fn resolve_novel_id(input: &str, storage: &dyn BookStorage) -> Result<Option<NovelId>> {
    if input == "all" {
        return Ok(None);
    }

    // Try exact ID first.
    let direct_id = NovelId::new(input.to_string());
    if storage.exists_novel(&direct_id).await.unwrap_or(false) {
        return Ok(Some(direct_id));
    }

    // Try URL lookup.
    if let Ok(_url) = Url::parse(input) {
        if let Ok(Some(novel_id)) = storage.find_novel_id_by_url(input).await {
            return Ok(Some(novel_id));
        }
    }

    let novels = storage.list_novels(&NovelFilter::default()).await?;

    // Case-insensitive exact title match.
    if let Some(novel) = novels
        .iter()
        .find(|n| n.title.to_lowercase() == input.to_lowercase())
    {
        return Ok(Some(novel.id.clone()));
    }

    // Partial title match.
    let input_lower = input.to_lowercase();
    let matches: Vec<_> = novels
        .iter()
        .filter(|n| n.title.to_lowercase().contains(&input_lower))
        .collect();

    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches[0].id.clone())),
        _ => {
            println!("Multiple novels match '{}':", input);
            for novel in matches.iter().take(10) {
                println!("  {} - {}", novel.id.as_str(), novel.title);
            }
            if matches.len() > 10 {
                println!("  ... and {} more", matches.len() - 10);
            }
            println!("Please use a more specific query or an exact ID.");
            Ok(None)
        }
    }
}

/// Print a helpful message when a novel cannot be found for `input`.
pub async fn show_novel_not_found_help(input: &str, storage: &dyn BookStorage) {
    eprintln!("Not found: '{}'", input);
    eprintln!("Novels can be identified by ID, URL, or title (partial match allowed).");

    if let Ok(novels) = storage.list_novels(&NovelFilter::default()).await {
        if novels.is_empty() {
            eprintln!("Library is empty. Use 'quelle add <url>' to add a novel.");
        } else {
            eprintln!("Library (first 3):");
            for novel in novels.iter().take(3) {
                eprintln!("  {} - {}", novel.id.as_str(), novel.title);
            }
            if novels.len() > 3 {
                eprintln!("  ... and {} more", novels.len() - 3);
            }
            eprintln!("Run 'quelle library list' to see all novels.");
        }
    }
}
