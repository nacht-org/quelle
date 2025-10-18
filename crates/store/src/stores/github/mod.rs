//! GitHub store implementation with API utilities

pub mod api;
pub mod file_operations;
pub mod store;

// Re-export the main types for convenience
pub use api::{
    check_reference_exists, create_authenticated_client, create_default_client,
    get_repository_info, list_directory_names, list_repository_contents, resolve_git_reference,
    ContentItem, ContentType, RepositoryInfo, ACCEPT_HEADER, USER_AGENT,
};
pub use file_operations::{GitHubFileOperations, GitHubFileOperationsBuilder};
pub use store::{GitHubStore, GitHubStoreBuilder};
