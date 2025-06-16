use chrono::NaiveDateTime;
use eyre::eyre;
use once_cell::sync::Lazy;
use quelle_extension::{novel::Metadata, prelude::*};
use scraper::{Html, Selector};

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
            cover: select_first_attr_opt(&doc, ".fic_image img", "src")?,
            status: select_first_text(
                &doc,
                ".widget_fic_similar > li:last-child > span:last-child",
            )
            .map(|node| NovelStatus::from_str(&node))
            .unwrap_or(NovelStatus::Unknown),
            volumes: volumes(&self.client, id)?,
            metadata: vec![],
            url,
        };

        Ok(novel)
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        let response = Request::get(&url)
            .send(&self.client)
            .map_err(|e| eyre!(e))?;

        let text = response
            .text()?
            .ok_or_else(|| eyre!("Failed to get data"))?;

        let doc = Html::parse_document(&text);
        let content = select_first(&doc, "#chp_raw")?;

        Ok(ChapterContent {
            data: content.html(),
        })
    }
}

// fn metadata(doc: &Html) -> Result<Vec<Metadata>, eyre::Report> {
//     let mut metadata = vec![];

//     if let Ok(nodes) = select(doc, "a.fic_genre") {
//         for node in nodes {
//             metadata.push(Metadata::new(
//                 String::from("subject"),
//                 node.get_text(),
//                 None,
//             ));
//         }
//     }

//     if let Ok(nodes) = select(doc, "a.stag") {
//         for node in nodes {
//             metadata.push(Metadata::new(String::from("tag"), node.get_text(), None));
//         }
//     }

//     if let Ok(nodes) = select(doc, ".mature_contains > a") {
//         for node in nodes {
//             metadata.push(Metadata::new(
//                 String::from("warning"),
//                 node.get_text(),
//                 None,
//             ));
//         }
//     }

//     let rating_element = select_first(doc, "#ratefic_user > span");
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
