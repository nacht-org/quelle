use std::collections::HashMap;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::ParseError;

#[derive(Serialize, Deserialize, Debug)]
pub struct Meta {
    pub id: String,
    pub name: String,
    pub lang: String,
    pub version: String,
    pub base_urls: Vec<String>,
    pub rds: Vec<ReadingDirection>,
    pub attrs: Vec<Attribute>,
}

impl Meta {
    pub fn derive_abs_url(
        &self,
        mut url: String,
        current: Option<&str>,
    ) -> Result<String, ParseError> {
        if url.starts_with("https://") || url.starts_with("http://") {
            return Ok(url);
        } else if url.starts_with("//") {
            let base_url = Url::parse(current.unwrap_or(&self.base_urls[0]))
                .map_err(|_| ParseError::FailedURLParse)?;
            url.insert_str(0, base_url.scheme());
        } else if url.starts_with("/") {
            let base_url = &self.base_urls[0];
            let base_url = base_url.strip_suffix("/").unwrap_or(base_url);
            url.insert_str(0, base_url);
        } else if let Some(current) = current {
            let base_url = current.strip_suffix("/").unwrap_or(current);
            url.insert_str(0, base_url);
        }

        Ok(url)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ReadingDirection {
    Ltr,
    Rtl,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Attribute {
    Fanfiction,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Novel {
    pub title: String,
    pub authors: Vec<String>,
    pub url: String,
    pub thumb: Option<String>,
    pub desc: Vec<String>,
    pub volumes: Vec<Volume>,
    pub metadata: Vec<Metadata>,
    pub status: NovelStatus,
    pub lang: String,
}

const DUBLIN_CORE: [&'static str; 10] = [
    "title",
    "language",
    "subject",
    "creator",
    "contributor",
    "publisher",
    "rights",
    "coverage",
    "date",
    "description",
];

#[derive(Serialize, Deserialize, Debug)]
pub struct Metadata {
    name: String,
    value: String,
    ns: Namespace,
    others: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Namespace {
    DC,
    OPF,
}

impl Metadata {
    pub fn new(name: String, value: String, others: Option<HashMap<String, String>>) -> Self {
        let ns = if DUBLIN_CORE.contains(&name.as_str()) {
            Namespace::DC
        } else {
            Namespace::OPF
        };

        Metadata {
            name,
            value,
            ns,
            others: others.unwrap_or_default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Volume {
    pub index: i32,
    pub name: String,
    pub chapters: Vec<Chapter>,
}

impl Default for Volume {
    fn default() -> Self {
        Self {
            index: -1,
            name: String::from("_default"),
            chapters: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Chapter {
    pub index: i32,
    pub title: String,
    pub url: String,
    pub updated_at: Option<TaggedDateTime>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TaggedDateTime {
    Utc(DateTime<Utc>),
    Local(NaiveDateTime),
}

impl From<DateTime<Utc>> for TaggedDateTime {
    #[inline]
    fn from(value: DateTime<Utc>) -> Self {
        Self::Utc(value)
    }
}

impl From<NaiveDateTime> for TaggedDateTime {
    #[inline]
    fn from(value: NaiveDateTime) -> Self {
        Self::Local(value)
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub enum NovelStatus {
    Ongoing,
    Hiatus,
    Completed,
    Stub,
    #[default]
    Unknown,
}

impl From<&str> for NovelStatus {
    fn from(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "ongoing" => NovelStatus::Ongoing,
            "hiatus" => NovelStatus::Hiatus,
            "completed" => NovelStatus::Completed,
            "stub" => NovelStatus::Stub,
            _ => NovelStatus::Unknown,
        }
    }
}
