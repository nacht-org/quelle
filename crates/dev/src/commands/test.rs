//! Interactive testing commands for extensions

use eyre::Result;
use url::Url;

use crate::server::DevServer;
use crate::utils::find_extension_path;

/// Start interactive testing session for an extension
pub async fn start_interactive(
    extension_name: String,
    url: Option<Url>,
    query: Option<String>,
) -> Result<()> {
    println!("Starting interactive test session for '{}'", extension_name);

    let extension_path = find_extension_path(&extension_name)?;
    let mut dev_server = DevServer::new(extension_name.clone(), extension_path, false).await?;

    println!("Building extension...");
    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    // If URL is provided, test novel info immediately
    if let Some(url) = url {
        println!("Testing novel info for: {}", url);
        test_novel_info_with_url(&mut dev_server, &url.to_string()).await?;
    }

    // If query is provided, test search immediately
    if let Some(query) = query {
        println!("Testing search for: '{}'", query);
        test_search_with_query(&mut dev_server, &query).await?;
    }

    // Start interactive shell for additional testing
    start_test_shell(dev_server).await
}

/// Start an interactive testing shell
async fn start_test_shell(mut dev_server: DevServer) -> Result<()> {
    use rustyline::{DefaultEditor, error::ReadlineError};

    println!();
    println!("Interactive testing session ready!");
    println!("Type 'help' for available commands, 'quit' to exit.");
    println!();

    let mut rl = DefaultEditor::new()?;

    loop {
        match rl.readline("test> ") {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(line)?;

                match line {
                    "quit" | "exit" | "q" => {
                        println!("👋 Goodbye!");
                        break;
                    }
                    "help" | "h" => {
                        print_test_help();
                    }
                    "clear" | "cls" => {
                        print!("\x1B[2J\x1B[1;1H"); // Clear screen
                    }
                    "meta" | "info" => {
                        show_extension_info(&dev_server).await?;
                    }
                    cmd if cmd.starts_with("novel ") => {
                        let url = cmd.strip_prefix("novel ").unwrap();
                        test_novel_info_with_url(&mut dev_server, url).await?;
                    }
                    cmd if cmd.starts_with("chapter ") => {
                        let url = cmd.strip_prefix("chapter ").unwrap();
                        test_chapter_with_url(&mut dev_server, url).await?;
                    }
                    cmd if cmd.starts_with("search ") => {
                        let query = cmd.strip_prefix("search ").unwrap();
                        test_search_with_query(&mut dev_server, query).await?;
                    }
                    "rebuild" => {
                        println!("Rebuilding...");
                        if let Err(e) = dev_server.build_extension().await {
                            println!("Error: Build failed: {}", e);
                        } else if let Err(e) = dev_server.load_extension().await {
                            println!("Error: Load failed: {}", e);
                        } else {
                            println!("Success: Rebuild successful");
                        }
                    }
                    _ => {
                        println!(
                            "❓ Unknown command: '{}'. Type 'help' for available commands.",
                            line
                        );
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("👋 Goodbye!");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

/// Test novel info fetching with a specific URL
async fn test_novel_info_with_url(dev_server: &DevServer, url: &str) -> Result<()> {
    println!("Testing novel info for: {}", url);

    // Validate URL first
    if let Err(e) = url::Url::parse(url) {
        println!("Error: Invalid URL: {}", e);
        return Ok(());
    }

    // Get WASM path and create runner
    let project_root = crate::utils::find_project_root(dev_server.extension_path())?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", dev_server.extension_name()));

    if !wasm_path.exists() {
        println!(
            "Error: Extension WASM file not found: {}",
            wasm_path.display()
        );
        println!("   Run 'just build {}' first", dev_server.extension_name());
        return Ok(());
    }

    let runner = dev_server
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

/// Test chapter content fetching with a specific URL
async fn test_chapter_with_url(dev_server: &DevServer, url: &str) -> Result<()> {
    println!("Testing chapter content for: {}", url);

    // Validate URL first
    if let Err(e) = url::Url::parse(url) {
        println!("Error: Invalid URL: {}", e);
        return Ok(());
    }

    // Get WASM path and create runner
    let project_root = crate::utils::find_project_root(dev_server.extension_path())?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", dev_server.extension_name()));

    if !wasm_path.exists() {
        println!(
            "Error: Extension WASM file not found: {}",
            wasm_path.display()
        );
        println!("   Run 'just build {}' first", dev_server.extension_name());
        return Ok(());
    }

    let runner = dev_server
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
                        println!(
                            "      {}",
                            if trimmed.len() > 80 {
                                format!("{}...", &trimmed[..80])
                            } else {
                                trimmed.to_string()
                            }
                        );
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

/// Test search functionality with a query
async fn test_search_with_query(dev_server: &DevServer, query: &str) -> Result<()> {
    println!("Testing search for: '{}'", query);

    if query.trim().is_empty() {
        println!("Error: Search query cannot be empty");
        return Ok(());
    }

    // Get WASM path and create runner
    let project_root = crate::utils::find_project_root(dev_server.extension_path())?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", dev_server.extension_name()));

    if !wasm_path.exists() {
        println!(
            "Error: Extension WASM file not found: {}",
            wasm_path.display()
        );
        println!("   Run 'just build {}' first", dev_server.extension_name());
        return Ok(());
    }

    let runner = dev_server
        .engine()
        .new_runner_from_file(wasm_path.to_str().unwrap())
        .await?;

    println!("Searching for '{}'...", query);
    let search_query = quelle_engine::bindings::quelle::extension::novel::SimpleSearchQuery {
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

/// Show extension metadata and information
async fn show_extension_info(dev_server: &DevServer) -> Result<()> {
    println!("Extension Information:");

    // Get WASM path and create runner
    let project_root = crate::utils::find_project_root(dev_server.extension_path())?;
    let wasm_path = project_root
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("extension_{}.wasm", dev_server.extension_name()));

    if !wasm_path.exists() {
        println!(
            "Error: Extension WASM file not found: {}",
            wasm_path.display()
        );
        println!("   Run 'just build {}' first", dev_server.extension_name());
        return Ok(());
    }

    let runner = dev_server
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

/// Print help text for testing commands
fn print_test_help() {
    println!("Available testing commands:");
    println!("  help, h                    - Show this help");
    println!("  novel <url>                - Test novel info fetching from URL");
    println!("  chapter <url>              - Test chapter content fetching from URL");
    println!("  search <query>             - Test search functionality with query");
    println!("  meta, info                 - Show extension metadata");
    println!("  rebuild                    - Rebuild and reload the extension");
    println!("  clear, cls                 - Clear screen");
    println!("  quit, exit, q              - Exit the test session");
    println!();
    println!("Examples:");
    println!("  novel https://example.com/novel/123");
    println!("  chapter https://example.com/chapter/456");
    println!("  search mystery adventure");
}
