use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

use crate::proto::{
    get_novel_request::Selector, GetChapterListRequest, GetChapterListResponse, GetNovelRequest,
    GetNovelResponse, Timestamp,
};

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/wuxiaworld.api.v2.rs"));
}

register_extension!(Extension);

const BASE_URL: &str = "https://www.wuxiaworld.com";
const API_URL: &str = "https://api2.wuxiaworld.com/wuxiaworld.api.v2.";

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.wuxiaworld"),
    name: String::from("Wuxiaworld"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![BASE_URL.to_string()],
    rds: vec![ReadingDirection::Ltr],
    attrs: vec![],
    capabilities: SourceCapabilities {
        search: Some(SearchCapabilities {
            supports_simple_search: true,
            supports_complex_search: false,
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
        let slug = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .ok_or_else(|| eyre::eyre!("Invalid novel URL"))?;

        let response = Request::post(format!("{API_URL}Novels/GetNovel"))
            .protobuf(&GetNovelRequest {
                selector: Some(Selector::Slug(slug.to_string())),
            })?
            .add_grpc_web_headers()
            .send(&self.client)?;

        let novel_data = response
            .protobuf::<GetNovelResponse>()?
            .item
            .ok_or_else(|| eyre::eyre!("Novel not found"))?;

        let status = novel_data.status().into();

        let mut authors = Vec::new();
        if let Some(author) = novel_data.author_name {
            authors.push(author.value);
        }

        let mut metadata = Vec::new();
        for genre in novel_data.genres {
            metadata.push(Metadata::new("subject".to_string(), genre, None));
        }

        if let Some(translator) = novel_data.translator_name {
            metadata.push(Metadata::new(
                "translator".to_string(),
                translator.value,
                None,
            ));
        }

        let novel = Novel {
            title: novel_data.name,
            url: url.clone(),
            authors,
            cover: novel_data.cover_url.map(|url| url.value),
            description: novel_data
                .synopsis
                .map(|desc| {
                    desc.value
                        .split('\n')
                        .map(|p| p.trim().to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            volumes: fetch_volumes(&self.client, novel_data.id, url.clone())?,
            metadata: metadata,
            status,
            langs: META.langs.clone(),
        };

        Ok(novel)
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

fn fetch_volumes(
    client: &Client,
    novel_id: i32,
    novel_url: String,
) -> Result<Vec<Volume>, eyre::Report> {
    let response = Request::post(format!("{API_URL}Novels/SearchNovels"))
        .protobuf(&GetChapterListRequest {
            novel_id,
            filter: None,
            count: None,
        })?
        .add_grpc_web_headers()
        .send(client)?;

    let response_volumes = response.protobuf::<GetChapterListResponse>()?.items;

    let mut volumes = Vec::new();
    for volume_data in response_volumes {
        let mut volume = Volume {
            index: volume_data.order,
            name: volume_data.title,
            chapters: vec![],
        };

        // TODO: check if chapter is free or premium

        for chapter_data in volume_data.chapter_list {
            let url = format!("{}/{}", novel_url.trim_end_matches("/"), chapter_data.slug);

            let chapter = Chapter {
                title: chapter_data.name,
                index: chapter_data.offset,
                url: url,
                updated_at: chapter_data
                    .published_at
                    .and_then(|ts| DateTime::<Utc>::try_from(ts).ok())
                    .map(|dt| dt.to_rfc3339()),
            };

            volume.chapters.push(chapter);
        }

        volumes.push(volume);
    }

    Ok(volumes)
}

impl From<proto::novel_item::Status> for NovelStatus {
    fn from(value: proto::novel_item::Status) -> Self {
        match value {
            proto::novel_item::Status::Finished => NovelStatus::Completed,
            proto::novel_item::Status::Active => NovelStatus::Ongoing,
            proto::novel_item::Status::Hiatus => NovelStatus::Hiatus,
            proto::novel_item::Status::All => NovelStatus::Unknown,
        }
    }
}

impl TryFrom<Timestamp> for DateTime<Utc> {
    type Error = ();

    fn try_from(ts: Timestamp) -> Result<Self, Self::Error> {
        DateTime::from_timestamp(ts.seconds, ts.nanos.try_into().unwrap_or(0)).ok_or(())
    }
}
