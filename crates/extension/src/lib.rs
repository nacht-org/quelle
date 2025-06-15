mod register;
mod tracing;

use wit::*;

use crate::register::extension;

pub use register::register_extension_internal;
pub use wit::quelle::extension::{http, novel, source};

mod wit {
    wit_bindgen::generate!({
        skip: ["register-extension"],
        path: "../../wit"
    });
}

pub struct Component;

wit::export!(Component);

impl wit::Guest for Component {
    fn meta() -> SourceMeta {
        extension().meta()
    }

    fn init() -> Result<(), String> {
        extension().init()
    }

    fn fetch_novel_info(url: String) -> Result<wit::Novel, String> {
        extension().fetch_novel_info(url)
    }

    fn fetch_chapter(url: String) -> Result<wit::ChapterContent, String> {
        extension().fetch_chapter(url)
    }
}

pub trait QuelleExtension: Send + Sync {
    /// Returns a new instance of the extension.
    fn new() -> Self
    where
        Self: Sized;

    /// Returns the metadata of the extension.
    fn meta(&self) -> SourceMeta;

    /// Initializes the extension.
    fn init(&self) -> Result<(), String>;

    /// Fetches novel information from the given URL.
    fn fetch_novel_info(&self, url: String) -> Result<wit::Novel, String>;

    /// Fetches chapter content from the given URL.
    fn fetch_chapter(&self, url: String) -> Result<wit::ChapterContent, String>;
}
