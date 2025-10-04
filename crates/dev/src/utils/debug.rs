//! Debug utilities for displaying extension data in development

use quelle_engine::bindings::quelle::extension::{
    novel::{ChapterContent, Novel, SearchResult},
    source::SourceMeta,
};

/// Display novel information in a formatted way
pub fn display_novel_info(novel: &Novel) {
    println!("Novel Info:");
    println!("  Title: {}", novel.title);
    println!("  Authors: {:?}", novel.authors);
    println!("  Status: {:?}", novel.status);
    println!("  Languages: {:?}", novel.langs);
    println!("  Volumes: {}", novel.volumes.len());

    for (i, volume) in novel.volumes.iter().enumerate() {
        println!("  Volume {}: {} chapters", i, volume.chapters.len());
    }

    if !novel.description.is_empty() {
        println!("  Description: {}", novel.description.join(" "));
    }

    if let Some(ref cover) = novel.cover {
        println!("  Cover: {}", cover);
    }

    if !novel.metadata.is_empty() {
        println!("  Metadata: {} items", novel.metadata.len());
    }
}

/// Display search results in a formatted way
pub fn display_search_results(query: &str, results: &SearchResult) {
    println!("Search Results for '{}':", query);
    println!("  Found: {} novels", results.novels.len());
    println!("  Page: {}", results.current_page);

    if let Some(total_pages) = results.total_pages {
        println!("  Total pages: {}", total_pages);
    }

    println!("  Has next: {}", results.has_next_page);

    for (i, novel) in results.novels.iter().take(5).enumerate() {
        println!("  {}. {} - {}", i + 1, novel.title, novel.url);
    }

    if results.novels.len() > 5 {
        println!("  ... and {} more", results.novels.len() - 5);
    }
}

/// Display chapter content with preview
pub fn display_chapter_content(url: &str, content: &ChapterContent) {
    println!("Chapter Content from {}:", url);
    println!("  Length: {} characters", content.data.len());

    let preview = if content.data.len() > 200 {
        format!("{}...", &content.data[..197])
    } else {
        content.data.clone()
    };
    println!("  Preview: {}", preview.replace('\n', " "));
}

/// Display extension metadata
pub fn display_extension_meta(meta: &SourceMeta) {
    println!("Extension Metadata:");
    println!("  ID: {}", meta.id);
    println!("  Name: {}", meta.name);
    println!("  Version: {}", meta.version);
    println!("  Languages: {:?}", meta.langs);
    println!("  Base URLs: {:?}", meta.base_urls);
    println!("  Reading Directions: {:?}", meta.rds);

    if let Some(search_caps) = &meta.capabilities.search {
        println!("  Search Capabilities:");
        println!("    Simple search: {}", search_caps.supports_simple_search);
        println!(
            "    Complex search: {}",
            search_caps.supports_complex_search
        );
    }
}
