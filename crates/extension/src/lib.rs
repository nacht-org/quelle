mod modules;
mod register;

pub mod prelude;

use crate::register::extension;
use crate::wit::*;

pub use modules::error::install_panic_hook;
pub use modules::http::RequestFormBuilder;
pub use register::{register_extension_internal, register_tracing};
pub use wit::quelle::extension::{error, http, novel, source};

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

    fn init() -> Result<(), error::Error> {
        extension().init().map_err(Into::into)
    }

    fn fetch_novel_info(url: String) -> Result<wit::Novel, error::Error> {
        extension().fetch_novel_info(url).map_err(Into::into)
    }

    fn fetch_chapter(url: String) -> Result<wit::ChapterContent, error::Error> {
        extension().fetch_chapter(url).map_err(Into::into)
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
    fn init(&self) -> Result<(), eyre::Report>;

    /// Fetches novel information from the given URL.
    fn fetch_novel_info(&self, url: String) -> Result<wit::Novel, eyre::Report>;

    /// Fetches chapter content from the given URL.
    fn fetch_chapter(&self, url: String) -> Result<wit::ChapterContent, eyre::Report>;
}
