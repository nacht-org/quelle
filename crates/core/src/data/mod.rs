mod chapter;
mod meta;
mod novel;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use chapter::{Chapter, Content, TaggedDateTime};
pub use meta::Meta;
pub use novel::{BasicNovel, Novel};

#[derive(Serialize, Deserialize, Debug)]
pub enum ReadingDirection {
    Ltr,
    Rtl,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Attribute {
    Fanfiction,
}

/// https://www.dublincore.org/specifications/dublin-core/dces/
pub const DUBLIN_CORE: [&str; 16] = [
    // An entity responsible for making contributions to the resource.
    "contributor",
    // The spatial or temporal topic of the resource, the spatial applicability of the resource, or the jurisdiction under which the resource is relevant.
    "coverage",
    // An entity primarily responsible for making the resource.
    "creator",
    // point or period of time associated with an event in the lifecycle of the resource.
    "date",
    // An account of the resource.
    "description",
    // The file format, physical medium, or dimensions of the resource.
    "format",
    // Information about rights held in and over the resource.
    "rights",
    // The topic of the resource.
    "subject",
    // A name given to the resource.
    "title",
    // A related resource from which the described resource is derived.
    "source",
    // Information about rights held in and over the resource.
    "rights",
    // A related resource.
    "relation",
    // An entity responsible for making the resource available.
    "publisher",
    // A language of the resource.
    "language",
    // An unambiguous reference to the resource within a given context.An unambiguous reference to the resource within a given context.
    "identifier",
    // The nature or genre of the resource
    "type",
];

#[derive(Serialize, Deserialize, Debug)]
pub struct Metadata {
    pub name: String,
    pub value: String,
    pub ns: Namespace,
    pub others: HashMap<String, String>,
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

#[derive(Serialize, Deserialize, Default, Debug)]
pub enum NovelStatus {
    Ongoing,
    Hiatus,
    Completed,
    Stub,
    Dropped,
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
            "dropped" => NovelStatus::Dropped,
            _ => NovelStatus::Unknown,
        }
    }
}
