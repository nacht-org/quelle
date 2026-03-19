//! Interactive testing commands for extensions

use eyre::Result;
use url::Url;

use crate::server::{DevServer, Executor};
use crate::utils::find_extension_path;

/// Start interactive testing session for an extension
pub async fn start_interactive(
    extension_name: String,
    url: Option<Url>,
    query: Option<String>,
    executor: Executor,
) -> Result<()> {
    println!("Starting interactive test session for '{}'", extension_name);

    let extension_path = find_extension_path(&extension_name)?;
    let mut dev_server = DevServer::new(extension_name.clone(), extension_path, executor).await?;

    println!("Building extension...");
    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    // If URL is provided, test novel info immediately
    if let Some(url) = url {
        println!("Testing novel info for: {}", url);
        crate::server::commands::test_novel_info(&dev_server, url.as_ref()).await?;
    }

    // If query is provided, test search immediately
    if let Some(query) = query {
        println!("Testing search for: '{}'", query);
        crate::server::commands::test_search(&dev_server, &[query]).await?;
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
                        crate::server::commands::show_extension_meta(&dev_server).await?;
                    }
                    cmd if cmd.starts_with("novel ") => {
                        let url = cmd.strip_prefix("novel ").unwrap();
                        crate::server::commands::test_novel_info(&dev_server, url).await?;
                    }
                    cmd if cmd.starts_with("chapter ") => {
                        let url = cmd.strip_prefix("chapter ").unwrap();
                        crate::server::commands::test_chapter_content(&dev_server, url).await?;
                    }
                    cmd if cmd.starts_with("search ") => {
                        let query = cmd.strip_prefix("search ").unwrap();
                        crate::server::commands::test_search(&dev_server, &[query.to_string()])
                            .await?;
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
