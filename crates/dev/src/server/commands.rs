//! Command handling for the development server

use clap::Parser;
use eyre::{Result, eyre};
use quelle_engine::bindings::quelle::extension::novel::SimpleSearchQuery;

use url::Url;

use super::DevServer;

/// CLI arguments for the development server
#[derive(Parser, Debug)]
pub struct DevServerCli {
    /// Development server command to execute
    #[clap(subcommand)]
    pub command: DevServerCommand,
}

/// Commands available in the development server
#[derive(Parser, Debug)]
pub enum DevServerCommand {
    /// Test novel info fetching
    Test {
        /// URL to test
        url: String,
    },
    /// Test search functionality
    Search {
        /// Search query (multiple words allowed)
        #[arg(trailing_var_arg = true)]
        query: Vec<String>,
    },
    /// Test chapter content fetching
    Chapter {
        /// Chapter URL to test
        url: String,
    },
    /// Show extension metadata
    Meta,
    /// Clear HTTP cache
    CacheClear,
    /// Show cache statistics
    CacheStats,
    /// Quit the development server
    Quit,
}

/// Handle a development server command
pub async fn handle(server: &mut DevServer, cmd: DevServerCommand) -> Result<()> {
    match cmd {
        DevServerCommand::Test { url } => test_novel_info(server, &url).await,
        DevServerCommand::Search { query } => test_search(server, &query).await,
        DevServerCommand::Chapter { url } => test_chapter_content(server, &url).await,
        DevServerCommand::Meta => show_extension_meta(server).await,
        DevServerCommand::CacheClear => server.clear_cache().await,
        DevServerCommand::CacheStats => server.show_cache_stats().await,
        DevServerCommand::Quit => {
            println!("Quitting development server...");
            server.stop();
            Ok(())
        }
    }
}

/// Test novel info fetching from a URL
async fn test_novel_info(server: &DevServer, url: &str) -> Result<()> {
    println!("Testing novel info for: {}", url);

    // Validate URL format
    let parsed_url = Url::parse(url).map_err(|e| eyre!("Invalid URL '{}': {}", url, e))?;

    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(eyre!("URL must use HTTP or HTTPS protocol"));
    }

    println!("Fetching novel information...");

    // Get WASM path and create runner
    let project_root = crate::utils::find_project_root(server.extension_path())?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", server.extension_name()));

    if !wasm_path.exists() {
        println!(
            "Error: Extension WASM file not found: {}",
            wasm_path.display()
        );
        println!("   Run 'just build {}' first", server.extension_name());
        return Ok(());
    }

    let runner = server
        .engine()
        .new_runner_from_file(wasm_path.to_str().unwrap())
        .await?;

    println!("Fetching novel info...");
    let (_, result) = runner.fetch_novel_info(url).await?;

    match result {
        Ok(novel) => {
            println!("Success: Novel info fetched successfully!");
            println!("   Title: {}", novel.title);
            println!(
                "   Authors: {}",
                if novel.authors.is_empty() {
                    "Unknown".to_string()
                } else {
                    novel.authors.join(", ")
                }
            );
            let description_text = novel.description.join(" ");
            println!(
                "   Description: {}",
                if description_text.len() > 100 {
                    format!("{}...", &description_text[..100])
                } else {
                    description_text
                }
            );
            println!("   Status: {:?}", novel.status);
            if !novel.langs.is_empty() {
                println!("   Languages: {}", novel.langs.join(", "));
            }
            if !novel.volumes.is_empty() {
                let total_chapters: usize = novel.volumes.iter().map(|v| v.chapters.len()).sum();
                println!(
                    "   Volumes: {}, Chapters: {}",
                    novel.volumes.len(),
                    total_chapters
                );
            }
            if let Some(chapter) = novel.volumes.first().and_then(|v| v.chapters.first()) {
                println!("   First Chapter: {}", chapter.title);
                println!("      URL: {}", chapter.url);
            }
        }
        Err(e) => {
            println!("Error: Failed to fetch novel info: {}", e.message);
            if let Some(location) = e.location {
                println!("   Location: {}", location);
            }
        }
    }

    Ok(())
}

/// Test search functionality
async fn test_search(server: &DevServer, query_parts: &[String]) -> Result<()> {
    if query_parts.is_empty() {
        return Err(eyre!("Search query cannot be empty"));
    }

    let query_string = query_parts.join(" ");
    println!("Testing search for: '{}'", query_string);

    // Create search query
    let _search_query = SimpleSearchQuery {
        query: query_string.clone(),
        page: Some(1),
        limit: None,
    };

    println!("Executing search...");

    // Get WASM path and create runner
    let project_root = crate::utils::find_project_root(server.extension_path())?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", server.extension_name()));

    if !wasm_path.exists() {
        println!(
            "Error: Extension WASM file not found: {}",
            wasm_path.display()
        );
        println!("   Run 'just build {}' first", server.extension_name());
        return Ok(());
    }

    let runner = server
        .engine()
        .new_runner_from_file(wasm_path.to_str().unwrap())
        .await?;

    let query = query_parts.join(" ");
    println!("Searching for '{}'...", query);
    let search_query = SimpleSearchQuery {
        query: query.to_string(),
        page: Some(1),
        limit: Some(10),
    };

    let (_, result) = runner.simple_search(&search_query).await?;

    match result {
        Ok(search_result) => {
            println!("Success: Search completed successfully!");
            println!("   Found {} novels", search_result.novels.len());

            for (i, novel) in search_result.novels.iter().enumerate() {
                println!("   {}. {}", i + 1, novel.title);
                println!("      URL: {}", novel.url);
                if let Some(cover) = &novel.cover {
                    println!("      Cover: {}", cover);
                }
            }

            println!("   Has more pages: {}", search_result.has_next_page);
        }
        Err(e) => {
            println!("Error: Search failed: {}", e.message);
            if let Some(location) = e.location {
                println!("   Location: {}", location);
            }
        }
    }

    Ok(())
}

/// Test chapter content fetching
async fn test_chapter_content(server: &DevServer, url: &str) -> Result<()> {
    println!("Testing chapter content for: {}", url);

    // Validate URL format
    let parsed_url = Url::parse(url).map_err(|e| eyre!("Invalid URL '{}': {}", url, e))?;

    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(eyre!("URL must use HTTP or HTTPS protocol"));
    }

    println!("Fetching chapter content...");

    // Get WASM path and create runner
    let project_root = crate::utils::find_project_root(server.extension_path())?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", server.extension_name()));

    if !wasm_path.exists() {
        println!(
            "Error: Extension WASM file not found: {}",
            wasm_path.display()
        );
        println!("   Run 'just build {}' first", server.extension_name());
        return Ok(());
    }

    let runner = server
        .engine()
        .new_runner_from_file(wasm_path.to_str().unwrap())
        .await?;

    println!("Fetching chapter content...");
    let (_, result) = runner.fetch_chapter(url).await?;

    match result {
        Ok(chapter) => {
            println!("Success: Chapter content fetched successfully!");
            println!("   Content length: {} characters", chapter.data.len());

            // Show first few lines of content
            let preview_lines: Vec<&str> = chapter.data.lines().take(3).collect();
            if !preview_lines.is_empty() {
                println!("   Preview:");
                for line in preview_lines {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        println!("      {trimmed}",);
                    }
                }
            }
        }
        Err(e) => {
            println!("Error: Failed to fetch chapter content: {}", e.message);
            if let Some(location) = e.location {
                println!("   Location: {}", location);
            }
        }
    }

    Ok(())
}

/// Show extension metadata
async fn show_extension_meta(server: &DevServer) -> Result<()> {
    println!("Extension Metadata:");

    // Get WASM path and create runner
    let project_root = crate::utils::find_project_root(server.extension_path())?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", server.extension_name()));

    if !wasm_path.exists() {
        println!(
            "Error: Extension WASM file not found: {}",
            wasm_path.display()
        );
        println!("   Run 'just build {}' first", server.extension_name());
        return Ok(());
    }

    let runner = server
        .engine()
        .new_runner_from_file(wasm_path.to_str().unwrap())
        .await?;

    let (_, meta) = runner.meta().await?;

    println!("   ID: {}", meta.id);
    println!("   Name: {}", meta.name);
    println!("   Base URLs: {:?}", meta.base_urls);
    println!("   Languages: {:?}", meta.langs);
    println!("   Version: {}", meta.version);

    println!("   Reading Directions: {:?}", meta.rds);
    println!("   Attributes: {:?}", meta.attrs);

    if let Some(search_caps) = meta.capabilities.search {
        println!("   Search Support:");
        println!("      Simple: {}", search_caps.supports_simple_search);
        println!("      Complex: {}", search_caps.supports_complex_search);
    }

    Ok(())
}
