use eyre::{eyre, Context, OptionExt};
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

register_extension!(Extension);

const BASE_URL: &str = "https://novelfull.net";

static META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.novelfull"),
    name: String::from("NovelFull"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![
        "http://novelfull.com".to_string(),
        "https://novelfull.com".to_string(),
        "https://novelfull.net".to_string(),
    ],
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
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch novel page")?;

        // Extract novel title
        let title = doc.select_first("h3.title")?.text_or_empty();

        // Extract cover image
        let cover = doc
            .select_first_opt(".book img")?
            .and_then(|img| img.attr_opt("data-src").or_else(|| img.attr_opt("src")))
            .map(|src| make_absolute_url(&src, BASE_URL));

        // Extract authors using multiple possible selectors
        let author_selectors = vec![
            ".info a[href*='/a/']",
            ".info a[href*='/au/']",
            ".info a[href*='author']",
        ];

        let mut authors = Vec::new();
        for selector in &author_selectors {
            let Ok(links) = doc.select(selector) else {
                continue;
            };

            for link in links {
                if let Some(author) = link.text_opt() {
                    authors.push(author);
                }
            }
        }

        // Extract synopsis/description
        let description = doc
            .select(".desc-text > p")?
            .into_iter()
            .flat_map(|desc| desc.text_opt())
            .collect::<Vec<_>>();

        let status = doc
            .select_first_opt("a[href*='/status']")?
            .and_then(|s| s.text_opt())
            .and_then(|s| s.parse().ok())
            .unwrap_or(NovelStatus::Unknown);

        let metadata = extract_metadata(&doc)?;
        let volumes = extract_chapters(&self.client, &doc)?;

        let novel = Novel {
            title,
            authors,
            description,
            langs: META.langs.clone(),
            cover,
            status,
            volumes,
            metadata,
            url,
        };

        Ok(novel)
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch chapter page")?;

        let content = doc
            .select_first_opt("#chr-content")?
            .or_else(|| doc.select_first_opt("#chapter-content").ok().flatten())
            .ok_or_eyre("Chapter content not found")?;

        // NovelFull embeds proxy images and misaligned divs alongside the usual noise.
        let cleaner = ContentCleaner::empty()
            .remove(r#"div[align="left"]"#)
            .remove(r#"img[src*="proxy?container=focus"]"#)
            .remove_tag("div")
            .remove_tag("script")
            .remove_tag("style");

        Ok(ChapterContent {
            data: cleaner.clean(&content)?,
        })
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        let current_page = query.page();

        let doc = Request::get(format!("{BASE_URL}/search"))
            .param("keyword", &query.query)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch search results")?;

        let mut novels = Vec::new();

        // Extract search results
        for element in doc.select("#list-page .row h3[class*='title'] > a")? {
            let title = element
                .attr_opt("title")
                .or_else(|| element.text_opt())
                .unwrap_or_default();

            let url = element
                .attr_opt("href")
                .map(|href| make_absolute_url(&href, BASE_URL))
                .ok_or_eyre("Failed to get novel URL")?;

            novels.push(BasicNovel {
                title,
                cover: None,
                url,
            });
        }

        Ok(SearchResult {
            novels,
            total_count: None,
            current_page,
            total_pages: Some(1),
            has_next_page: false,
            has_previous_page: false,
        })
    }
}

fn extract_metadata(doc: &Html) -> eyre::Result<Vec<Metadata>> {
    let mut metadata = Vec::new();

    let Some(info_section) = doc.select_first_opt(".info, .info-meta")? else {
        return Ok(metadata);
    };

    let Ok(list_items) = info_section.select("li") else {
        tracing::warn!("No list items found in info section for metadata extraction");
        return Ok(metadata);
    };

    for li in list_items {
        // Check if this li has a header with "Genre" or "Tag"
        let Ok(Some(header)) = li.select_first_opt("h3") else {
            continue;
        };

        let header_text = header.text_or_empty();
        if !header_text.contains("Genre") && !header_text.contains("Tag") {
            continue;
        }

        // Extract all links in this section
        let Ok(links) = li.select("a") else {
            continue;
        };

        for link in links {
            let tag_text = link.text_or_empty();
            if !tag_text.is_empty() {
                metadata.push(Metadata::new(String::from("subject"), tag_text, None));
            }
        }
    }

    Ok(metadata)
}

fn extract_chapters(client: &Client, doc: &Html) -> Result<Vec<Volume>, eyre::Report> {
    // Extract novel ID from the rating element
    let novel_id = doc
        .select_first("#rating[data-novel-id]")?
        .attr_opt("data-novel-id")
        .ok_or_eyre("No novel_id found")?;

    // Check which AJAX endpoint to use based on script content
    let ajax_url = if let Ok(scripts) = doc.select("script") {
        let has_ajax_chapter_option = scripts
            .into_iter()
            .any(|script| script.text_or_empty().contains("ajaxChapterOptionUrl"));

        tracing::info!(
            "Using ajax-chapter-option endpoint: {}",
            has_ajax_chapter_option
        );

        if has_ajax_chapter_option {
            format!("{}/ajax-chapter-option?novelId={}", BASE_URL, novel_id)
        } else {
            format!("{}/ajax/chapter-archive?novelId={}", BASE_URL, novel_id)
        }
    } else {
        format!("{}/ajax/chapter-archive?novelId={}", BASE_URL, novel_id)
    };

    tracing::info!("Fetching chapters from: {}", ajax_url);

    // Fetch chapter list
    let chapter_doc = Request::get(&ajax_url)
        .html(client)
        .map_err(|e| eyre!(e))
        .wrap_err("Failed to fetch chapter list")?;

    let mut volume = Volume::default();

    // Try both possible chapter selectors
    let mut chapter_links = chapter_doc.select("ul.list-chapter > li > a[href]")?;
    if chapter_links.is_empty() {
        chapter_links = chapter_doc.select("select > option[value]")?;
    }

    for (index, link) in chapter_links.into_iter().enumerate() {
        let title = link.text_or_empty();

        // Get URL from either href or value attribute
        let url = link
            .attr_opt("href")
            .or_else(|| link.attr_opt("value"))
            .map(|href| make_absolute_url(&href, BASE_URL))
            .ok_or_eyre("Failed to get chapter URL")?;

        let chapter = Chapter {
            index: index as i32,
            title,
            url,
            updated_at: None,
        };

        volume.chapters.push(chapter);
    }

    Ok(vec![volume])
}
