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

        let mut doc = Html::new(&text);

        // Outside the loop regex compilation to avoid recompiling on each iteration
        let hidden_class_re = regex::Regex::new(
            r"(\.[a-zA-Z][a-zA-Z0-9_-]*)\{[^}]*display:none[^}]*speak:never[^}]*\}",
        )
        .map_err(|e| eyre!("Regex error: {}", e))?;

        // Find hidden class patterns from CSS (anti-scraping protection)
        let mut hidden_classes = Vec::new();
        for style_element in doc.select("head > style")? {
            let css_text = style_element.text_or_empty();

            // Remove whitespace and newlines for easier parsing
            let clean_css = css_text.replace(&[' ', '\n', '\r', '\t'][..], "");

            // Look for patterns like .abc123{display:none;speak:never;}
            for cap in hidden_class_re.captures_iter(&clean_css) {
                if let Some(class) = cap.get(1) {
                    hidden_classes.push(class.as_str().to_string());
                }
            }
        }

        tracing::debug!(
            "Found {} hidden classes: {:?}",
            hidden_classes.len(),
            hidden_classes
        );

        // Remove hidden elements based on CSS patterns (anti-scraping protection)
        if !hidden_classes.is_empty() {
            // Create selector string for all hidden classes
            let hidden_selector = hidden_classes.join(", ");

            if let Ok(hidden_elements) = doc.select(&hidden_selector) {
                // Collect node IDs of elements to remove
                let mut node_ids_to_remove = Vec::new();
                for element in hidden_elements {
                    node_ids_to_remove.push(element.element.id());
                }

                tracing::debug!(
                    "Removing {} hidden elements from chapter content",
                    node_ids_to_remove.len()
                );

                // Remove each hidden element using proper node detachment
                for node_id in node_ids_to_remove {
                    doc.detach(node_id);
                }
            }
        }

        // Get chapter content after hidden element removal
        let cleaned_content = doc.select_first(".chapter .chapter-content").html()?;

        Ok(ChapterContent {
            data: cleaned_content,
        })
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
