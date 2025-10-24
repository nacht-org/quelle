use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

register_extension!(Extension);

const BASE_URL: &str = "https://www.novelpub.com";

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.novelpub"),
    name: String::from("Novelpub"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![BASE_URL.to_string()],
    rds: vec![ReadingDirection::Ltr],
    attrs: vec![],
    capabilities: SourceCapabilities {
        search: Some(SearchCapabilities {
            supports_simple_search: true,
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
