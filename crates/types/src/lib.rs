//! Domain types for the Quelle project.
//!
//! This crate provides plain Rust structs and enums representing the core
//! domain types used across the workspace. It has no dependency on Wasmtime
//! or any other heavyweight crate — only `serde` for serialization support.
pub mod datetime;
pub use datetime::Timestamp;
pub mod version;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Novel {
    pub url: String,
    pub authors: Vec<String>,
    pub title: String,
    pub cover: Option<String>,
    pub description: Vec<String>,
    pub volumes: Vec<Volume>,
    pub metadata: Vec<Metadata>,
    pub status: NovelStatus,
    pub langs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,
    pub index: i32,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub index: i32,
    pub url: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub name: String,
    pub value: String,
    pub ns: Namespace,
    pub others: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Namespace {
    Dc,
    Opf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum NovelStatus {
    Ongoing,
    Hiatus,
    Completed,
    Stub,
    Dropped,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterContent {
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicNovel {
    pub title: String,
    pub cover: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub novels: Vec<BasicNovel>,
    pub total_count: Option<u32>,
    pub current_page: u32,
    pub total_pages: Option<u32>,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}
