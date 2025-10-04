//! Development tools for Quelle extensions
//!
//! This crate provides development commands and utilities for building,
//! testing, and debugging Quelle extensions.

use clap::{Parser, Subcommand};
use eyre::{Result, eyre};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use quelle_engine::bindings::quelle::extension::{
    novel::{ChapterContent, Novel, SearchResult, SimpleSearchQuery},
    source::SourceMeta,
};
use quelle_engine::{ExtensionEngine, ExtensionRunner};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::debug;
use url::Url;

pub mod http_caching;
use http_caching::CachingHttpExecutor;

#[derive(Subcommand, Debug, Clone)]
pub enum DevCommands {
    /// Start development server with hot reload
    Server {
        /// Extension name to develop
        extension: String,
        /// Enable verbose logging
        #[arg(long, short)]
        verbose: bool,
        /// Auto-rebuild on file changes
        #[arg(long, default_value = "true")]
        watch: bool,
        /// Use Chrome HTTP executor instead of Reqwest (better for JS-heavy sites)
        #[arg(long, default_value = "true")]
        chrome: bool,
    },
    /// Interactive testing shell for extensions
    Test {
        /// Extension name to test
        extension: String,
        /// Test URL for novel info testing
        #[arg(long)]
        url: Option<Url>,
        /// Test search query
        #[arg(long)]
        query: Option<String>,
        /// Enable verbose logging
        #[arg(long, short)]
        verbose: bool,
    },
    /// Validate extension without publishing
    Validate {
        /// Extension name to validate
        extension: String,
        /// Run extended validation tests
        #[arg(long)]
        extended: bool,
    },
}

/// Internal dev server commands
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
    /// Force rebuild extension
    Rebuild,
    /// Clear HTTP cache
    ClearCache,
    /// Show cache statistics
    CacheStats,
    /// Exit development server
    Quit,
}

pub async fn handle_dev_command(cmd: DevCommands) -> Result<()> {
    match cmd {
        DevCommands::Server {
            extension,
            verbose: _verbose,
            watch,
            chrome,
        } => start_dev_server(extension, watch, chrome).await,
        DevCommands::Test {
            extension,
            url,
            query,
            verbose: _verbose,
        } => start_interactive_test(extension, url, query).await,
        DevCommands::Validate {
            extension,
            extended,
        } => validate_extension(extension, extended).await,
    }
}

async fn start_dev_server(extension_name: String, watch: bool, use_chrome: bool) -> Result<()> {
    println!("Starting dev server: {}", extension_name);

    let extension_path = find_extension_path(&extension_name)?;
    let mut dev_server = DevServer::new(extension_name.clone(), extension_path, use_chrome).await?;

    println!("Building...");
    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    if watch {
        println!("Watching for changes...");
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = res
                    && matches!(event.kind, EventKind::Modify(_))
                {
                    let _ = tx.send(event);
                }
            },
            notify::Config::default(),
        )?;

        watcher.watch(&dev_server.extension_path, RecursiveMode::Recursive)?;

        let dev_server = Arc::new(Mutex::new(dev_server));

        // File watcher thread
        let server_for_watcher = dev_server.clone();
        std::thread::spawn(move || {
            // Create a new tokio runtime for this thread
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            let mut last_build = Instant::now();

            while let Ok(_event) = rx.recv() {
                // Debounce: only rebuild if it's been more than 500ms since last build
                if last_build.elapsed() < Duration::from_millis(500) {
                    continue;
                }

                let rebuild_start = Instant::now();
                // Clear current line and show rebuild message
                print!("\r\x1b[K"); // Clear current line
                print!("Rebuilding... ");
                std::io::stdout().flush().unwrap();

                let server_result = server_for_watcher.try_lock();
                if let Ok(mut server) = server_result {
                    match rt.block_on(server.build_extension_silent()) {
                        Ok(_) => {
                            if let Err(e) = rt.block_on(server.load_extension()) {
                                println!("failed to reload: {}", e);
                            } else {
                                let rebuild_time = rebuild_start.elapsed();
                                println!("done ({:.2}s)", rebuild_time.as_secs_f64());
                            }
                        }
                        Err(e) => println!("failed: {}", e),
                    }
                    last_build = Instant::now();
                }
                // Print prompt again for clarity
                print!("dev> ");
                std::io::stdout().flush().unwrap();
            }
        });

        println!("Commands: test <url>, search <query>, chapter <url>, meta, rebuild, quit");

        // Create readline editor with history
        let mut rl =
            DefaultEditor::new().map_err(|e| eyre!("Failed to create readline editor: {}", e))?;

        loop {
            match rl.readline("dev> ") {
                Ok(line) => {
                    let command_line = line.trim();
                    if command_line.is_empty() {
                        continue;
                    }

                    // Add to history
                    let _ = rl.add_history_entry(command_line);

                    // Parse command using clap
                    let args: Vec<&str> = command_line.split_whitespace().collect();
                    if args.is_empty() {
                        continue;
                    }

                    // Create a proper command line for clap parsing
                    let full_args = vec!["dev-server"]
                        .into_iter()
                        .chain(args.iter().copied())
                        .collect::<Vec<_>>();

                    match DevServerCommand::try_parse_from(full_args) {
                        Ok(cmd) => {
                            let mut server = dev_server.lock().await;
                            if let Err(e) = server.handle_parsed_command(cmd).await {
                                println!("Command failed: {}", e);
                            }
                        }
                        Err(e) => {
                            // Check if this is a help request
                            let is_help = command_line == "help"
                                || command_line == "--help"
                                || args.contains(&"--help")
                                || args.contains(&"-h");

                            if is_help {
                                // Use clap's built-in help
                                if let Err(help_err) =
                                    DevServerCommand::try_parse_from(vec!["dev-server", "--help"])
                                {
                                    println!("{}", help_err);
                                }
                            } else {
                                // Check if it's a subcommand help request
                                if args.len() >= 2 && (args[1] == "--help" || args[1] == "-h") {
                                    println!("{}", e);
                                } else {
                                    println!(
                                        "Invalid command. Type 'help' for available commands."
                                    );
                                    if !e.to_string().contains("clap") {
                                        println!("Error: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    if command_line == "quit" {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("CTRL-C");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("CTRL-D");
                    break;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }
    }

    println!("Server stopped");
    Ok(())
}

async fn start_interactive_test(
    extension_name: String,
    url: Option<Url>,
    query: Option<String>,
) -> Result<()> {
    println!("Testing extension: {}", extension_name);

    let extension_path = find_extension_path(&extension_name)?;
    let mut dev_server = DevServer::new(extension_name, extension_path, false).await?;

    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    if let Some(ref test_url) = url {
        println!("Testing novel: {}", test_url);
        dev_server.test_novel_info(test_url.to_string()).await?;
    }

    if let Some(ref search_query) = query {
        println!("Testing search: {}", search_query);
        dev_server.test_search(search_query.clone()).await?;
    }

    if url.is_none() && query.is_none() {
        println!("No test URL or query provided. Use --url or --query flags.");
    }

    Ok(())
}

async fn validate_extension(extension_name: String, extended: bool) -> Result<()> {
    println!("Validating: {}", extension_name);

    let extension_path = find_extension_path(&extension_name)?;

    // Check if extension directory exists and has proper structure
    if !extension_path.exists() {
        return Err(eyre!("Extension directory not found: {:?}", extension_path));
    }

    let cargo_toml = extension_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(eyre!("Cargo.toml not found in extension directory"));
    }

    let src_dir = extension_path.join("src");
    if !src_dir.exists() {
        return Err(eyre!("src directory not found in extension"));
    }

    let lib_rs = src_dir.join("lib.rs");
    if !lib_rs.exists() {
        return Err(eyre!("lib.rs not found in src directory"));
    }

    println!("Structure OK");

    // Try to build the extension
    println!("Building...");
    let mut dev_server = DevServer::new(extension_name, extension_path, false).await?;
    dev_server.build_extension().await?;

    println!("Build OK");

    // Try to load and validate metadata
    println!("Loading metadata...");
    dev_server.load_extension().await?;

    let runner = dev_server.create_runner().await?;
    match runner.meta().await {
        Ok((_, meta)) => {
            println!("Metadata OK");
            debug_extension_meta(&meta);

            if extended {
                println!("Running extended validation...");

                if meta.capabilities.search.is_some() {
                    println!("  Search capability declared");
                }

                // TODO: Add more extended validation tests
                println!("Extended validation completed");
            }
        }
        Err(e) => {
            println!("Failed to get metadata: {}", e);
        }
    }

    println!("Validation completed");
    Ok(())
}

fn find_extension_path(extension_name: &str) -> Result<PathBuf> {
    let extension_path = PathBuf::from("extensions").join(extension_name);
    if !extension_path.exists() {
        return Err(eyre!(
            "Extension '{}' not found in extensions/ directory",
            extension_name
        ));
    }
    Ok(extension_path)
}

fn create_extension_engine_with_cache(
    cache_dir: Option<std::path::PathBuf>,
    use_chrome: bool,
) -> Result<ExtensionEngine> {
    // Create base executor
    let base_executor: std::sync::Arc<dyn quelle_engine::http::HttpExecutor> = if use_chrome {
        std::sync::Arc::new(quelle_engine::http::HeadlessChromeExecutor::new())
    } else {
        std::sync::Arc::new(quelle_engine::http::ReqwestExecutor::new())
    };

    // Wrap with caching executor
    let http_executor = if let Some(cache_dir) = cache_dir {
        std::sync::Arc::new(
            CachingHttpExecutor::with_file_cache(base_executor, cache_dir)
                .with_ttl(600) // 10 minutes TTL for dev
                .with_max_cache_size(500),
        ) as std::sync::Arc<dyn quelle_engine::http::HttpExecutor>
    } else {
        std::sync::Arc::new(
            CachingHttpExecutor::new(base_executor)
                .with_ttl(300) // 5 minutes TTL for in-memory cache
                .with_max_cache_size(200),
        ) as std::sync::Arc<dyn quelle_engine::http::HttpExecutor>
    };

    ExtensionEngine::new(http_executor)
        .map_err(|e| eyre!("Failed to create extension engine: {}", e))
}

struct DevServer {
    extension_name: String,
    extension_path: PathBuf,
    engine: ExtensionEngine,
    build_cache: HashMap<String, Instant>,
}

impl DevServer {
    async fn new(
        extension_name: String,
        extension_path: PathBuf,
        use_chrome: bool,
    ) -> Result<Self> {
        // Create cache directory for this extension
        let cache_dir = std::env::temp_dir()
            .join("quelle_dev_cache")
            .join(&extension_name);
        let engine = create_extension_engine_with_cache(Some(cache_dir), use_chrome)?;

        Ok(Self {
            extension_name,
            extension_path,
            engine,
            build_cache: HashMap::new(),
        })
    }

    async fn build_extension(&mut self) -> Result<()> {
        self.build_extension_with_options(false).await
    }

    async fn build_extension_silent(&mut self) -> Result<()> {
        self.build_extension_with_options(true).await
    }

    async fn build_extension_with_options(&mut self, silent: bool) -> Result<()> {
        let start_time = Instant::now();

        if !silent {
            println!("Building '{}'...", self.extension_name);
        }

        let output = tokio::process::Command::new("cargo")
            .args([
                "component",
                "build",
                "-r",
                "-p",
                &format!("extension_{}", self.extension_name),
                "--target",
                "wasm32-unknown-unknown",
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Build failed:\n{}", stderr));
        }

        let build_time = start_time.elapsed();
        if !silent {
            println!("Build completed ({:.2}s)", build_time.as_secs_f64());
        }

        self.build_cache
            .insert(self.extension_name.clone(), Instant::now());
        Ok(())
    }

    async fn load_extension(&mut self) -> Result<()> {
        let wasm_path = PathBuf::from("target")
            .join("wasm32-unknown-unknown")
            .join("release")
            .join(format!("extension_{}.wasm", self.extension_name));

        if !wasm_path.exists() {
            return Err(eyre!("WASM file not found: {:?}", wasm_path));
        }

        debug!("Loading extension from: {:?}", wasm_path);

        // Read WASM bytes
        let wasm_bytes = tokio::fs::read(&wasm_path).await?;

        // Create runner from bytes
        let runner = self.engine.new_runner_from_bytes(&wasm_bytes).await?;

        // Test that the extension loads properly - get metadata
        let (_, _meta) = runner.meta().await?;
        debug!("Extension loaded successfully");
        Ok(())
    }

    async fn handle_parsed_command(&mut self, command: DevServerCommand) -> Result<()> {
        match command {
            DevServerCommand::Test { url } => {
                self.test_novel_info(url).await?;
            }
            DevServerCommand::Search { query } => {
                let query_string = query.join(" ");
                self.test_search(query_string).await?;
            }
            DevServerCommand::Chapter { url } => {
                self.test_chapter_content(url).await?;
            }
            DevServerCommand::Meta => {
                self.show_metadata().await?;
            }
            DevServerCommand::Rebuild => {
                println!("Rebuilding...");
                self.build_extension().await?;
                self.load_extension().await?;
            }
            DevServerCommand::ClearCache => {
                self.clear_cache().await?;
            }
            DevServerCommand::CacheStats => {
                self.show_cache_stats().await?;
            }
            DevServerCommand::Quit => {
                println!("Goodbye!");
            }
        }

        Ok(())
    }

    async fn test_novel_info(&mut self, url: String) -> Result<()> {
        println!("Testing novel: {}", url);
        let start_time = Instant::now();

        let runner = self.create_runner().await?;

        // Fetch novel info
        let (runner, result) = runner.fetch_novel_info(&url).await.map_err(|e| {
            println!("Failed to fetch novel: {}", e);
            e
        })?;

        let novel = result.map_err(|e| {
            println!("Extension error: {:?}", e);
            eyre!("Extension error: {:?}", e)
        })?;

        let fetch_time = start_time.elapsed();
        println!("Novel fetched ({:.2}s)", fetch_time.as_secs_f64());
        debug_novel_info(&novel);

        // Try to fetch first chapter
        if let Some(first_volume) = novel.volumes.first() {
            if let Some(first_chapter) = first_volume.chapters.first() {
                println!("Testing first chapter: {}", first_chapter.title);
                let chapter_start_time = Instant::now();

                let (_, chapter_result) =
                    runner
                        .fetch_chapter(&first_chapter.url)
                        .await
                        .map_err(|e| {
                            println!("Failed to fetch chapter: {}", e);
                            e
                        })?;

                let content = chapter_result.map_err(|e| {
                    println!("Chapter extension error: {:?}", e);
                    eyre!("Chapter extension error: {:?}", e)
                })?;

                let chapter_fetch_time = chapter_start_time.elapsed();
                println!("Chapter fetched ({:.2}s)", chapter_fetch_time.as_secs_f64());
                debug_chapter_content(&first_chapter.url, &content);
            } else {
                println!("No chapters in first volume");
            }
        } else {
            println!("No volumes found");
        }

        Ok(())
    }

    async fn create_runner(&self) -> Result<ExtensionRunner<'_>> {
        let wasm_path = PathBuf::from("target")
            .join("wasm32-unknown-unknown")
            .join("release")
            .join(format!("extension_{}.wasm", self.extension_name));

        let wasm_bytes = tokio::fs::read(&wasm_path).await?;
        let runner = self.engine.new_runner_from_bytes(&wasm_bytes).await?;
        Ok(runner)
    }

    async fn test_search(&mut self, query: String) -> Result<()> {
        println!("Testing search: '{}'", query);
        let start_time = Instant::now();

        let search_query = SimpleSearchQuery {
            query: query.clone(),
            page: Some(1),
            limit: None,
        };

        let runner = self.create_runner().await?;

        match runner.simple_search(&search_query).await {
            Ok((_, result)) => match result {
                Ok(results) => {
                    let search_time = start_time.elapsed();
                    println!("Search completed ({:.2}s)", search_time.as_secs_f64());
                    debug_search_results(&query, &results);
                }
                Err(e) => {
                    println!("Extension error: {:?}", e);
                }
            },
            Err(e) => {
                println!("Search failed: {}", e);
            }
        }

        Ok(())
    }

    async fn test_chapter_content(&mut self, url: String) -> Result<()> {
        println!("Testing chapter: {}", url);
        let start_time = Instant::now();

        let runner = self.create_runner().await?;

        match runner.fetch_chapter(&url).await {
            Ok((_, result)) => match result {
                Ok(content) => {
                    let fetch_time = start_time.elapsed();
                    println!("Chapter fetched ({:.2}s)", fetch_time.as_secs_f64());
                    debug_chapter_content(&url, &content);
                }
                Err(e) => {
                    println!("Extension error: {:?}", e);
                }
            },
            Err(e) => {
                println!("Failed to fetch chapter: {}", e);
            }
        }

        Ok(())
    }

    async fn show_metadata(&mut self) -> Result<()> {
        let runner = self.create_runner().await?;

        match runner.meta().await {
            Ok((_, meta)) => {
                debug_extension_meta(&meta);

                // Show recent build info if available
                if let Some(build_time) = self.build_cache.get(&self.extension_name) {
                    let age = build_time.elapsed();
                    println!("  Last built: {:.1} minutes ago", age.as_secs_f64() / 60.0);
                }
            }
            Err(e) => {
                println!("Failed to get metadata: {}", e);
            }
        }

        Ok(())
    }

    /// Clear the HTTP cache
    async fn clear_cache(&mut self) -> Result<()> {
        // We need to access the caching executor, but it's wrapped in the engine
        // For now, we'll just print a message. In a real implementation, we'd need
        // to expose the cache clearing functionality through the engine
        println!("Cache clearing not implemented - restart server to clear cache");
        Ok(())
    }

    /// Show cache statistics
    async fn show_cache_stats(&mut self) -> Result<()> {
        println!("Cache statistics not implemented");
        Ok(())
    }
}

/// Debug output functions for development server
fn debug_novel_info(novel: &Novel) {
    println!("Novel Info:");
    println!("  Title: {}", novel.title);
    println!("  Authors: {:?}", novel.authors);
    println!("  Status: {:?}", novel.status);
    println!("  Languages: {:?}", novel.langs);
    println!("  Volumes: {}", novel.volumes.len());

    for (i, volume) in novel.volumes.iter().enumerate() {
        println!("  Volume {}: {} chapters", i, volume.chapters.len());
    }

    if let Some(description) = novel.description.first() {
        let desc_preview = if description.len() > 100 {
            format!("{}...", &description[..100])
        } else {
            description.clone()
        };
        println!("  Description: {}", desc_preview);
    }

    println!("  Metadata entries: {}", novel.metadata.len());
}

fn debug_search_results(query: &str, results: &SearchResult) {
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
        println!("  ... and {} more results", results.novels.len() - 5);
    }
}

fn debug_chapter_content(url: &str, content: &ChapterContent) {
    println!("Chapter Content from {}:", url);
    println!("  Length: {} characters", content.data.len());

    let preview = if content.data.len() > 200 {
        format!("{}...", &content.data[..197])
    } else {
        content.data.clone()
    };
    println!("  Preview: {}", preview.replace('\n', " "));
}

fn debug_extension_meta(meta: &SourceMeta) {
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
    }
}
