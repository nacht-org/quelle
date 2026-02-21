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

// Character mapping for text jumbling/reordering used as anti-scraping on dragontea.
// Each char in JUMBLED_CHARS maps to the corresponding char in NORMAL_CHARS.
const JUMBLED_CHARS: &str = "ZYXWVUTSRQPONMLKJIHGFEDCBAzyxwvutsrqponmlkjihgfedcba";
const NORMAL_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

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

        let title = doc.select_first(".post-title").text()?;
        let author = doc.select_first(".author-content").text()?;

        let thumbnail_url = doc
            .select_first(".summary_image img")?
            .attr("src")
            .ok()
            .map(|src| make_absolute_url(&src, &url))
            .unwrap_or_default();

        // Extract synopsis — each paragraph has jumbled text that needs remapping.
        let mut description = Vec::new();
        for element in doc.select(".summary__content > p")? {
            remap_text_nodes(&element, JUMBLED_CHARS, NORMAL_CHARS);
            let text = element.text_or_empty();
            if !text.trim().is_empty() {
                description.push(text);
            }
        }

        let mut metadata = Vec::new();

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

        for genre in doc.select(".genres-content > a")? {
            metadata.push(Metadata::new(
                String::from("subject"),
                genre.text_or_empty(),
                None,
            ));
        }

        for tag in doc.select(".tags-content > a")? {
            metadata.push(Metadata::new(
                String::from("tag"),
                tag.text_or_empty(),
                None,
            ));
        }

        if let Ok(artist) = doc.select_first(".artist-content > a") {
            metadata.push(Metadata::new(
                String::from("contributor"),
                artist.text_or_empty(),
                Some(vec![(String::from("role"), String::from("ill"))]),
            ));
        }

        // Fetch chapters via POST request.
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

        // Extract chapters in reverse order (newest-first in DOM, oldest-first in output).
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
        // Convert HTTPS to HTTP as in original Python code.
        let http_url = url.replace("https:", "http:");

        let doc = Request::get(&http_url)
            .wait_for_element(".reading-content")
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch chapter page")?;

        let content = doc
            .select_first_opt(".text-left")?
            .or_else(|| doc.select_first_opt(".reading-content").ok().flatten())
            .ok_or_else(|| eyre!("Could not find chapter content"))?;

        // Remap all jumbled text nodes within the content element in-place.
        remap_text_nodes(&content, JUMBLED_CHARS, NORMAL_CHARS);

        Ok(ChapterContent {
            data: content
                .html_opt()
                .ok_or_else(|| eyre!("Failed to extract chapter HTML"))?,
        })
    }
}

/// Recursively walks the children of `element` in pre-order and remaps the text
/// content of every text node using the provided character mapping.
///
/// `from` and `to` must be the same length; each character in `from` is replaced
/// by the corresponding character in `to`. Characters not in `from` are unchanged.
fn remap_text_nodes(element: &Element, from: &str, to: &str) {
    let map: std::collections::HashMap<char, char> = from.chars().zip(to.chars()).collect();
    remap_recursive(element, &map);
}

fn remap_recursive(element: &Element, map: &std::collections::HashMap<char, char>) {
    for child in element.children() {
        match child {
            ChildNode::Text(text_node) => {
                let remapped: String = text_node
                    .text()
                    .chars()
                    .map(|c| map.get(&c).copied().unwrap_or(c))
                    .collect();
                text_node.set_text(remapped);
            }
            ChildNode::Element(child_elem) => {
                remap_recursive(&child_elem, map);
            }
        }
    }
}
