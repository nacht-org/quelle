use chrono::NaiveDateTime;
use eyre::eyre;
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;
use scraper::{ElementRef, Html, Selector};

register_extension!(Extension);

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.scribblehub"),
    name: String::from("ScribbleHub"),
    langs: vec![String::from("en")],
    version: String::from("0.1.0"),
    base_urls: vec![String::from("https://www.scribblehub.com/")],
    rds: vec![ReadingDirection::Ltr],
    attrs: vec![],
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

    fn init(&self) -> Result<(), eyre::Report> {
        Ok(())
    }

    fn fetch_novel_info(&self, url: String) -> Result<Novel, eyre::Report> {
        let response = Request::get(&url)
            .send(&self.client)
            .map_err(|e| eyre!(e))?;

        let text = response
            .text()?
            .ok_or_else(|| eyre!("Failed to get data"))?;

        let doc = Html::parse_document(&text);

        let id = url
            .split("/")
            .nth(4)
            .ok_or_else(|| eyre!("The url does not have an id"))?;

        let novel = Novel {
            title: select_first_text(&doc, "div.fic_title")?,
            authors: select_text(&doc, "span.auth_name_fic")?,
            description: select_text(&doc, ".wi_fic_desc > p")?,
            langs: META.langs.clone(),
            cover: select_first(&doc, ".fic_image img")?
                .attr("src")
                .map(|e| e.to_string()),
            status: select_first_text(
                &doc,
                ".widget_fic_similar > li:last-child > span:last-child",
            )
            .map(|node| str_to_status(&node))
            .unwrap_or(NovelStatus::Unknown),
            volumes: volumes(&self.client, id)?,
            metadata: vec![],
            url,
        };

        Ok(novel)
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        let response = self
            .client
            .request(&Request {
                method: Method::Get,
                url,
                params: None,
                data: None,
                headers: None,
            })
            .map_err(|e| eyre!(e))?;

        let text = response.data.ok_or_else(|| eyre!("Failed to get data"))?;
        let text = String::from_utf8(text).map_err(|e| eyre!(e))?;

        let doc = Html::parse_document(&text);
        let content = select_first(&doc, "#chp_raw")?;

        Ok(ChapterContent {
            data: content.html(),
        })
    }
}

fn str_to_status(value: &str) -> NovelStatus {
    match value.to_ascii_lowercase().as_str() {
        "ongoing" => NovelStatus::Ongoing,
        "completed" => NovelStatus::Completed,
        "hiatus" => NovelStatus::Hiatus,
        "dropped" => NovelStatus::Dropped,
        "stub" => NovelStatus::Stub,
        _ => NovelStatus::Unknown,
    }
}

fn select_first<'a>(doc: &'a Html, selector_str: &str) -> Result<ElementRef<'a>, eyre::Report> {
    let selector =
        Selector::parse(selector_str).map_err(|e| eyre!("Failed to compile selector: {e}"))?;

    doc.select(&selector)
        .next()
        .ok_or_else(|| eyre!("Element not found: {selector_str}"))
}

fn select_first_text(doc: &Html, selector_str: &str) -> Result<String, eyre::Report> {
    Ok(select_first(doc, selector_str)?.text().collect::<String>())
}

fn select<'a>(doc: &'a Html, selector_str: &str) -> Result<Vec<ElementRef<'a>>, eyre::Report> {
    let selector =
        Selector::parse(selector_str).map_err(|e| eyre!("Failed to compile selector: {e}"))?;
    Ok(doc.select(&selector).collect())
}

fn select_text(doc: &Html, selector_str: &str) -> Result<Vec<String>, eyre::Report> {
    Ok(select(doc, selector_str)?
        .iter()
        .map(|node| node.text().collect::<String>())
        .collect())
}

// fn metadata(doc: &NodeRef) -> Result<Vec<Metadata>, QuelleError> {
//     let mut metadata = vec![];

//     if let Ok(nodes) = doc.select("a.fic_genre") {
//         for node in nodes {
//             metadata.push(Metadata::new(
//                 String::from("subject"),
//                 node.get_text(),
//                 None,
//             ));
//         }
//     }

//     if let Ok(nodes) = doc.select("a.stag") {
//         for node in nodes {
//             metadata.push(Metadata::new(String::from("tag"), node.get_text(), None));
//         }
//     }

//     if let Ok(nodes) = doc.select(".mature_contains > a") {
//         for node in nodes {
//             metadata.push(Metadata::new(
//                 String::from("warning"),
//                 node.get_text(),
//                 None,
//             ));
//         }
//     }

//     let rating_element = doc.select_first("#ratefic_user > span");
//     if let Some(element) = rating_element.ok() {
//         metadata.push(Metadata::new(
//             String::from("rating"),
//             element.get_text(),
//             None,
//         ));
//     }

//     Ok(metadata)
// }

fn volumes(client: &Client, id: &str) -> Result<Vec<Volume>, eyre::Report> {
    let response = Request::post("https://www.scribblehub.com/wp-admin/admin-ajax.php")
        .body(
            RequestFormBuilder::new()
                .param("action", "wi_getreleases_pagination")
                .param("pagenum", "-a")
                .param("mypostid", id)
                .build(),
        )
        .send(client)
        .map_err(|e| eyre!(e))?;

    let text = response
        .text()?
        .ok_or_else(|| eyre!("Failed to get data"))?;

    let doc = Html::parse_document(&text);
    let mut volume = Volume {
        name: "_default".to_string(),
        index: -1,
        chapters: vec![],
    };

    if let Ok(elements) = select(&doc, "li.toc_w") {
        for element in elements.into_iter().rev() {
            let Some(a) = element.select(&Selector::parse("a").unwrap()).next() else {
                continue;
            };

            let Some(href) = a.attr("href") else {
                continue;
            };

            let time = element
                .select(&Selector::parse(".fic_date_pub").unwrap())
                .next()
                .map(|e| e.attr("title"))
                .flatten();

            // TODO: parse relative time
            let updated_at = time
                .map(|time| NaiveDateTime::parse_from_str(&time, "").ok())
                .flatten()
                .map(|time| time.to_string());

            let chapter = Chapter {
                index: volume.chapters.len() as i32,
                title: a.text().collect(),
                url: href.to_string(),
                updated_at,
            };

            volume.chapters.push(chapter);
        }
    }

    Ok(vec![volume])
}
