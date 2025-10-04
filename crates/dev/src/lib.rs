//! Development tools for Quelle extensions
//!
//! This crate provides development commands and utilities for building,
//! testing, and debugging Quelle extensions. It includes:
//!
//! - **Extension Generator**: Create new extensions from templates with interactive prompts
//! - **Development Server**: Hot-reloading server for rapid extension development
//! - **Interactive Testing**: Test extension functionality with a REPL-style interface
//! - **Validation Tools**: Ensure extensions meet quality and correctness standards
//!
//! ## Usage
//!
//! The main entry point is the [`commands::handle_command`] function, which dispatches
//! to the appropriate command handler based on the [`commands::DevCommands`] enum.
//!
//! ```rust,no_run
//! use quelle_dev::{commands::{DevCommands, handle_command}};
//!
//! # async fn example() -> eyre::Result<()> {
//! let cmd = DevCommands::Generate {
//!     name: Some("my_extension".to_string()),
//!     display_name: Some("My Extension".to_string()),
//!     base_url: Some("https://example.com".to_string()),
//!     language: None,
//!     reading_direction: None,
//!     force: false,
//! };
//!
//! handle_command(cmd).await?;
//! # Ok(())
//! # }
//! ```

// Re-export commonly used types and functions
pub use commands::{DevCommands, handle_command};

pub mod commands;
pub mod generator;
pub mod http_caching;
pub mod server;
pub mod utils;

// Re-export key utilities for external usage
pub use utils::{debug, find_extension_path, find_project_root, fs, validation};
