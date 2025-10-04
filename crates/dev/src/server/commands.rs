//! Command handling for the development server

use clap::Parser;
use eyre::{Result, eyre};
use quelle_engine::bindings::quelle::extension::novel::SimpleSearchQuery;
use url::Url;

use super::DevServer;

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
    }
}

/// Test novel info fetching from a URL
async fn test_novel_info(_server: &DevServer, url: &str) -> Result<()> {
    println!("🔍 Testing novel info for: {}", url);

    // Validate URL format
    let parsed_url = Url::parse(url).map_err(|e| eyre!("Invalid URL '{}': {}", url, e))?;

    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(eyre!("URL must use HTTP or HTTPS protocol"));
    }

    // Create a mock runner for testing
    // Note: This would need to be adapted based on the actual ExtensionEngine API
    println!("⏳ Fetching novel information...");

    // TODO: Implement actual novel info fetching through the extension
    // This would involve calling the extension's fetch_novel_info method
    println!("⚠️  Novel info testing not yet implemented in refactored code");
    println!("    This requires integration with the ExtensionRunner API");

    Ok(())
}

/// Test search functionality
async fn test_search(_server: &DevServer, query_parts: &[String]) -> Result<()> {
    if query_parts.is_empty() {
        return Err(eyre!("Search query cannot be empty"));
    }

    let query_string = query_parts.join(" ");
    println!("🔍 Testing search for: '{}'", query_string);

    // Create search query
    let _search_query = SimpleSearchQuery {
        query: query_string.clone(),
        page: Some(1),
        limit: None,
    };

    println!("⏳ Executing search...");

    // TODO: Implement actual search through the extension
    // This would involve calling the extension's simple_search method
    println!("⚠️  Search testing not yet implemented in refactored code");
    println!("    This requires integration with the ExtensionRunner API");

    Ok(())
}

/// Test chapter content fetching
async fn test_chapter_content(_server: &DevServer, url: &str) -> Result<()> {
    println!("📖 Testing chapter content for: {}", url);

    // Validate URL format
    let parsed_url = Url::parse(url).map_err(|e| eyre!("Invalid URL '{}': {}", url, e))?;

    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(eyre!("URL must use HTTP or HTTPS protocol"));
    }

    println!("⏳ Fetching chapter content...");

    // TODO: Implement actual chapter content fetching through the extension
    // This would involve calling the extension's fetch_chapter method
    println!("⚠️  Chapter content testing not yet implemented in refactored code");
    println!("    This requires integration with the ExtensionRunner API");

    Ok(())
}

/// Show extension metadata
async fn show_extension_meta(_server: &DevServer) -> Result<()> {
    println!("📋 Extension Metadata:");

    // TODO: Get actual metadata from the loaded extension
    // This would involve calling the extension's meta method
    println!("⚠️  Metadata display not yet implemented in refactored code");
    println!("    This requires integration with the ExtensionRunner API");

    Ok(())
}

/// Validate that a URL is properly formatted and uses HTTP/HTTPS
pub fn validate_url(url: &str) -> Result<Url> {
    let parsed = Url::parse(url).map_err(|e| eyre!("Invalid URL format: {}", e))?;

    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        scheme => Err(eyre!(
            "Invalid URL scheme '{}'. Only HTTP and HTTPS are supported",
            scheme
        )),
    }
}

/// Create a simple search query with pagination
pub fn create_search_query(query: &str, page: u32) -> SimpleSearchQuery {
    SimpleSearchQuery {
        query: query.to_string(),
        page: Some(page),
        limit: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://test.org/path").is_ok());

        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("not-a-url").is_err());
        assert!(validate_url("").is_err());
    }

    #[test]
    fn test_create_search_query() {
        let query = create_search_query("test novel", 1);
        assert_eq!(query.query, "test novel");
        assert_eq!(query.page, Some(1));
    }
}
