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
    /// Generate a new extension from template
    Generate {
        /// Extension name (lowercase, no spaces)
        name: Option<String>,
        /// Display name for the extension
        #[arg(long)]
        display_name: Option<String>,
        /// Base URL of the target website
        #[arg(long)]
        base_url: Option<String>,
        /// Primary language code (default: en)
        #[arg(long)]
        language: Option<String>,
        /// Reading direction (ltr or rtl)
        #[arg(long)]
        reading_direction: Option<String>,
        /// Force overwrite if extension already exists
        #[arg(long)]
        force: bool,
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
        DevCommands::Generate {
            name,
            display_name,
            base_url,
            language,
            reading_direction,
            force,
        } => {
            handle_generate_command(
                name,
                display_name,
                base_url,
                language,
                reading_direction,
                force,
            )
            .await
        }
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

fn create_extension_engine_with_cache_ref(
    cache_dir: Option<PathBuf>,
    use_chrome: bool,
) -> Result<(ExtensionEngine, Option<Arc<CachingHttpExecutor>>)> {
    let base_executor: std::sync::Arc<dyn quelle_engine::http::HttpExecutor> = if use_chrome {
        std::sync::Arc::new(quelle_engine::http::HeadlessChromeExecutor::new())
    } else {
        std::sync::Arc::new(quelle_engine::http::ReqwestExecutor::new())
    };

    let caching_executor = if let Some(cache_dir) = cache_dir {
        Arc::new(
            CachingHttpExecutor::with_file_cache(base_executor, cache_dir)
                .with_ttl(600) // 10 minutes TTL for dev
                .with_max_cache_size(500),
        )
    } else {
        Arc::new(
            CachingHttpExecutor::new(base_executor)
                .with_ttl(300) // 5 minutes TTL for in-memory cache
                .with_max_cache_size(200),
        )
    };

    let executor_clone =
        caching_executor.clone() as std::sync::Arc<dyn quelle_engine::http::HttpExecutor>;
    let engine = quelle_engine::ExtensionEngine::new(executor_clone)?;

    Ok((engine, Some(caching_executor)))
}

struct DevServer {
    extension_name: String,
    extension_path: PathBuf,
    engine: ExtensionEngine,
    build_cache: HashMap<String, Instant>,
    caching_executor: Option<Arc<CachingHttpExecutor>>,
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
        if let Some(caching_executor) = &self.caching_executor {
            caching_executor.clear_cache().await;
            println!("✅ Cache cleared successfully");
        } else {
            println!("ℹ️  No cache to clear (using non-caching HTTP executor)");
        }
        Ok(())
    }

    /// Show cache statistics
    async fn show_cache_stats(&mut self) -> Result<()> {
        if let Some(caching_executor) = &self.caching_executor {
            let (memory_count, file_count) = caching_executor.cache_stats().await;
            println!("📊 Cache Statistics:");
            println!("  Memory cache entries: {}", memory_count);
            println!("  File cache entries: {}", file_count);
        } else {
            println!("ℹ️  No cache statistics available (using non-caching HTTP executor)");
        }
        Ok(())
    }
}

async fn handle_generate_command(
    name: Option<String>,
    display_name: Option<String>,
    base_url: Option<String>,
    language: Option<String>,
    reading_direction: Option<String>,
    force: bool,
) -> Result<()> {
    use std::collections::HashMap;
    use std::fs;

    // Collect parameters interactively if needed
    let extension_name = match name {
        Some(n) if !n.is_empty() => validate_extension_name(n)?,
        _ => prompt_for_extension_name()?,
    };

    let display_name = match display_name {
        Some(d) if !d.is_empty() => d,
        _ => prompt_for_display_name(&extension_name)?,
    };

    let base_url = match base_url {
        Some(u) if !u.is_empty() => validate_base_url(u)?,
        _ => prompt_for_base_url()?,
    };

    let language = match language {
        Some(l) if !l.is_empty() => validate_language(l)?,
        _ => prompt_for_language()?,
    };

    let reading_direction = match reading_direction {
        Some(r) if !r.is_empty() => validate_reading_direction(r)?,
        _ => prompt_for_reading_direction()?,
    };

    let force = if !force {
        check_force_needed(&extension_name)?
    } else {
        force
    };

    println!("🏗️  Generating extension '{}'...", extension_name);

    // Find project root
    let current_dir = std::env::current_dir()?;
    let project_root = find_project_root(&current_dir)?;
    let extensions_dir = project_root.join("extensions");
    let output_dir = extensions_dir.join(&extension_name);

    // Check if extension exists
    if output_dir.exists() && !force {
        return Err(eyre!(
            "Extension '{}' already exists! Use --force to overwrite",
            extension_name
        ));
    }

    if output_dir.exists() && force {
        println!("🗑️  Removing existing extension '{}'...", extension_name);
        fs::remove_dir_all(&output_dir)?;
    }

    // Create extension directory
    fs::create_dir_all(&output_dir)?;
    fs::create_dir_all(output_dir.join("src"))?;

    // Template replacements
    let mut replacements = HashMap::new();
    replacements.insert("EXTENSION_NAME", extension_name.as_str());
    replacements.insert("EXTENSION_DISPLAY_NAME", display_name.as_str());
    replacements.insert("BASE_URL", base_url.trim_end_matches('/'));
    replacements.insert("LANGUAGE", language.as_str());
    replacements.insert("READING_DIRECTION", &reading_direction);

    // Create Cargo.toml
    let cargo_toml_content = create_cargo_toml_template(&replacements);
    fs::write(output_dir.join("Cargo.toml"), cargo_toml_content)?;
    println!("   ✓ Cargo.toml");

    // Create lib.rs
    let lib_rs_content = create_lib_rs_template(&replacements);
    fs::write(output_dir.join("src/lib.rs"), lib_rs_content)?;
    println!("   ✓ src/lib.rs");

    println!("✅ Extension '{}' generated successfully!", extension_name);
    println!("   Location: {}", output_dir.display());
    println!("\n📋 Next steps:");
    println!("   1. Edit the selectors in src/lib.rs");
    println!("   2. Build: just build-extension {}", extension_name);
    println!("   3. Test: just dev-server {}", extension_name);
    println!("   4. Publish: just publish {}", extension_name);

    Ok(())
}

fn prompt_for_extension_name() -> Result<String> {
    print!("📝 Extension name (lowercase, no spaces): ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    validate_extension_name(input.trim().to_string())
}

fn prompt_for_display_name(extension_name: &str) -> Result<String> {
    let suggested = extension_name
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    print!("✨ Display name [{}]: ", suggested);
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        Ok(suggested)
    } else {
        Ok(input.to_string())
    }
}

fn prompt_for_base_url() -> Result<String> {
    print!("🌐 Base URL (https://example.com): ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    validate_base_url(input.trim().to_string())
}

fn prompt_for_language() -> Result<String> {
    print!("🌍 Language code [en]: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        Ok("en".to_string())
    } else {
        validate_language(input.to_string())
    }
}

fn prompt_for_reading_direction() -> Result<String> {
    print!("📖 Reading direction (ltr/rtl) [ltr]: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        Ok("Ltr".to_string())
    } else {
        validate_reading_direction(input.to_string())
    }
}

fn check_force_needed(extension_name: &str) -> Result<bool> {
    let current_dir = std::env::current_dir()?;
    let project_root = find_project_root(&current_dir)?;
    let extensions_dir = project_root.join("extensions");
    let output_dir = extensions_dir.join(extension_name);

    if output_dir.exists() {
        print!(
            "⚠️  Extension '{}' already exists. Overwrite? (y/N): ",
            extension_name
        );
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        Ok(input == "y" || input == "yes")
    } else {
        Ok(false)
    }
}

fn validate_extension_name(name: String) -> Result<String> {
    let extension_name = name.to_lowercase().replace("-", "_");
    if !extension_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(eyre!(
            "Extension name must contain only letters, numbers, and underscores"
        ));
    }

    if extension_name.len() < 2 {
        return Err(eyre!("Extension name must be at least 2 characters long"));
    }

    Ok(extension_name)
}

fn validate_base_url(url: String) -> Result<String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(eyre!("Base URL must start with http:// or https://"));
    }
    Ok(url)
}

fn validate_language(lang: String) -> Result<String> {
    if lang.len() != 2 {
        return Err(eyre!(
            "Language code must be exactly 2 characters (ISO 639-1)"
        ));
    }
    Ok(lang.to_lowercase())
}

fn validate_reading_direction(dir: String) -> Result<String> {
    match dir.to_lowercase().as_str() {
        "ltr" => Ok("Ltr".to_string()),
        "rtl" => Ok("Rtl".to_string()),
        _ => Err(eyre!("Reading direction must be 'ltr' or 'rtl'")),
    }
}

fn find_project_root(start_dir: &std::path::Path) -> Result<std::path::PathBuf> {
    let mut current = start_dir;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") && content.contains("extensions") {
                    return Ok(current.to_path_buf());
                }
            }
        }
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            return Err(eyre!("Could not find Quelle project root"));
        }
    }
}

fn create_cargo_toml_template(replacements: &HashMap<&str, &str>) -> String {
    let template = r#"[package]
name = "extension_{{EXTENSION_NAME}}"
version = "0.1.0"
edition = "2021"

[dependencies]
quelle_extension = { path = "../../crates/extension" }
chrono = { workspace = true }
once_cell = { workspace = true }
tracing = { workspace = true }
eyre = { workspace = true }
scraper = { workspace = true }
ego-tree = { workspace = true }
url = { workspace = true }

[lib]
crate-type = ["cdylib"]
"#;

    apply_replacements(template, replacements)
}

fn create_lib_rs_template(replacements: &HashMap<&str, &str>) -> String {
    let template = r#"use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

register_extension!(Extension);

const BASE_URL: &str = "{{BASE_URL}}";

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("{{LANGUAGE}}.{{EXTENSION_NAME}}"),
    name: String::from("{{EXTENSION_DISPLAY_NAME}}"),
    langs: vec![String::from("{{LANGUAGE}}")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![BASE_URL.to_string()],
    rds: vec![ReadingDirection::{{READING_DIRECTION}}],
    attrs: vec![],
    capabilities: SourceCapabilities {
        search: Some(SearchCapabilities {
            supports_simple_search: true,
            supports_complex_search: false,
            ..Default::default()
        }),
    },
});

pub struct Extension {
    client: Client,
}

impl QuelleExtension for Extension {
    fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn meta(&self) -> SourceMeta {
        META.clone()
    }

    fn fetch_novel_info(&self, url: String) -> Result<Novel, eyre::Report> {
        // TODO: Implement novel info scraping for your target website
        // 1. Make HTTP request to the URL
        // 2. Parse HTML response
        // 3. Extract novel information (title, authors, description, etc.)
        // 4. Extract chapters and organize into volumes
        // 5. Extract additional metadata (genres, tags, ratings, etc.)
        todo!("Implement novel info scraping for your target website")
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        // TODO: Implement chapter content scraping for your target website
        // 1. Make HTTP request to the chapter URL
        // 2. Parse HTML response
        // 3. Extract chapter content using appropriate selectors
        // 4. Return ChapterContent with the extracted data
        todo!("Implement chapter content scraping for your target website")
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        // TODO: Implement search functionality for your target website
        // 1. Build search URL with query parameters
        // 2. Make HTTP request to search endpoint
        // 3. Parse HTML response
        // 4. Extract search results (novels list)
        // 5. Handle pagination if supported
        // 6. Return SearchResult with novels and pagination info
        todo!("Implement search functionality for your target website")
    }
}
"#;

    apply_replacements(template, replacements)
}

fn apply_replacements(template: &str, replacements: &HashMap<&str, &str>) -> String {
    let mut result = template.to_string();
    for (key, value) in replacements {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
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
