//! HTML to Typst conversion utilities
//!
//! This module provides converters for transforming HTML content into Typst markup,
//! serving as fallbacks when external tools like Pandoc are not available.

#[cfg(feature = "pdf")]
pub mod html_to_typst;

// Re-export main types and functions
#[cfg(feature = "pdf")]
pub use html_to_typst::{
    convert_html_to_typst, convert_html_to_typst_with_config, ConversionConfig,
    HtmlToTypstConverter,
};
