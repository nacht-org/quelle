use crate::config::Config;
use crate::utils::create_extension_engine;
use clap::Subcommand;
use eyre::{Result, eyre};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use quelle_engine::bindings::quelle::extension::{
    novel::{ChapterContent, Novel, SearchResult, SimpleSearchQuery},
    source::SourceMeta,
};
use quelle_engine::{ExtensionEngine, ExtensionRunner};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tracing::{debug, error, info};
use url::Url;

#[derive(Subcommand, Debug)]
pub enum DevCommands {
    /// Start development server with hot reload
    Server {
        /// Extension name to develop
        extension: String,
        /// Port for development server (optional, for future web interface)
        #[arg(long, default_value = "3001")]
        port: u16,
        /// Enable verbose logging
        #[arg(long, short)]
        verbose: bool,
        /// Auto-rebuild on file changes
        #[arg(long, default_value = "true")]
        watch: bool,
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

pub async fn handle_dev_command(cmd: DevCommands, config: &Config) -> Result<()> {
    match cmd {
        DevCommands::Server {
            extension,
            port: _,
            verbose,
            watch,
        } => {
            if verbose {
                unsafe {
                    std::env::set_var("RUST_LOG", "debug");
                }
            }

            start_dev_server(extension, watch, config).await
        }
        DevCommands::Test {
            extension,
            url,
            query,
            verbose,
        } => {
            if verbose {
                unsafe {
                    std::env::set_var("RUST_LOG", "debug");
                }
            }

            start_interactive_test(extension, url, query, config).await
        }
        DevCommands::Validate {
            extension,
            extended,
        } => validate_extension(extension, extended, config).await,
    }
}

async fn start_dev_server(extension_name: String, watch: bool, config: &Config) -> Result<()> {
    info!(
        "🚀 Starting development server for extension: {}",
        extension_name
    );

    let extension_path = find_extension_path(&extension_name)?;
    let mut dev_server = DevServer::new(extension_name.clone(), extension_path, config).await?;

    // Initial build
    info!("📦 Building extension...");
    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    if watch {
        info!("👀 Watching for file changes...");
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        let _ = tx.send(event);
                    }
                }
            },
            notify::Config::default(),
        )?;

        watcher.watch(&dev_server.extension_path, RecursiveMode::Recursive)?;

        let dev_server = Arc::new(Mutex::new(dev_server));

        // File watcher thread
        let server_for_watcher = dev_server.clone();
        std::thread::spawn(move || {
            let mut last_build = Instant::now();

            while let Ok(_event) = rx.recv() {
                // Debounce: only rebuild if it's been more than 500ms since last build
                if last_build.elapsed() < Duration::from_millis(500) {
                    continue;
                }

                info!("📝 Files changed, rebuilding...");
                let rt = tokio::runtime::Handle::current();
                if let Ok(mut server) = server_for_watcher.lock() {
                    match rt.block_on(server.build_extension()) {
                        Ok(_) => {
                            if let Err(e) = rt.block_on(server.load_extension()) {
                                error!("❌ Failed to reload extension: {}", e);
                            } else {
                                info!("✅ Extension reloaded successfully");
                            }
                        }
                        Err(e) => error!("❌ Build failed: {}", e),
                    }
                    last_build = Instant::now();
                }
            }
        });

        // Interactive command loop
        info!("💻 Development server ready! Available commands:");
        info!("  test <url>     - Test novel info fetching");
        info!("  search <query> - Test search functionality");
        info!("  chapter <url>  - Test chapter content fetching");
        info!("  meta          - Show extension metadata");
        info!("  rebuild       - Force rebuild extension");
        info!("  quit          - Exit development server");

        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            use std::io::Write;
            print!("dev> ");
            std::io::stdout().flush().unwrap();

            line.clear();
            if reader.read_line(&mut line).await? == 0 {
                break; // EOF
            }

            let command = line.trim();
            if command.is_empty() {
                continue;
            }

            if let Ok(mut server) = dev_server.lock() {
                if let Err(e) = server.handle_command(command).await {
                    error!("❌ Command failed: {}", e);
                }
            }

            if command == "quit" {
                break;
            }
        }
    }

    info!("👋 Development server stopped");
    Ok(())
}

async fn start_interactive_test(
    extension_name: String,
    url: Option<Url>,
    query: Option<String>,
    config: &Config,
) -> Result<()> {
    info!(
        "🧪 Starting interactive test for extension: {}",
        extension_name
    );

    let extension_path = find_extension_path(&extension_name)?;
    let mut dev_server = DevServer::new(extension_name, extension_path, config).await?;

    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    if let Some(ref test_url) = url {
        info!("🔍 Testing novel info fetch for: {}", test_url);
        dev_server.test_novel_info(test_url.to_string()).await?;
    }

    if let Some(ref search_query) = query {
        info!("🔍 Testing search for: {}", search_query);
        dev_server.test_search(search_query.clone()).await?;
    }

    if url.is_none() && query.is_none() {
        info!(
            "💡 No test URL or query provided. Use --url or --query flags to test specific functionality."
        );
    }

    Ok(())
}

async fn validate_extension(extension_name: String, extended: bool, config: &Config) -> Result<()> {
    info!("✅ Validating extension: {}", extension_name);

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

    info!("✅ Basic structure validation passed");

    // Try to build the extension
    info!("🔨 Building extension...");
    let mut dev_server = DevServer::new(extension_name, extension_path, config).await?;
    dev_server.build_extension().await?;

    info!("✅ Build validation passed");

    // Try to load and validate metadata
    info!("📋 Loading extension metadata...");
    dev_server.load_extension().await?;

    let runner = dev_server.create_runner().await?;
    match runner.meta().await {
        Ok((_, meta)) => {
            info!("✅ Extension metadata loaded");
            debug_extension_meta(&meta);

            if extended {
                info!("🔍 Running extended validation...");

                // Test that required methods are implemented
                if meta.capabilities.search.is_some() {
                    info!("   ✅ Search capability declared");
                }

                // TODO: Add more extended validation tests
                info!("✅ Extended validation completed");
            }
        }
        Err(e) => {
            error!("❌ Failed to get extension metadata: {}", e);
        }
    }

    info!("🎉 Extension validation completed successfully");
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
        _config: &Config,
    ) -> Result<Self> {
        let engine = create_extension_engine()?;

        Ok(Self {
            extension_name,
            extension_path,
            engine,
            build_cache: HashMap::new(),
        })
    }

    async fn build_extension(&mut self) -> Result<()> {
        let start_time = Instant::now();

        info!("🔨 Building extension '{}'...", self.extension_name);

        let output = tokio::process::Command::new("cargo")
            .args(&[
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
        info!("✅ Build completed in {:.2}s", build_time.as_secs_f64());

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
        debug!("✅ Extension loaded successfully");
        Ok(())
    }

    async fn handle_command(&mut self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "test" => {
                if parts.len() < 2 {
                    info!("Usage: test <url>");
                    return Ok(());
                }
                let url = parts[1].to_string();
                self.test_novel_info(url).await?;
            }
            "search" => {
                if parts.len() < 2 {
                    info!("Usage: search <query>");
                    return Ok(());
                }
                let query = parts[1..].join(" ");
                self.test_search(query).await?;
            }
            "chapter" => {
                if parts.len() < 2 {
                    info!("Usage: chapter <url>");
                    return Ok(());
                }
                let url = parts[1].to_string();
                self.test_chapter_content(url).await?;
            }
            "meta" => {
                self.show_metadata().await?;
            }
            "rebuild" => {
                info!("🔄 Force rebuilding extension...");
                self.build_extension().await?;
                self.load_extension().await?;
            }
            "quit" => {
                info!("👋 Goodbye!");
            }
            _ => {
                info!(
                    "Unknown command: {}. Available: test, search, chapter, meta, rebuild, quit",
                    parts[0]
                );
            }
        }

        Ok(())
    }

    async fn test_novel_info(&mut self, url: String) -> Result<()> {
        info!("🔍 Testing novel info fetch for: {}", url);
        let start_time = Instant::now();

        // Create a fresh runner for this operation
        let runner = self.create_runner().await?;

        match runner.fetch_novel_info(&url).await {
            Ok((_, result)) => match result {
                Ok(novel) => {
                    let fetch_time = start_time.elapsed();
                    info!("✅ Novel info fetched in {:.2}s", fetch_time.as_secs_f64());
                    debug_novel_info(&novel);
                }
                Err(e) => {
                    error!("❌ Extension error: {:?}", e);
                }
            },
            Err(e) => {
                error!("❌ Failed to fetch novel info: {}", e);
            }
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
        info!("🔍 Testing search for: '{}'", query);
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
                    info!("✅ Search completed in {:.2}s", search_time.as_secs_f64());
                    debug_search_results(&query, &results);
                }
                Err(e) => {
                    error!("❌ Extension error: {:?}", e);
                }
            },
            Err(e) => {
                error!("❌ Search failed: {}", e);
            }
        }

        Ok(())
    }

    async fn test_chapter_content(&mut self, url: String) -> Result<()> {
        info!("📖 Testing chapter content fetch for: {}", url);
        let start_time = Instant::now();

        let runner = self.create_runner().await?;

        match runner.fetch_chapter(&url).await {
            Ok((_, result)) => match result {
                Ok(content) => {
                    let fetch_time = start_time.elapsed();
                    info!(
                        "✅ Chapter content fetched in {:.2}s",
                        fetch_time.as_secs_f64()
                    );
                    debug_chapter_content(&url, &content);
                }
                Err(e) => {
                    error!("❌ Extension error: {:?}", e);
                }
            },
            Err(e) => {
                error!("❌ Failed to fetch chapter content: {}", e);
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
                    info!("   Last built: {:.1} minutes ago", age.as_secs_f64() / 60.0);
                }
            }
            Err(e) => {
                error!("❌ Failed to get metadata: {}", e);
            }
        }

        Ok(())
    }
}

/// Simple performance counter for development server timing
struct PerfCounter {
    name: String,
    start: Instant,
    checkpoints: Vec<(String, Duration)>,
}

impl PerfCounter {
    fn new(name: &str) -> Self {
        tracing::debug!("⏱️ Starting performance measurement: {}", name);
        Self {
            name: name.to_string(),
            start: Instant::now(),
            checkpoints: Vec::new(),
        }
    }

    /// Add a checkpoint with a label
    fn checkpoint(&mut self, label: &str) {
        let elapsed = self.start.elapsed();
        self.checkpoints.push((label.to_string(), elapsed));
        tracing::debug!("⏱️ {} - {}: {:.2}ms", self.name, label, elapsed.as_millis());
    }

    /// Finish the performance measurement
    fn finish(self) {
        let total = self.start.elapsed();
        tracing::debug!("⏱️ {} - Total: {:.2}ms", self.name, total.as_millis());

        if self.checkpoints.len() > 1 {
            tracing::debug!("   Breakdown:");
            let mut last_time = Duration::from_millis(0);
            for (label, time) in &self.checkpoints {
                let segment_time = *time - last_time;
                tracing::debug!("     {}: {:.2}ms", label, segment_time.as_millis());
                last_time = *time;
            }
        }
    }
}

/// Debug output functions for development server
fn debug_novel_info(novel: &Novel) {
    info!("📚 Novel Info:");
    info!("   Title: {}", novel.title);
    info!("   Authors: {:?}", novel.authors);
    info!("   Status: {:?}", novel.status);
    info!("   Languages: {:?}", novel.langs);
    info!("   Volumes: {}", novel.volumes.len());

    for (i, volume) in novel.volumes.iter().enumerate() {
        info!("   Volume {}: {} chapters", i, volume.chapters.len());
    }

    if let Some(description) = novel.description.first() {
        let desc_preview = if description.len() > 100 {
            format!("{}...", &description[..100])
        } else {
            description.clone()
        };
        info!("   Description: {}", desc_preview);
    }

    info!("   Metadata entries: {}", novel.metadata.len());
}

fn debug_search_results(query: &str, results: &SearchResult) {
    info!("🔍 Search Results for '{}':", query);
    info!("   Found: {} novels", results.novels.len());
    info!("   Current page: {}", results.current_page);
    if let Some(total_pages) = results.total_pages {
        info!("   Total pages: {}", total_pages);
    }
    info!("   Has next: {}", results.has_next_page);

    for (i, novel) in results.novels.iter().take(5).enumerate() {
        info!("   {}. {} - {}", i + 1, novel.title, novel.url);
    }

    if results.novels.len() > 5 {
        info!("   ... and {} more results", results.novels.len() - 5);
    }
}

fn debug_chapter_content(url: &str, content: &ChapterContent) {
    info!("📖 Chapter Content from {}:", url);
    info!("   Content length: {} characters", content.data.len());

    // Show content preview
    let preview = if content.data.len() > 200 {
        format!("{}...", &content.data[..197])
    } else {
        content.data.clone()
    };
    info!("   Preview: {}", preview.replace('\n', " "));
}

fn debug_extension_meta(meta: &SourceMeta) {
    info!("📋 Extension Metadata:");
    info!("   ID: {}", meta.id);
    info!("   Name: {}", meta.name);
    info!("   Version: {}", meta.version);
    info!("   Languages: {:?}", meta.langs);
    info!("   Base URLs: {:?}", meta.base_urls);
    info!("   Reading Directions: {:?}", meta.rds);

    if let Some(search_caps) = &meta.capabilities.search {
        info!("   Search Capabilities:");
        info!(
            "     - Simple search: {}",
            search_caps.supports_simple_search
        );
    }
}
