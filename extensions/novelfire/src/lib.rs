use eyre::{eyre, Context, OptionExt};
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

register_extension!(Extension);

const BASE_URL: &str = "https://novelfire.net";

static META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.novelfire"),
    name: String::from("Novel Fire"),
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
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch novel page")?;

        let title = doc.select_first("h1")?.text_or_empty();

        let cover = doc
            .select_first_opt("img[src*='server-']")?
            .and_then(|img| img.attr_opt("src"))
            .map(|src| make_absolute_url(&src, BASE_URL));

        let authors = doc
            .select(r#"a[href*="/author/"]"#)?
            .into_iter()
            .filter_map(|a| a.text_opt())
            .collect::<Vec<_>>();

        let status = doc
            .select("span")?
            .into_iter()
            .find_map(|el| {
                el.text_opt()
                    .and_then(|t| t.trim().parse::<NovelStatus>().ok())
            })
            .unwrap_or(NovelStatus::Unknown);

        let mut metadata = Vec::new();
        for genre_link in doc.select(r#"a[href*="/genre-"]"#)? {
            if let Some(text) = genre_link.text_opt() {
                metadata.push(Metadata::new("subject".into(), text, None));
            }
        }

        let description = doc
            .select_first_opt(
                ".summary, [class*='summary'], [class*='synopsis'], [class*='description']",
            )?
            .map(|el| {
                el.text_or_empty()
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let volumes = fetch_all_chapters(&self.client, &url)?;

        Ok(Novel {
            url,
            title,
            authors,
            cover,
            description,
            status,
            metadata,
            volumes,
            langs: META.langs.clone(),
        })
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch chapter page")?;

        let content = doc
            .select_first_opt("#chapter-container")?
            .or(doc.select_first_opt(".chapter-content")?)
            .or(doc.select_first_opt("[class*='chapter-content']")?)
            .or(doc.select_first_opt("[class*='chapter-body']")?)
            .ok_or_eyre("Could not locate chapter content element")?;

        let data = ContentCleaner::new()
            .remove("[class*='ads']")
            .remove("[class*='ad-']")
            .remove("[class*='donate']")
            .remove("[class*='share']")
            .remove("[class*='report']")
            .remove("[class*='tip']")
            .remove("img[src*='giphy']")
            .remove("#scroll-percent")
            .remove("#restore-scroll-btn")
            .remove_tag("aside")
            .clean(&content)?;

        Ok(ChapterContent { data })
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        let current_page = query.page();

        let doc = Request::get(format!("{BASE_URL}/search"))
            .param("keyword", &query.query)
            .param("page", current_page.to_string())
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch search results")?;

        let mut novels = Vec::new();

        for card_link in doc.select(r#"a[href*="/book/"]"#)? {
            let href = match card_link.attr_opt("href") {
                Some(h) if h.contains("/book/") && !h.contains("/chapter") => h,
                _ => continue,
            };

            let title = match card_link.select_first_opt("h4, h3, h2, [class*='title']")? {
                Some(el) => el.text_or_empty(),
                None => match card_link.text_opt() {
                    Some(t) if !t.trim().is_empty() => t,
                    _ => continue,
                },
            };

            let cover = card_link
                .select_first_opt("img")?
                .and_then(|img| img.attr_opt("src"))
                .map(|src| make_absolute_url(&src, BASE_URL));

            novels.push(BasicNovel {
                title,
                url: make_absolute_url(&href, BASE_URL),
                cover,
            });
        }

        novels.dedup_by(|a, b| a.url == b.url);

        let total_pages = parse_total_pages(&doc).unwrap_or(current_page);

        Ok(SearchResult {
            novels,
            total_count: None,
            current_page,
            total_pages: Some(total_pages),
            has_next_page: current_page < total_pages,
            has_previous_page: current_page > 1,
        })
    }
}

fn fetch_all_chapters(client: &Client, novel_url: &str) -> eyre::Result<Vec<Volume>> {
    let mut volume = Volume::default();
    let mut page = 1u32;
    let mut global_index = 0i32;

    loop {
        let chapters_url = format!("{}/chapters?page={}", novel_url.trim_end_matches('/'), page);

        let doc = Request::get(&chapters_url)
            .html(client)
            .map_err(|e| eyre!(e))
            .wrap_err_with(|| format!("Failed to fetch chapters page {page}"))?;

        let rows = doc.select("ul li a[href*='/chapter-']")?;

        if rows.is_empty() {
            break;
        }

        for link in rows {
            let href = match link.attr_opt("href") {
                Some(h) => h,
                None => continue,
            };

            if !href.contains("/chapter-") {
                continue;
            }

            let title = link
                .select_first_opt("strong, b")?
                .and_then(|el| el.text_opt())
                .or_else(|| link.text_opt())
                .unwrap_or_default();

            volume.chapters.push(Chapter {
                index: global_index,
                title,
                url: make_absolute_url(&href, BASE_URL),
                updated_at: None,
            });

            global_index += 1;
        }

        let has_next = doc
            .select("a[href*='chapters?page=']")?
            .into_iter()
            .any(|a| {
                a.attr_opt("href")
                    .and_then(|h| h.split("page=").nth(1).and_then(|p| p.parse::<u32>().ok()))
                    .map(|p| p > page)
                    .unwrap_or(false)
            });

        if !has_next {
            break;
        }

        page += 1;
        tracing::debug!("Fetching chapters page {page}");
    }

    Ok(vec![volume])
}

fn parse_total_pages(doc: &Html) -> Option<u32> {
    doc.select("a[href*='page=']")
        .ok()?
        .into_iter()
        .filter_map(|a| {
            a.attr_opt("href").and_then(|h| {
                h.split("page=")
                    .nth(1)
                    .and_then(|p| p.split('&').next())
                    .and_then(|p| p.parse::<u32>().ok())
            })
        })
        .max()
}
