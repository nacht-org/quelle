#[allow(unused_imports)]
#[macro_use]
extern crate quelle_glue;

use kuchiki::{traits::TendrilSink, NodeRef};
use once_cell::sync::Lazy;
use quelle_core::prelude::*;
use quelle_glue::prelude::*;
use regex::Regex;
use serde_json::json;

define_meta! {
    let META = {
        id: "en.creativenovels",
        name: "CreativeNovels",
        langs: ["en"],
        base_urls: ["https://creativenovels.com"],
        rds: [Ltr],
        attrs: [],
    };
}

#[cfg(debug_assertions)]
#[expose]
pub fn setup() {
    set_panic_hook();
}

#[expose]
pub fn fetch_novel(url: String) -> Result<Novel, QuelleError> {
    let response = Request::get(url.clone()).send()?;
    let doc = kuchiki::parse_html().one(response.text().unwrap());

    println!("doc created");

    let author = doc
        .select_first(".x-bar-container > [class*='14']")
        .get_text()?;

    println!("got author");

    let cover_element = doc.select_first("img.book_cover").ok();
    let cover = cover_element
        .map(|node| match node.get_attribute("src") {
            Some(value) => Some(value),
            None => node.get_attribute("data-cfsrc"),
        })
        .flatten();

    println!("got cover");

    let novel = Novel {
        title: doc
            .select_first(".x-bar-container > [class*='12']")
            .get_text()?,
        authors: vec![author],
        cover,
        description: doc.select(".novel_page_synopsis > p").collect_text(),
        volumes: collect_volumes(&doc)?,
        metadata: collect_metadata(&doc)?,
        langs: META.langs.clone(),
        status: doc
            .select_first(".novel_status")
            .get_text()?
            .as_str()
            .into(),
        url,
    };

    Ok(novel)
}

fn collect_volumes(doc: &NodeRef) -> Result<Vec<Volume>, QuelleError> {
    let mut volume = Volume::default();

    let novel_id =
        get_novel_id(doc).ok_or_else(|| QuelleError::ParseFailed(ParseError::ElementNotFound))?;
    let security_key = get_security_key(doc);

    let response = Request::post(String::from(
        "https://creativenovels.com/wp-admin/admin-ajax.php",
    ))
    .json_data(&json!({
        "action": "crn_chapter_list",
        "view_id": novel_id,
        "s": security_key,
    }))
    .unwrap()
    .send()?;

    let content = response.text().unwrap();
    if content.starts_with("success") {
        let content = &content["success.define.".len()..];
        for data in content.split(".end_data.") {
            let parts = data.split(".data.").collect::<Vec<_>>();
            if parts.len() < 2 {
                continue;
            }

            let chapter = Chapter {
                index: volume.chapters.len() as i32,
                title: parts[0].to_owned(),
                url: parts[1].to_owned(),
                updated_at: None,
            };

            volume.chapters.push(chapter);
        }
    }

    Ok(vec![volume])
}

fn get_novel_id(doc: &NodeRef) -> Option<usize> {
    let shortlink = doc
        .select_first("link[rel='shortlink']")
        .get_attribute("href");

    shortlink
        .as_ref()
        .map(|link| link.split_once('?'))
        .flatten()
        .map(|(_, query)| query.split_once('='))
        .flatten()
        .map(|(name, value)| if name == "p" { Some(value) } else { None })
        .flatten()
        .map(|value| value.parse().ok())
        .flatten()
}

fn get_security_key(doc: &NodeRef) -> String {
    let mut security_key = String::new();
    let p = Regex::new(r#""([^"]+)""#).unwrap();

    let Ok(scripts) = doc.select("script") else { return security_key; };
    for script in scripts {
        let text = script.get_text();
        if text.is_empty() || !text.contains("var chapter_list_summon") {
            continue;
        }

        let matches = p.find_iter(&text).map(|m| m.as_str()).collect::<Vec<_>>();
        if let ["\"ajaxurl\"", "\"https:\\/\\/creativenovels.com\\/wp-admin\\/admin-ajax.php\"", "\"security\"", key] =
            &matches[..]
        {
            security_key = key[1..(key.len() - 1)].to_string();
        }
    }

    security_key
}

fn collect_metadata(doc: &NodeRef) -> Result<Vec<Metadata>, QuelleError> {
    let mut metadata = vec![];

    let genres = doc.select(".genre_novel > a");
    if let Ok(elements) = genres {
        for element in elements {
            metadata.push(Metadata::new(
                String::from("subject"),
                element.get_text(),
                None,
            ))
        }
    }

    let tags = doc.select(".suggest_tag > a");
    if let Ok(elements) = tags {
        for element in elements {
            metadata.push(Metadata::new(String::from("tag"), element.get_text(), None))
        }
    }

    Ok(metadata)
}

#[expose]
pub fn fetch_chapter_content(url: String) -> Result<String, QuelleError> {
    Ok(String::new())
}