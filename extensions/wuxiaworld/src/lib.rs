use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;
use serde::Deserialize;

use crate::proto::{
    get_chapter_by_property::{ByNovelAndChapterSlug, ByProperty},
    get_novel_request::Selector,
    ChapterItem, DecimalValue, GetChapterByProperty, GetChapterListRequest, GetChapterListResponse,
    GetChapterRequest, GetNovelRequest, GetNovelResponse, Timestamp,
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

        tracing::info!("Fetching novel info for slug: {}", slug);

        let response = Request::post(format!("{API_URL}Novels/GetNovel"))
            .grpc(&GetNovelRequest {
                selector: Some(Selector::Slug(slug.to_string())),
            })?
            .send(&self.client)?;

        let novel_data = response
            .grpc::<GetNovelResponse>()?
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

        let max_free_chapter = novel_data
            .karma_info
            .and_then(|ki| ki.max_free_chapter)
            .map(f64::from);

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
            volumes: fetch_volumes(&self.client, novel_data.id, url.clone(), max_free_chapter)?,
            metadata,
            status,
            langs: META.langs.clone(),
        };

        Ok(novel)
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        let url_parts: Vec<&str> = url.trim_end_matches('/').rsplit('/').collect();

        let novel_slug = url_parts
            .get(1)
            .ok_or_else(|| eyre::eyre!("Invalid chapter URL"))?;
        let chapter_slug = url_parts
            .first()
            .ok_or_else(|| eyre::eyre!("Invalid chapter URL"))?;

        let response = Request::post(format!("{API_URL}Chapters/GetChapter"))
            .grpc(&GetChapterRequest {
                chapter_property: Some(GetChapterByProperty {
                    by_property: Some(ByProperty::Slugs(ByNovelAndChapterSlug {
                        novel_slug: novel_slug.to_string(),
                        chapter_slug: chapter_slug.to_string(),
                    })),
                }),
            })?
            .send(&self.client)?;

        let chapter_data = response
            .grpc::<proto::GetChapterResponse>()?
            .item
            .ok_or_else(|| eyre::eyre!("Chapter not found"))?;

        let content = chapter_data.content.map(|c| c.value).unwrap_or_default();
        if content.is_empty() {
            return Err(eyre::eyre!("Chapter content is empty"));
        }

        Ok(ChapterContent { data: content })
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        let response = Request::get(format!("{BASE_URL}/api/novels/search"))
            .param("query", query.query)
            .send(&self.client)?;

        let data = response
            .data
            .ok_or_else(|| eyre::eyre!("No data in search response"))?;

        tracing::info!("Search response data: {:?}", String::from_utf8_lossy(&data));

        let novels = parse_novels(data)?;

        Ok(SearchResult {
            novels,
            total_count: None,
            current_page: 1,
            total_pages: None,
            has_next_page: false,
            has_previous_page: false,
        })
    }
}

fn fetch_volumes(
    client: &Client,
    novel_id: i32,
    novel_url: String,
    max_free_chapter: Option<f64>,
) -> Result<Vec<Volume>, eyre::Report> {
    let response = Request::post(format!("{API_URL}Chapters/GetChapterList"))
        .grpc(&GetChapterListRequest {
            novel_id,
            filter: None,
            count: None,
        })?
        .send(client)?;

    let response_volumes = response.grpc::<GetChapterListResponse>()?.items;

    let mut volumes = Vec::new();
    for (i, volume_data) in response_volumes.into_iter().enumerate() {
        let mut volume = Volume {
            index: i as i32,
            name: volume_data.title,
            chapters: vec![],
        };

        for chapter_data in volume_data.chapter_list {
            // Skip locked chapters for now (FIXME: implement chapter status)
            if !is_chapter_unlocked(&chapter_data, max_free_chapter) {
                continue;
            }

            let url = format!("{}/{}", novel_url.trim_end_matches("/"), chapter_data.slug);

            let chapter = Chapter {
                title: chapter_data.name,
                index: chapter_data.offset,
                url,
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

fn is_chapter_unlocked(chapter_item: &ChapterItem, max_free_chapter: Option<f64>) -> bool {
    let mut is_chapter_unlocked = chapter_item
        .related_user_info
        .and_then(|ruu| ruu.is_chapter_unlocked)
        .map(|bool_value| bool_value.value)
        .unwrap_or(false);

    if !is_chapter_unlocked {
        let is_chapter_free = max_free_chapter
            .and_then(|max_free| {
                chapter_item
                    .number
                    .map(|chapter_number| (max_free, f64::from(chapter_number)))
            })
            .map(|(max_free, chapter_number)| chapter_number <= max_free)
            .unwrap_or(true);

        is_chapter_unlocked = is_chapter_free;
    }

    is_chapter_unlocked
}

fn parse_novels(data: Vec<u8>) -> eyre::Result<Vec<BasicNovel>> {
    #[derive(Deserialize)]
    pub struct ApiSearchResponse {
        pub items: Vec<ListNovel>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ListNovel {
        pub name: String,
        pub slug: String,
        pub cover_url: Option<String>,
    }

    let response: ApiSearchResponse = serde_json::from_slice(&data)
        .map_err(|e| eyre::eyre!("Failed to parse novel list: {}", e))?;

    let basic_novels = response
        .items
        .into_iter()
        .map(|novel| BasicNovel {
            title: novel.name,
            url: format!("{}/novel/{}", BASE_URL, novel.slug),
            cover: novel.cover_url,
        })
        .collect();

    Ok(basic_novels)
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

impl From<DecimalValue> for f64 {
    fn from(value: DecimalValue) -> Self {
        value.units as f64 + (value.nanos as f64 / 1_000_000_000.0)
    }
}
