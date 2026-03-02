use eyre::{Context, OptionExt, eyre};
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;
use regex::Regex;

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
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch novel page")?;

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
            status: NovelStatus::Unknown,
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

        // RoyalRoad injects hidden spans via dynamically-generated CSS classes as an
        // anti-scraping measure. Extract those class names from inline <style> blocks
        // and pass them to the cleaner so they get removed along with everything else.
        let hidden_class_re =
            Regex::new(r"(\.[a-zA-Z][a-zA-Z0-9_-]*)\{[^}]*display:none[^}]*speak:never[^}]*\}")
                .map_err(|e| eyre!("Regex error: {}", e))?;

        let mut cleaner = ContentCleaner::new();

        for style_element in doc.select("head > style")? {
            let clean_css = style_element
                .text_or_empty()
                .replace(&[' ', '\n', '\r', '\t'][..], "");

            for cap in hidden_class_re.captures_iter(&clean_css) {
                if let Some(class) = cap.get(1) {
                    cleaner = cleaner.remove(class.as_str());
                }
            }
        }

        let content = doc.select_first(".chapter .chapter-content")?;
        Ok(ChapterContent {
            data: cleaner.clean(&content)?,
        })
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        let current_page = query.page();
        let search_query = query.query.to_lowercase().replace(" ", "+");
        let search_url = format!(
            "https://www.royalroad.com/fictions/search?keyword={}",
            search_query
        );

        let doc = Request::get(&search_url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch search results")?;

        let mut novels = Vec::new();

        // Extract search results — limit to first 5 as per original behaviour
        for element in doc.select("h2.fiction-title a[href]")?.into_iter().take(5) {
            let title = element.text_or_empty();
            let url = element
                .attr_opt("href")
                .map(|href| make_absolute_url(&href, BASE_URL))
                .ok_or_eyre("Failed to get novel URL")?;

            let cover = None;

            novels.push(BasicNovel { title, cover, url });
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

fn extract_chapters(_client: &Client, doc: &Html) -> Result<Vec<Volume>, eyre::Report> {
    let mut volume = Volume::default();

    // Royal Road chapter structure — look for table rows containing chapter links
    let chapter_rows = doc.select("table tbody tr")?;

    for (index, row) in chapter_rows.into_iter().enumerate() {
        // Look for chapter link — Royal Road puts chapter links in the first column
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
