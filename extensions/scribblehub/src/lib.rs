use eyre::eyre;
use once_cell::sync::Lazy;
use quelle_extension::{common::time::parse_date_or_relative_time, prelude::*};

register_extension!(Extension);

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.scribblehub"),
    name: String::from("ScribbleHub"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![String::from("https://www.scribblehub.com")],
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
                .map(|node| NovelStatus::from_str(&node.text_or_empty()))
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
                .param("pagenum", "-a")
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
