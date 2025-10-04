use eyre::{OptionExt, eyre};
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

register_extension!(Extension);

const BASE_URL: &str = "https://www.royalroad.com";

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.royalroad"),
    name: String::from("Royal Road"),
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
        let response = Request::get(&url)
            .send(&self.client)
            .map_err(|e| eyre!(e))?
            .error_for_status()?;

        let text = response
            .text()?
            .ok_or_else(|| eyre!("Failed to get data"))?;

        let doc = Html::new(&text);

        // Extract novel title
        let title = doc.select_first(".fic-header h1")?.text_or_empty();

        // Extract cover image
        let cover = doc
            .select_first_opt(".fic-header img.thumbnail")?
            .and_then(|img| img.attr_opt("src"))
            .map(|src| make_absolute_url(&src, BASE_URL));

        // Extract authors
        let authors = doc
            .select(r#".fic-header a[href^="/profile/"]"#)?
            .into_iter()
            .filter_map(|a| a.text_opt())
            .collect::<Vec<_>>();

        // Extract synopsis/description
        let description = doc
            .select_first_opt(".fiction-info .description .hidden-content")?
            .map(|desc| vec![desc.text_or_empty()])
            .unwrap_or_default();

        // Extract tags
        let mut metadata = Vec::new();
        for tag in doc.select(r#".fiction-info .tags a[href^="/fictions/search"]"#)? {
            metadata.push(Metadata::new(
                String::from("subject"),
                tag.text_or_empty(),
                None,
            ));
        }

        // Extract chapters
        let volumes = extract_chapters(&self.client, &doc)?;

        let novel = Novel {
            title,
            authors,
            description,
            langs: META.langs.clone(),
            cover,
            status: NovelStatus::Unknown, // RoyalRoad doesn't have a clear status indicator
            volumes,
            metadata,
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

        // Find hidden class patterns from CSS (anti-scraping protection)
        let mut hidden_classes = Vec::new();
        for style_element in doc.select("head > style")? {
            let css_text = style_element.text_or_empty();
            // Look for patterns like .abc123{display:none;speak:never;}
            let re = regex::Regex::new(r"(\.[a-zA-Z0-9]+)\{display:none;speak:never;\}")
                .map_err(|e| eyre!("Regex error: {}", e))?;

            for cap in re.captures_iter(&css_text) {
                if let Some(class) = cap.get(1) {
                    hidden_classes.push(class.as_str().to_string());
                }
            }
        }

        // Get chapter content and extract HTML
        let content_element = doc.select_first(".chapter .chapter-content")?;
        let content_html = content_element.html_opt().unwrap_or_default();

        // For now, we'll return the content as-is since removing hidden elements
        // requires DOM manipulation which isn't easily available in the scraper API
        // The anti-scraping protection removal would need a different approach

        Ok(ChapterContent { data: content_html })
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        let current_page = query.page();
        let search_query = query.query.to_lowercase().replace(" ", "+");
        let search_url = format!(
            "https://www.royalroad.com/fictions/search?keyword={}",
            search_query
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

        // Extract search results - limit to first 5 as per original Python code
        for element in doc.select("h2.fiction-title a[href]")?.into_iter().take(5) {
            let title = element.text_or_empty();
            let url = element
                .attr_opt("href")
                .map(|href| make_absolute_url(&href, BASE_URL))
                .ok_or_eyre("Failed to get novel URL")?;

            // For now, we'll skip cover images in search results as they require
            // more complex DOM traversal that would need additional scraping
            let cover = None;

            novels.push(BasicNovel { title, cover, url });
        }

        Ok(SearchResult {
            novels,
            total_count: None,
            current_page,
            total_pages: Some(1), // RoyalRoad search doesn't seem to have pagination in the basic search
            has_next_page: false,
            has_previous_page: false,
        })
    }
}

fn extract_chapters(_client: &Client, doc: &Html) -> Result<Vec<Volume>, eyre::Report> {
    let mut volume = Volume::default();

    // Royal Road chapter structure - look for table rows containing chapter links
    let chapter_rows = doc.select("table tbody tr")?;

    for (index, row) in chapter_rows.into_iter().enumerate() {
        // Look for chapter link - Royal Road puts chapter links in the first column
        let link_opt = row.select_first_opt("td:first-child a[href]")?;

        let Some(link) = link_opt else {
            continue; // Skip rows without chapter links
        };

        // Only process links that are actually chapter links
        let href = link.attr_opt("href").unwrap_or_default();
        if !href.contains("/chapter/") {
            continue;
        };

        let title = link.text_or_empty();
        let url = link
            .attr_opt("href")
            .map(|href| make_absolute_url(&href, BASE_URL))
            .ok_or_eyre("Failed to get chapter URL")?;

        // Extract publish date from the time element's datetime attribute
        let updated_at = row
            .select_first_opt("time[datetime]")?
            .and_then(|time_elem| time_elem.attr_opt("datetime"));

        let chapter = Chapter {
            index: index as i32,
            title,
            url,
            updated_at,
        };

        volume.chapters.push(chapter);
    }

    Ok(vec![volume])
}
