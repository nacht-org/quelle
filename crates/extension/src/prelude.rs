pub use crate::common::net::make_absolute_url;
pub use crate::common::scraping::*;
pub use crate::http::{Client, Method, Request};
pub use crate::novel::{
    BasicNovel, Chapter, ChapterContent, ComplexSearchQuery, Metadata, Novel, NovelStatus,
    SearchResult, SimpleSearchQuery, Volume,
};
pub use crate::register_extension;
pub use crate::source::{ReadingDirection, SearchCapabilities, SourceCapabilities, SourceMeta};
pub use crate::{QuelleExtension, RequestFormBuilder};
