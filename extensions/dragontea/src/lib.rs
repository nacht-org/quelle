use eyre::{eyre, Context};
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

register_extension!(Extension);

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.dragontea"),
    name: String::from("Dragon Tea"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![String::from("https://dragontea.ink")],
    rds: vec![ReadingDirection::Ltr],
    attrs: vec![],
    capabilities: SourceCapabilities::default(),
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
            .wait_for_element(".post-title")
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch novel page")?;

        // Extract basic novel info
        let title = doc.select_first(".post-title").text()?;
        let author = doc.select_first(".author-content").text()?;

        let thumbnail_url = doc
            .select_first(".summary_image img")?
            .attr("src")
            .ok()
            .map(|src| make_absolute_url(&src, &url))
            .unwrap_or_default();

        // Extract synopsis with text reordering
        let mut description = Vec::new();
        for element in doc.select(".summary__content > p")? {
            let text = jumble::reorder_text(&element.text_or_empty());
            if !text.trim().is_empty() {
                description.push(text);
            }
        }

        let mut metadata = Vec::new();

        // Extract metadata from post-content items
        for item in doc.select(".post-content_item")? {
            if let (Ok(key_elem), Ok(value_elem)) = (
                item.select_first(".summary-heading"),
                item.select_first(".summary-content"),
            ) {
                let key = key_elem.text_or_empty();
                let value = value_elem.text_or_empty();

                match key.as_str() {
                    "Alternative" => {
                        metadata.push(Metadata::new(
                            String::from("title"),
                            value,
                            Some(vec![(String::from("role"), String::from("alt"))]),
                        ));
                    }
                    "Type" => {
                        metadata.push(Metadata::new(String::from("type"), value, None));
                    }
                    _ => {}
                }
            }
        }

        // Extract genres
        for genre in doc.select(".genres-content > a")? {
            metadata.push(Metadata::new(
                String::from("subject"),
                genre.text_or_empty(),
                None,
            ));
        }

        // Extract tags
        for tag in doc.select(".tags-content > a")? {
            metadata.push(Metadata::new(
                String::from("tag"),
                tag.text_or_empty(),
                None,
            ));
        }

        // Extract artist if available
        if let Ok(artist) = doc.select_first(".artist-content > a") {
            metadata.push(Metadata::new(
                String::from("contributor"),
                artist.text_or_empty(),
                Some(vec![(String::from("role"), String::from("ill"))]),
            ));
        }

        // Fetch chapters via POST request
        let chapters_url = format!("{}/ajax/chapters/", url.trim_end_matches('/'));
        let chapters_response = Request::post(&chapters_url)
            .send(&self.client)
            .map_err(|e| eyre!(e))?
            .error_for_status()?;

        let chapters_text = chapters_response
            .text()?
            .ok_or_else(|| eyre!("Failed to get chapters data"))?;

        let chapters_doc = Html::new(&chapters_text);
        let mut chapters = Vec::new();

        // Extract chapters in reverse order (as in original Python code)
        let chapter_links: Vec<_> = chapters_doc
            .select(".wp-manga-chapter > a")?
            .into_iter()
            .collect();

        for (index, chapter_link) in chapter_links.into_iter().rev().enumerate() {
            let chapter_title = chapter_link.text_or_empty();
            let chapter_url = chapter_link.attr_opt("href").unwrap_or_default();
            let absolute_chapter_url = make_absolute_url(&chapter_url, &url);

            chapters.push(Chapter {
                index: index as i32,
                title: chapter_title,
                url: absolute_chapter_url,
                updated_at: None,
            });
        }

        let volume = Volume {
            index: 0,
            name: String::new(),
            chapters,
        };

        let novel = Novel {
            title,
            authors: vec![author],
            description,
            langs: META.langs.clone(),
            cover: Some(thumbnail_url),
            status: NovelStatus::Unknown,
            volumes: vec![volume],
            metadata,
            url,
        };

        Ok(novel)
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        // Convert HTTPS to HTTP as in original Python code
        let http_url = url.replace("https:", "http:");

        let mut html = Request::get(&http_url)
            .wait_for_element(".reading-content")
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch chapter page")?;

        // TODO: Clean and reorder text content
        let content = html
            .select_first_opt(".text-left")?
            .or_else(|| html.select_first_opt(".reading-content").ok().flatten())
            .ok_or_else(|| eyre!("Could not find chapter content"))?;

        let text_nodes = content
            .element
            .descendants()
            .filter(|node| node.value().is_text())
            .map(|node| node.id())
            .collect::<Vec<_>>();

        jumble::reorder_html_text(&mut html.doc, text_nodes);

        // Re-select content after modifications
        let content = html
            .select_first_opt(".text-left")?
            .or_else(|| html.select_first_opt(".reading-content").ok().flatten())
            .ok_or_else(|| eyre!("Could not find chapter content"))?;

        Ok(ChapterContent {
            data: content
                .html_opt()
                .ok_or_else(|| eyre!("Failed to extract chapter HTML"))?,
        })
    }
}

mod jumble {
    use ego_tree::NodeId;
    use once_cell::sync::Lazy;
    use scraper::Html;
    use std::collections::HashMap;

    // Character mapping for text jumbling/reordering
    const NORMAL_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    const JUMBLED_CHARS: &str = "ZYXWVUTSRQPONMLKJIHGFEDCBAzyxwvutsrqponmlkjihgfedcba";

    static JUMBLE_MAP: Lazy<HashMap<char, char>> =
        Lazy::new(|| JUMBLED_CHARS.chars().zip(NORMAL_CHARS.chars()).collect());

    pub fn reorder_text(text: &str) -> String {
        text.chars()
            .map(|c| JUMBLE_MAP.get(&c).copied().unwrap_or(c))
            .collect()
    }

    pub fn reorder_html_text(doc: &mut Html, node_ids: Vec<NodeId>) {
        for node_id in node_ids {
            if let Some(mut node) = doc.tree.get_mut(node_id) {
                if let scraper::Node::Text(text) = node.value() {
                    let reordered = reorder_text(&text.text);
                    text.text = reordered.into();
                }
            }
        }
    }
}
