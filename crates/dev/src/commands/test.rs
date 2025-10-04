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
    println!(
        "🧪 Starting interactive test session for '{}'",
        extension_name
    );

    let extension_path = find_extension_path(&extension_name)?;
    let mut dev_server = DevServer::new(extension_name.clone(), extension_path, false).await?;

    println!("🔨 Building extension...");
    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    // If URL is provided, test novel info immediately
    if let Some(url) = url {
        println!("🔍 Testing novel info for: {}", url);
        test_novel_info_with_url(&mut dev_server, &url.to_string()).await?;
    }

    // If query is provided, test search immediately
    if let Some(query) = query {
        println!("🔍 Testing search for: '{}'", query);
        test_search_with_query(&mut dev_server, &query).await?;
    }

    // Start interactive shell for additional testing
    start_test_shell(dev_server).await
}

/// Start an interactive testing shell
async fn start_test_shell(mut dev_server: DevServer) -> Result<()> {
    use rustyline::{DefaultEditor, error::ReadlineError};

    println!();
    println!("🎯 Interactive testing session ready!");
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
                        println!("🔨 Rebuilding...");
                        if let Err(e) = dev_server.build_extension().await {
                            println!("❌ Build failed: {}", e);
                        } else if let Err(e) = dev_server.load_extension().await {
                            println!("❌ Load failed: {}", e);
                        } else {
                            println!("✅ Rebuild successful");
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
                println!("❌ Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

/// Test novel info fetching with a specific URL
async fn test_novel_info_with_url(_dev_server: &mut DevServer, url: &str) -> Result<()> {
    println!("📖 Testing novel info for: {}", url);

    // Validate URL first
    if let Err(e) = url::Url::parse(url) {
        println!("❌ Invalid URL: {}", e);
        return Ok(());
    }

    // TODO: Implement actual novel info testing
    // This requires integration with the ExtensionRunner API
    println!("⚠️  Novel info testing not yet implemented");
    println!("   URL would be: {}", url);

    Ok(())
}

/// Test chapter content fetching with a specific URL
async fn test_chapter_with_url(_dev_server: &mut DevServer, url: &str) -> Result<()> {
    println!("📄 Testing chapter content for: {}", url);

    // Validate URL first
    if let Err(e) = url::Url::parse(url) {
        println!("❌ Invalid URL: {}", e);
        return Ok(());
    }

    // TODO: Implement actual chapter testing
    // This requires integration with the ExtensionRunner API
    println!("⚠️  Chapter content testing not yet implemented");
    println!("   URL would be: {}", url);

    Ok(())
}

/// Test search functionality with a query
async fn test_search_with_query(_dev_server: &mut DevServer, query: &str) -> Result<()> {
    println!("🔍 Testing search for: '{}'", query);

    if query.trim().is_empty() {
        println!("❌ Search query cannot be empty");
        return Ok(());
    }

    // TODO: Implement actual search testing
    // This requires integration with the ExtensionRunner API
    println!("⚠️  Search testing not yet implemented");
    println!("   Query would be: '{}'", query);

    Ok(())
}

/// Show extension metadata and information
async fn show_extension_info(_dev_server: &DevServer) -> Result<()> {
    println!("📋 Extension Information:");

    // TODO: Get actual extension metadata
    // This requires integration with the ExtensionRunner API
    println!("⚠️  Extension info display not yet implemented");

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
