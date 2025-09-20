use eyre::eyre;
use once_cell::sync::Lazy;
use quelle_extension::{common::time::parse_date_or_relative_time, prelude::*};
use urlencoding;

register_extension!(Extension);

const BASE_URL: &str = "https://www.scribblehub.com";

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.scribblehub"),
    name: String::from("ScribbleHub"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![String::from(BASE_URL)],
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
        let response = Request::get(&url)
            .send(&self.client)
            .map_err(|e| eyre!(e))?
            .error_for_status()?;

        let text = response
            .text()?
            .ok_or_else(|| eyre!("Failed to get data"))?;

        let doc = Html::new(&text);

        let id = url
            .split("/")
            .nth(4)
            .ok_or_else(|| eyre!("The url does not have an id"))?;

        let novel = Novel {
            title: doc.select_first("div.fic_title").text()?,
            authors: doc.select("span.auth_name_fic").text_all()?,
            description: doc.select(".wi_fic_desc > p").text_all()?,
            langs: META.langs.clone(),
            cover: doc.select_first(".fic_image img").attr_opt("src")?,
            status: doc
                .select_first(".widget_fic_similar > li:last-child > span:last-child")
                .map(|node| {
                    let text = node.text_or_empty();
                    text.split_once("-")
                        .map(|(status, _)| status.trim().to_string())
                        .unwrap_or(text)
                })
                .map(|text| NovelStatus::from_str(&text))
                .unwrap_or(NovelStatus::Unknown),
            volumes: volumes(&self.client, id)?,
            metadata: metadata(&doc)?,
            url,
        };

        Ok(novel)
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        let response = Request::get(&url)
            .send(&self.client)
            .map_err(|e| eyre!(e))?
            .error_for_status()?;

        let text = response
            .text()?
            .ok_or_else(|| eyre!("Failed to get data"))?;

        let doc = Html::new(&text);

        Ok(ChapterContent {
            data: doc.select_first("#chp_raw").html()?,
        })
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        let page = query.page.unwrap_or(1);
        let limit = query.limit.unwrap_or(20);

        // Build the search URL
        let search_url = format!(
            "https://www.scribblehub.com/series-finder/?sf=1&title_contains={}&paged={}",
            urlencoding::encode(&query.query),
            page
        );

        let response = Request::get(&search_url)
            .send(&self.client)
            .map_err(|e| eyre!(e))?
            .error_for_status()?;

        let text = response
            .text()?
            .ok_or_else(|| eyre!("Failed to get search data"))?;

        let doc = Html::new(&text);

        let mut novels = Vec::new();

        // ScribbleHub search results are displayed in a structured format
        // The results appear to be in a main content area with specific patterns

        // Look for series results - they contain rating info in parentheses
        let text = doc
            .select_first_opt("body")?
            .map(|b| b.text_or_empty())
            .unwrap_or_default();

        // Parse the text content to find series information
        // Pattern: (rating)Title Views Favorites Chapters etc.
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() && novels.len() < limit as usize {
            let line = lines[i].trim();

            // Look for rating pattern like "(4.7)" at the start of a line
            if line.starts_with('(') && line.contains(')') {
                if let Some(close_paren) = line.find(')') {
                    let title_part = &line[close_paren + 1..].trim();

                    // The title is usually the first part before view/chapter counts
                    let title = if let Some(pos) = title_part.find(" Views") {
                        title_part[..pos].trim()
                    } else if let Some(pos) = title_part.find(" Chapters") {
                        title_part[..pos].trim()
                    } else {
                        title_part
                    };

                    if !title.is_empty() && title.len() > 2 {
                        // Try to find the corresponding link in the HTML
                        let mut found_url = None;

                        for link in doc.select("a[href*='/series/']")? {
                            let link_title = link.text_or_empty();
                            let trimmed_title = link_title.trim();
                            if trimmed_title == title
                                || title.contains(trimmed_title)
                                || trimmed_title.contains(title)
                            {
                                found_url = Some(link.attr_opt("href").unwrap_or_default());
                                break;
                            }
                        }

                        if let Some(url) = found_url {
                            novels.push(BasicNovel {
                                title: title.to_string(),
                                cover: None, // ScribbleHub search doesn't show cover images in results
                                url: make_absolute_url(&url, BASE_URL),
                            });
                        }
                    }
                }
            }
            i += 1;
        }

        // Fallback: if no results found, try simpler link extraction
        if novels.is_empty() {
            for link in doc.select("a[href*='/series/']")? {
                let title = link.text_or_empty().trim().to_string();
                let url = link.attr_opt("href").unwrap_or_default();

                if !title.is_empty() && !url.is_empty() && title.len() > 2 {
                    novels.push(BasicNovel {
                        title,
                        cover: None,
                        url: make_absolute_url(&url, BASE_URL),
                    });

                    if novels.len() >= limit as usize {
                        break;
                    }
                }
            }
        }

        // Calculate pagination info
        let total_count = None; // ScribbleHub doesn't provide total count easily
        let current_page = page;
        let has_next_page = novels.len() as u32 >= limit;
        let has_previous_page = current_page > 1;

        Ok(SearchResult {
            novels,
            total_count,
            current_page,
            total_pages: None,
            has_next_page,
            has_previous_page,
        })
    }
}

fn metadata(doc: &Html) -> Result<Vec<Metadata>, eyre::Report> {
    let mut metadata = vec![];

    for node in doc.select("a.fic_genre")? {
        metadata.push(Metadata::new(
            String::from("subject"),
            node.text_or_empty(),
            None,
        ));
    }

    for node in doc.select("a.stag")? {
        metadata.push(Metadata::new(
            String::from("tag"),
            node.text_or_empty(),
            None,
        ));
    }

    for node in doc.select(".mature_contains > a")? {
        metadata.push(Metadata::new(
            String::from("warning"),
            node.text_or_empty(),
            None,
        ));
    }

    let rating_element = doc.select_first_opt("#ratefic_user > span")?;
    if let Some(element) = rating_element {
        metadata.push(Metadata::new(
            String::from("rating"),
            element.text_or_empty(),
            None,
        ));
    }

    Ok(metadata)
}

fn volumes(client: &Client, id: &str) -> Result<Vec<Volume>, eyre::Report> {
    let response = Request::post("https://www.scribblehub.com/wp-admin/admin-ajax.php")
        .body(
            RequestFormBuilder::new()
                .param("action", "wi_getreleases_pagination")
                .param("pagenum", "-1")
                .param("mypostid", id)
                .build(),
        )
        .send(client)
        .map_err(|e| eyre!(e))?
        .error_for_status()?;

    let text = response
        .text()?
        .ok_or_else(|| eyre!("Failed to get data"))?;

    let doc = Html::new(&text);
    let mut volume = Volume::default();

    for element in doc.select("li.toc_w")? {
        let Some(a) = element.select_first_opt("a")? else {
            continue;
        };

        let Some(href) = a.attr_opt("href") else {
            continue;
        };

        let time = element
            .select_first_opt(".fic_date_pub")?
            .map(|e| e.attr_opt("title"))
            .flatten();

        let updated_at = time
            .map(|time| parse_date_or_relative_time(&time, "%b %d, %Y").ok())
            .flatten()
            .map(|time| time.and_utc().to_rfc3339());

        let chapter = Chapter {
            index: volume.chapters.len() as i32,
            title: a.text_or_empty(),
            url: href.to_string(),
            updated_at,
        };

        volume.chapters.push(chapter);
    }

    Ok(vec![volume])
}
