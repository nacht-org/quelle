#[allow(warnings)]
mod bindings;
mod node;

use bindings::{
    exports::quelle::extension::{instance, meta},
    quelle::{
        core::{
            novel::{Chapter, ChapterContent, Novel, NovelStatus, Volume},
            source::{ReadingDirection, SourceMeta},
        },
        http::main::{Client, FormPart, Method, Request, RequestBody},
    },
};
use chrono::NaiveDateTime;
use kuchiki::traits::TendrilSink;
use node::{CollectText, GetAttribute, GetText, OuterHtml};
use once_cell::sync::Lazy;

pub struct Component;

bindings::export!(Component with_types_in bindings);

const INFO: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.scribblehub"),
    name: String::from("ScribbleHub"),
    langs: vec![String::from("en")],
    version: String::from("0.1.0"),
    base_urls: vec![String::from("https://www.scribblehub.com/")],
    rds: vec![ReadingDirection::Ltr],
    attrs: vec![],
});

impl meta::Guest for Component {
    fn extension_info() -> SourceMeta {
        INFO.clone()
    }

    fn setup() -> Result<(), String> {
        Ok(())
    }
}

pub struct ScribbleHub {
    pub client: Client,
}

impl instance::Guest for Component {
    type Source = ScribbleHub;
}

impl instance::GuestSource for ScribbleHub {
    fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn novel_info(&self, url: String) -> Result<Novel, String> {
        let response = self
            .client
            .request(&Request {
                method: Method::Get,
                url: url.clone(),
                params: None,
                data: None,
                headers: None,
            })
            .unwrap();

        let text = response.data.unwrap();
        let text = String::from_utf8(text).unwrap();

        let doc = kuchiki::parse_html().one(text);

        let id = url
            .split("/")
            .nth(4)
            .ok_or_else(|| String::from("The url does not have an id"))?;

        let novel = Novel {
            title: doc.select_first("div.fic_title").get_text().unwrap(),
            authors: vec![doc.select_first("span.auth_name_fic").get_text().unwrap()],
            description: doc.select(".wi_fic_desc > p").collect_text(),
            langs: INFO.langs.clone(),
            cover: doc.select_first(".fic_image img").get_attribute("src"),
            status: doc
                .select_first(".widget_fic_similar > li:last-child > span:last-child")
                .map(|node: kuchiki::NodeDataRef<kuchiki::ElementData>| {
                    str_to_status(node.get_text().as_str())
                })
                .unwrap_or(NovelStatus::Unknown),
            volumes: volumes(&self.client, id)?,
            metadata: vec![],
            url,
        };

        Ok(novel)
    }

    fn chapter_content(&self, url: String) -> Result<ChapterContent, String> {
        let response = self
            .client
            .request(&Request {
                method: Method::Get,
                url,
                params: None,
                data: None,
                headers: None,
            })
            .unwrap();

        let text = response.data.unwrap();
        let text = String::from_utf8(text).unwrap();

        let doc = kuchiki::parse_html().one(text);

        let content = doc
            .select_first("#chp_raw")
            .map(|node| node.as_node().outer_html())
            .ok()
            .transpose()
            .map_err(|e| "Failed to get element".to_string())?
            .ok_or_else(|| String::from("Element not found"))?;

        Ok(ChapterContent { data: content })
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

fn volumes(client: &Client, id: &str) -> Result<Vec<Volume>, String> {
    let body = RequestBody::Form(vec![
        (
            String::from("action"),
            FormPart::Text(String::from("wi_getreleases_pagination")),
        ),
        (String::from("pagenum"), FormPart::Text(String::from("-a"))),
        (String::from("mypostid"), FormPart::Text(id.to_string())),
    ]);

    let response = client
        .request(&Request {
            method: Method::Post,
            url: "https://www.scribblehub.com/wp-admin/admin-ajax.php".to_string(),
            params: None,
            data: Some(body),
            headers: None,
        })
        .unwrap();

    let text = response.data.unwrap();
    let text = String::from_utf8(text).unwrap();

    let doc = kuchiki::parse_html().one(text);
    let mut volume = Volume {
        name: "_default".to_string(),
        index: -1,
        chapters: vec![],
    };

    if let Ok(nodes) = doc.select("li.toc_w") {
        for node in nodes.rev() {
            let Ok(a) = node.as_node().select_first("a") else {
                continue;
            };
            let Some(href) = a.get_attribute("href") else {
                continue;
            };

            let time = node
                .as_node()
                .select_first(".fic_date_pub")
                .get_attribute("title");

            // TODO: parse relative time
            let updated_at = time
                .map(|time| NaiveDateTime::parse_from_str(&time, "").ok())
                .flatten()
                .map(|time| time.to_string());

            let chapter = Chapter {
                index: volume.chapters.len() as i32,
                title: a.get_text(),
                url: href,
                updated_at,
            };

            volume.chapters.push(chapter);
        }
    }

    Ok(vec![volume])
}
