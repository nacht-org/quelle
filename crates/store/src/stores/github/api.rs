//! GitHub API utilities and helper functions
//!
//! Centralizes GitHub API operations with consistent error handling and headers.

use reqwest::Client;
use serde_json::Value;
use tracing::{debug, warn};

use crate::error::{Result, StoreError};
use crate::stores::providers::git::GitReference;

pub const USER_AGENT: &str = "quelle-store/0.1.0";
pub const ACCEPT_HEADER: &str = "application/vnd.github.v3+json";
#[derive(Debug)]
pub struct RepositoryInfo {
    pub name: String,
    pub full_name: String,
    pub default_branch: String,
    pub private: bool,
}

#[derive(Debug)]
pub struct ContentItem {
    pub name: String,
    pub path: String,
    pub item_type: ContentType,
    pub size: Option<u64>,
}

#[derive(Debug, PartialEq)]
pub enum ContentType {
    File,
    Directory,
    Symlink,
    Submodule,
}

impl ContentType {
    fn from_str(s: &str) -> Self {
        match s {
            "file" => ContentType::File,
            "dir" => ContentType::Directory,
            "symlink" => ContentType::Symlink,
            "submodule" => ContentType::Submodule,
            _ => ContentType::File, // Default to file for unknown types
        }
    }
}

/// Create a default HTTP client configured for GitHub API usage
pub fn create_default_client() -> Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .build()
        .expect("Failed to create HTTP client")
}

/// Create an authenticated HTTP client for GitHub API usage
pub fn create_authenticated_client(token: &str) -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    let auth_value = format!("token {}", token);
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&auth_value).expect("Invalid authentication token"),
    );

    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()
        .expect("Failed to create authenticated HTTP client")
}

/// Get repository information from GitHub API
pub async fn get_repository_info(
    client: &Client,
    owner: &str,
    repo: &str,
) -> Result<RepositoryInfo> {
    let api_url = format!("https://api.github.com/repos/{}/{}", owner, repo);

    debug!("Fetching repository info from GitHub API: {}", api_url);

    let response = client
        .get(&api_url)
        .header("Accept", ACCEPT_HEADER)
        .send()
        .await
        .map_err(|e| {
            StoreError::NetworkError(format!(
                "Failed to fetch repository info for {}/{}: {}",
                owner, repo, e
            ))
        })?;

    if response.status() == 404 {
        return Err(StoreError::ExtensionNotFound(format!(
            "Repository {}/{} not found or not accessible",
            owner, repo
        )));
    }

    if !response.status().is_success() {
        return Err(StoreError::NetworkError(format!(
            "GitHub API repository info failed with status {}: {}",
            response.status(),
            response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown error")
        )));
    }

    let repo_data: Value = response.json().await.map_err(|e| {
        StoreError::NetworkError(format!(
            "Failed to parse GitHub API repository response for {}/{}: {}",
            owner, repo, e
        ))
    })?;

    let name = repo_data
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            StoreError::ParseError("Repository name missing from GitHub API response".to_string())
        })?
        .to_string();

    let full_name = repo_data
        .get("full_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            StoreError::ParseError(
                "Repository full_name missing from GitHub API response".to_string(),
            )
        })?
        .to_string();

    let default_branch = repo_data
        .get("default_branch")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            StoreError::ParseError("Default branch missing from GitHub API response".to_string())
        })?
        .to_string();

    let private = repo_data
        .get("private")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(RepositoryInfo {
        name,
        full_name,
        default_branch,
        private,
    })
}

/// List contents of a directory in a GitHub repository
pub async fn list_repository_contents(
    client: &Client,
    owner: &str,
    repo: &str,
    path: &str,
    reference: &str,
) -> Result<Vec<ContentItem>> {
    let api_url = format!(
        "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
        owner, repo, path, reference
    );

    debug!("Listing GitHub directory contents via API: {}", api_url);

    let response = client
        .get(&api_url)
        .header("Accept", ACCEPT_HEADER)
        .send()
        .await
        .map_err(|e| {
            StoreError::NetworkError(format!(
                "Failed to list directory {} in GitHub {}/{}: {}",
                path, owner, repo, e
            ))
        })?;

    if response.status() == 404 {
        return Ok(Vec::new()); // Directory doesn't exist
    }

    if !response.status().is_success() {
        return Err(StoreError::NetworkError(format!(
            "GitHub API directory listing failed with status {}: {} (repo: {}/{}, path: {})",
            response.status(),
            response
                .status()
                .canonical_reason()
                .unwrap_or("Unknown error"),
            owner,
            repo,
            path
        )));
    }

    let content: Value = response.json().await.map_err(|e| {
        StoreError::NetworkError(format!(
            "Failed to parse GitHub API response for directory {} in {}/{}: {}",
            path, owner, repo, e
        ))
    })?;

    let mut items = Vec::new();

    if let Some(array) = content.as_array() {
        for item in array {
            if let (Some(name), Some(path_str), Some(type_str)) = (
                item.get("name").and_then(|n| n.as_str()),
                item.get("path").and_then(|p| p.as_str()),
                item.get("type").and_then(|t| t.as_str()),
            ) {
                let size = item.get("size").and_then(|s| s.as_u64());

                items.push(ContentItem {
                    name: name.to_string(),
                    path: path_str.to_string(),
                    item_type: ContentType::from_str(type_str),
                    size,
                });
            }
        }
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// Check if a reference (branch/tag/commit) exists in a GitHub repository
pub async fn check_reference_exists(
    client: &Client,
    owner: &str,
    repo: &str,
    reference: &str,
) -> Result<bool> {
    let test_url = format!(
        "https://api.github.com/repos/{}/{}/contents?ref={}",
        owner, repo, reference
    );

    debug!("Checking reference existence: {}", reference);

    let response = client.head(&test_url).send().await.map_err(|e| {
        StoreError::NetworkError(format!(
            "Failed to check reference {} in {}/{}: {}",
            reference, owner, repo, e
        ))
    })?;

    Ok(response.status().is_success())
}

/// Resolve a GitReference to a concrete branch/tag/commit string
pub async fn resolve_git_reference(
    client: &Client,
    owner: &str,
    repo: &str,
    reference: &GitReference,
) -> Result<String> {
    match reference {
        GitReference::Default => {
            // First try: Use GitHub API to get the actual default branch
            match get_repository_info(client, owner, repo).await {
                Ok(repo_info) => {
                    debug!(
                        "Resolved default branch via GitHub API: {}",
                        repo_info.default_branch
                    );
                    return Ok(repo_info.default_branch);
                }
                Err(e) => {
                    debug!("Failed to get repository info via GitHub API: {}", e);
                    // Continue to fallback
                }
            }

            // Fallback: Test common branches by checking if they exist
            let default_branches = ["main", "master"];
            for branch in &default_branches {
                match check_reference_exists(client, owner, repo, branch).await {
                    Ok(true) => {
                        debug!("Resolved default branch via fallback: {}", branch);
                        return Ok(branch.to_string());
                    }
                    Ok(false) => continue,
                    Err(e) => {
                        debug!("Error checking branch {}: {}", branch, e);
                        continue;
                    }
                }
            }

            // Final fallback: assume "main"
            warn!(
                "Could not resolve default branch for {}/{}, using 'main' as fallback",
                owner, repo
            );
            Ok("main".to_string())
        }
        _ => Ok(reference.to_string()),
    }
}

/// Get the names of all items in a directory (convenience function)
pub async fn list_directory_names(
    client: &Client,
    owner: &str,
    repo: &str,
    path: &str,
    reference: &str,
) -> Result<Vec<String>> {
    let contents = list_repository_contents(client, owner, repo, path, reference).await?;
    Ok(contents.into_iter().map(|item| item.name).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_from_str() {
        assert_eq!(ContentType::from_str("file"), ContentType::File);
        assert_eq!(ContentType::from_str("dir"), ContentType::Directory);
        assert_eq!(ContentType::from_str("symlink"), ContentType::Symlink);
        assert_eq!(ContentType::from_str("submodule"), ContentType::Submodule);
        assert_eq!(ContentType::from_str("unknown"), ContentType::File);
    }

    #[test]
    fn test_user_agent_constant() {
        assert!(USER_AGENT.contains("quelle-store"));
        assert!(USER_AGENT.contains("/"));
    }

    #[test]
    fn test_accept_header_constant() {
        assert_eq!(ACCEPT_HEADER, "application/vnd.github.v3+json");
    }

    #[test]
    fn test_create_default_client() {
        let _client = create_default_client();
        // Just verify the client was created successfully without panicking
    }

    #[test]
    fn test_create_authenticated_client() {
        let token = "ghp_test_token";
        let _client = create_authenticated_client(token);
        // Just verify the client was created successfully without panicking
    }
}
