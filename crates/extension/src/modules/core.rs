use crate::novel::{Metadata, Namespace, NovelStatus, Volume};
use crate::source::{SearchCapabilities, SourceCapabilities};

impl Default for SourceCapabilities {
    fn default() -> Self {
        Self {
            search: Default::default(),
        }
    }
}

impl Default for SearchCapabilities {
    fn default() -> Self {
        Self {
            supports_simple_search: false,
            supports_complex_search: false,
            available_filters: Default::default(),
            available_sort_options: Default::default(),
        }
    }
}

impl NovelStatus {
    pub fn from_str(status: &str) -> Self {
        match status.to_ascii_lowercase().as_str() {
            "ongoing" => NovelStatus::Ongoing,
            "completed" => NovelStatus::Completed,
            "hiatus" => NovelStatus::Hiatus,
            "dropped" => NovelStatus::Dropped,
            "stub" => NovelStatus::Stub,
            _ => NovelStatus::Unknown,
        }
    }
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

impl Metadata {
    pub fn new(name: String, value: String, others: Option<Vec<(String, String)>>) -> Self {
        let ns = if DUBLIN_CORE.contains(&name.as_str()) {
            Namespace::Dc
        } else {
            Namespace::Opf
        };

        Metadata {
            name,
            value,
            ns,
            others: others.unwrap_or_default(),
        }
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self {
            name: "_default".to_string(),
            index: -1,
            chapters: vec![],
        }
    }
}
