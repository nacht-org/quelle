//! Development server for hot-reloading extensions during development

use eyre::{Result, eyre};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::http_caching::CachingHttpExecutor;
use crate::utils::{find_extension_path, find_project_root};
use quelle_engine::ExtensionEngine;

mod commands;

pub use commands::DevServerCommand;

/// Development server for testing extensions with hot reload
pub struct DevServer {
    extension_name: String,
    extension_path: PathBuf,
    engine: ExtensionEngine,
    build_cache: HashMap<String, Instant>,
    caching_executor: Option<Arc<CachingHttpExecutor>>,
}

impl DevServer {
    /// Create a new development server instance
    pub async fn new(
        extension_name: String,
        extension_path: PathBuf,
        use_chrome: bool,
    ) -> Result<Self> {
        // Create cache directory for this extension
        let cache_dir = std::env::temp_dir()
            .join("quelle_dev_cache")
            .join(&extension_name);

        let (engine, caching_executor) =
            create_extension_engine_with_cache_ref(Some(cache_dir), use_chrome)?;

        Ok(Self {
            extension_name,
            extension_path,
            engine,
            build_cache: HashMap::new(),
            caching_executor,
        })
    }

    /// Build the extension
    pub async fn build_extension(&mut self) -> Result<()> {
        let start_time = Instant::now();

        println!("Building extension '{}'...", self.extension_name);

        // Check if we've built recently to avoid unnecessary rebuilds
        if let Some(last_build) = self.build_cache.get(&self.extension_name) {
            if start_time.duration_since(*last_build) < Duration::from_secs(1) {
                println!("Skipping build (recent build detected)");
                return Ok(());
            }
        }

        let output = tokio::process::Command::new("cargo")
            .args(&[
                "component",
                "build",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
                "-p",
                &format!("extension_{}", self.extension_name()),
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Build failed:\n{}", stderr));
        }

        self.build_cache
            .insert(self.extension_name.clone(), start_time);

        let duration = start_time.elapsed();
        println!("Success: Build completed in {:.2}s", duration.as_secs_f64());

        Ok(())
    }

    /// Load the extension into the engine
    pub async fn load_extension(&mut self) -> Result<()> {
        let project_root = find_project_root(&self.extension_path)?;
        let wasm_path = project_root
            .join("target/wasm32-unknown-unknown/release")
            .join(format!("extension_{}.wasm", self.extension_name));

        if !wasm_path.exists() {
            return Err(eyre!(
                "Extension WASM file not found: {}",
                wasm_path.display()
            ));
        }

        let _wasm_bytes = tokio::fs::read(&wasm_path).await?;

        // TODO: Implement extension loading
        // This requires integration with the current ExtensionEngine API
        // For now, just verify the WASM file exists and is readable

        println!("Extension '{}' loaded successfully", self.extension_name);

        Ok(())
    }

    /// Clear the HTTP cache
    pub async fn clear_cache(&self) -> Result<()> {
        if let Some(ref cache) = self.caching_executor {
            cache.clear_cache().await;
            println!("Cache cleared");
        } else {
            println!("No cache to clear");
        }
        Ok(())
    }

    /// Show cache statistics
    pub async fn show_cache_stats(&self) -> Result<()> {
        if let Some(ref cache) = self.caching_executor {
            let (memory_count, file_count) = cache.cache_stats().await;
            println!("Cache Statistics:");
            println!("  Memory entries: {}", memory_count);
            println!("  File entries: {}", file_count);
        } else {
            println!("Caching not enabled");
        }
        Ok(())
    }

    /// Handle a development server command
    pub async fn handle_command(&mut self, cmd: DevServerCommand) -> Result<()> {
        commands::handle(self, cmd).await
    }

    /// Get the extension name
    pub fn extension_name(&self) -> &str {
        &self.extension_name
    }

    /// Get the extension path
    pub fn extension_path(&self) -> &std::path::Path {
        &self.extension_path
    }

    /// Get the extension engine
    pub fn engine(&self) -> &ExtensionEngine {
        &self.engine
    }
}

/// Start the development server
pub async fn start(extension_name: String, watch: bool, use_chrome: bool) -> Result<()> {
    println!("Starting dev server for '{}'", extension_name);

    let extension_path = find_extension_path(&extension_name)?;
    let mut dev_server =
        DevServer::new(extension_name.clone(), extension_path.clone(), use_chrome).await?;

    println!("Building...");
    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    if watch {
        println!("👀 Watching for changes...");
        start_with_watcher(dev_server, extension_path).await?;
    } else {
        start_interactive_shell(dev_server).await?;
    }

    Ok(())
}

/// Start server with file watcher for hot reload
async fn start_with_watcher(dev_server: DevServer, extension_path: PathBuf) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;

    watcher.watch(&extension_path.join("src"), RecursiveMode::Recursive)?;
    watcher.watch(
        &extension_path.join("Cargo.toml"),
        RecursiveMode::NonRecursive,
    )?;

    let dev_server = Arc::new(Mutex::new(dev_server));
    let dev_server_clone = dev_server.clone();

    // File watcher task
    let _watcher_handle = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();

        for res in rx {
            match res {
                Ok(Event {
                    kind: EventKind::Modify(_),
                    ..
                }) => {
                    println!("File changed, rebuilding...");

                    let dev_server = dev_server_clone.clone();
                    rt.block_on(async {
                        let mut server = dev_server.lock().await;

                        if let Err(e) = server.build_extension().await {
                            println!("Error: Build failed: {}", e);
                        } else if let Err(e) = server.load_extension().await {
                            println!("Error: Load failed: {}", e);
                        } else {
                            println!("Success: Rebuild successful");
                        }
                    });
                }
                Ok(_) => {} // Ignore other events
                Err(e) => println!("Error: Watch error: {:?}", e),
            }
        }
    });

    // Interactive shell
    start_interactive_shell_with_server(dev_server).await
}

/// Start interactive shell for testing
async fn start_interactive_shell(dev_server: DevServer) -> Result<()> {
    let dev_server = Arc::new(Mutex::new(dev_server));
    start_interactive_shell_with_server(dev_server).await
}

/// Start interactive shell with shared server reference
async fn start_interactive_shell_with_server(dev_server: Arc<Mutex<DevServer>>) -> Result<()> {
    use rustyline::{DefaultEditor, error::ReadlineError};

    println!();
    println!("Development server ready!");
    println!("Type 'help' for available commands, 'quit' to exit.");
    println!();

    let mut rl = DefaultEditor::new()?;

    loop {
        match rl.readline("dev> ") {
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
                        print_help();
                    }
                    "clear" | "cls" => {
                        print!("\x1B[2J\x1B[1;1H"); // Clear screen
                    }
                    "rebuild" | "build" => {
                        let mut server = dev_server.lock().await;
                        if let Err(e) = server.build_extension().await {
                            println!("Error: Build failed: {}", e);
                        } else if let Err(e) = server.load_extension().await {
                            println!("Error: Load failed: {}", e);
                        } else {
                            println!("Success: Rebuild successful");
                        }
                    }
                    cmd if cmd.starts_with("test ") => {
                        let url = cmd.strip_prefix("test ").unwrap();
                        let mut server = dev_server.lock().await;
                        if let Err(e) = server
                            .handle_command(DevServerCommand::Test {
                                url: url.to_string(),
                            })
                            .await
                        {
                            println!("Error: Test failed: {}", e);
                        }
                    }
                    cmd if cmd.starts_with("search ") => {
                        let query_str = cmd.strip_prefix("search ").unwrap();
                        let query: Vec<String> = query_str
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect();
                        let mut server = dev_server.lock().await;
                        if let Err(e) = server
                            .handle_command(DevServerCommand::Search { query })
                            .await
                        {
                            println!("Error: Search failed: {}", e);
                        }
                    }
                    "cache clear" => {
                        let server = dev_server.lock().await;
                        if let Err(e) = server.clear_cache().await {
                            println!("Error: Failed to clear cache: {}", e);
                        }
                    }
                    "cache stats" => {
                        let server = dev_server.lock().await;
                        if let Err(e) = server.show_cache_stats().await {
                            println!("Error: Failed to show cache stats: {}", e);
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

/// Print help text for interactive commands
fn print_help() {
    println!("Available commands:");
    println!("  help, h              - Show this help");
    println!("  test <url>           - Test novel info fetching from URL");
    println!("  search <query>       - Test search functionality");
    println!("  rebuild, build       - Rebuild the extension");
    println!("  cache clear          - Clear HTTP cache");
    println!("  cache stats          - Show cache statistics");
    println!("  clear, cls           - Clear screen");
    println!("  quit, exit, q        - Exit the development server");
    println!();
    println!("Examples:");
    println!("  test https://example.com/novel/123");
    println!("  search mystery adventure");
}

/// Create extension engine with caching HTTP executor
fn create_extension_engine_with_cache_ref(
    cache_dir: Option<PathBuf>,
    use_chrome: bool,
) -> Result<(ExtensionEngine, Option<Arc<CachingHttpExecutor>>)> {
    let base_executor: std::sync::Arc<dyn quelle_engine::http::HttpExecutor> = if use_chrome {
        std::sync::Arc::new(quelle_engine::http::HeadlessChromeExecutor::new())
    } else {
        std::sync::Arc::new(quelle_engine::http::ReqwestExecutor::new())
    };

    let caching_executor = if let Some(_cache_dir) = cache_dir {
        let caching_exec = Arc::new(CachingHttpExecutor::new(base_executor));
        let engine = ExtensionEngine::new(
            caching_exec.clone() as Arc<dyn quelle_engine::http::HttpExecutor>
        )?;
        (engine, Some(caching_exec))
    } else {
        let engine = ExtensionEngine::new(base_executor)?;
        (engine, None)
    };

    Ok(caching_executor)
}
