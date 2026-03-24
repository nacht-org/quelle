# Writing a Quelle Extension

This guide covers everything you need to know to write a new extension for Quelle, from
project setup to implementing every required and optional method.

Extensions are compiled to **WebAssembly components** and loaded at runtime via the WIT interface.
All extension logic is written in Rust using the `quelle_extension` crate.

---

## Table of Contents

1. [Project Setup](#1-project-setup)
2. [Extension Skeleton](#2-extension-skeleton)
3. [Metadata (`meta`)](#3-metadata-meta)
4. [Fetching Novel Info (`fetch_novel_info`)](#4-fetching-novel-info-fetch_novel_info)
5. [Fetching Chapter Content (`fetch_chapter`)](#5-fetching-chapter-content-fetch_chapter)
6. [Simple Search (`simple_search`)](#6-simple-search-simple_search)
7. [Complex Search (`complex_search`)](#7-complex-search-complex_search)
8. [HTTP Client](#8-http-client)
9. [HTML Scraping](#9-html-scraping)
10. [Content Cleaning](#10-content-cleaning)
11. [Tracing & Logging](#11-tracing--logging)
12. [Utilities](#12-utilities)
13. [Protobuf / gRPC APIs](#13-protobuf--grpc-apis)
14. [Complete Minimal Example](#14-complete-minimal-example)

---

## 1. Project Setup

Create a new directory under `extensions/` and add a `Cargo.toml`:

```extensions/mysite/Cargo.toml
[package]
name = "extension_mysite"
version = "0.1.0"
edition = "2024"

[dependencies]
quelle_extension = { path = "../../crates/extension" }
once_cell = { workspace = true }
eyre = { workspace = true }
tracing = { workspace = true }

[lib]
crate-type = ["cdylib"]
```

Register it in the workspace root:

```Cargo.toml
[workspace]
members = ["crates/*", "extensions/*"]
```

The `crate-type = ["cdylib"]` is mandatory — it tells `cargo` to produce a shared library
that can be compiled to a Wasm component.

---

## 2. Extension Skeleton

Every extension follows the same shape. Create `src/lib.rs`:

```extensions/mysite/src/lib.rs
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

// Register this struct as the extension entry-point.
register_extension!(Extension);

const BASE_URL: &str = "https://www.mysite.com";

// Build metadata once and re-use it across calls.
static META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.mysite"),
    name: String::from("My Site"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![BASE_URL.to_string()],
    rds: vec![ReadingDirection::Ltr],
    attrs: vec![],
    capabilities: SourceCapabilities {
        search: Some(SearchCapabilities {
            supports_simple_search: true,
            supports_complex_search: false,
            ..Default::default()
        }),
    },
});

pub struct Extension {
    client: Client,
}

impl QuelleExtension for Extension {
    fn new() -> Self {
        Self { client: Client::new() }
    }

    fn meta(&self) -> SourceMeta {
        META.clone()
    }

    fn fetch_novel_info(&self, url: String) -> Result<Novel, eyre::Report> {
        todo!()
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        todo!()
    }
}
```

### What `register_extension!` Does

The macro exports a C-ABI function `register-extension` that the host calls once to
instantiate your struct and wire it into the global extension slot. It also installs the
panic hook and the tracing subscriber so logs flow back to the host.

---

## 3. Metadata (`meta`)

`SourceMeta` describes your extension to the host. All fields are required.

```extensions/mysite/src/lib.rs
static META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    // Unique reverse-domain style ID: "<lang>.<sitename>"
    id: String::from("en.mysite"),

    // Human-readable display name.
    name: String::from("My Site"),

    // ISO 639-1 language codes for content on this source.
    langs: vec![String::from("en")],

    // Automatically set from Cargo.toml — keep it that way.
    version: String::from(env!("CARGO_PKG_VERSION")),

    // Every URL variant the source uses (with and without www, http vs https).
    base_urls: vec![
        "https://www.mysite.com".to_string(),
        "https://mysite.com".to_string(),
    ],

    // Reading direction: Ltr (left-to-right) or Rtl (right-to-left).
    rds: vec![ReadingDirection::Ltr],

    // Optional attributes. Use SourceAttr::Fanfiction for fan-fiction sites.
    attrs: vec![],

    // Declare which search modes are available (see Section 6 & 7).
    capabilities: SourceCapabilities {
        search: Some(SearchCapabilities {
            supports_simple_search: true,
            supports_complex_search: false,
            ..Default::default()
        }),
    },
});
```

---

## 4. Fetching Novel Info (`fetch_novel_info`)

This function receives the URL of a novel's landing page and must return a fully-populated
`Novel` struct.

```extensions/mysite/src/lib.rs
fn fetch_novel_info(&self, url: String) -> Result<Novel, eyre::Report> {
    use eyre::{Context, OptionExt, eyre};

    let doc = Request::get(&url)
        .html(&self.client)
        .map_err(|e| eyre!(e))
        .wrap_err("Failed to fetch novel page")?;

    // --- Title ---
    let title = doc.select_first("h1.novel-title")?.text_or_empty();

    // --- Cover ---
    let cover = doc
        .select_first_opt(".cover img")?
        .and_then(|img| img.attr_opt("src"))
        .map(|src| make_absolute_url(&src, BASE_URL));

    // --- Authors ---
    let authors = doc
        .select(".author a")?
        .into_iter()
        .filter_map(|a| a.text_opt())
        .collect::<Vec<_>>();

    // --- Description ---
    let description = doc
        .select(".synopsis p")?
        .into_iter()
        .filter_map(|p| p.text_opt())
        .collect::<Vec<_>>();

    // --- Status ---
    let status = doc
        .select_first_opt(".status-label")?
        .and_then(|el| el.text_opt())
        .and_then(|s| s.parse().ok())       // NovelStatus implements FromStr
        .unwrap_or(NovelStatus::Unknown);

    // --- Metadata (genres, tags, …) ---
    let mut metadata = Vec::new();
    for genre in doc.select(".genre-list a")? {
        metadata.push(Metadata::new("subject", genre.text_or_empty(), None));
    }

    // --- Volumes & Chapters ---
    let volumes = extract_chapters(&self.client, &doc)?;

    Ok(Novel {
        url,
        title,
        authors,
        cover,
        description,
        status,
        metadata,
        volumes,
        langs: META.langs.clone(),
    })
}
```

### Building Volumes & Chapters

Chapters must be placed inside at least one `Volume`. If the site has no real volumes,
use `Volume::default()` (index 0, empty name) and push all chapters into it.

```extensions/mysite/src/lib.rs
fn extract_chapters(client: &Client, doc: &Html) -> eyre::Result<Vec<Volume>> {
    let mut volume = Volume::default();

    for (index, row) in doc.select("ul.chapter-list li")?.into_iter().enumerate() {
        let link = match row.select_first_opt("a[href]")? {
            Some(el) => el,
            None => continue,
        };

        let title = link.text_or_empty();
        let url = link
            .attr_opt("href")
            .map(|href| make_absolute_url(&href, BASE_URL))
            .ok_or_eyre("Missing href on chapter link")?;

        // Optional ISO 8601 / RFC 3339 string, or None.
        let updated_at = row
            .select_first_opt("time[datetime]")?
            .and_then(|t| t.attr_opt("datetime"));

        volume.chapters.push(Chapter {
            index: index as i32,
            title,
            url,
            updated_at,
        });
    }

    Ok(vec![volume])
}
```

### `NovelStatus` Parsing

The following string values are recognised by `NovelStatus`'s `FromStr` implementation
(case-insensitive): `ongoing`, `hiatus`, `completed`, `stub`, `dropped`.
Everything else maps to `NovelStatus::Unknown`.

### `Metadata::new`

```extensions/mysite/src/lib.rs
// Signature: Metadata::new(name, value, namespace_override)
// Namespace defaults to Dublin Core (dc) when None.
Metadata::new("subject", "Fantasy", None);
Metadata::new("tag",     "isekai",  None);
```

---

## 5. Fetching Chapter Content (`fetch_chapter`)

Return the chapter body as an HTML string inside `ChapterContent { data }`. Use
`ContentCleaner` to strip ads, scripts, and other noise before returning.

```extensions/mysite/src/lib.rs
fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
    use eyre::{Context, OptionExt, eyre};

    let doc = Request::get(&url)
        .html(&self.client)
        .map_err(|e| eyre!(e))
        .wrap_err("Failed to fetch chapter page")?;

    // Grab the element that wraps the chapter text.
    let content = doc
        .select_first_opt("#chapter-content")?
        .ok_or_eyre("Chapter content element not found")?;

    // ContentCleaner::new() applies sensible defaults.
    // Chain .remove() / .remove_tag() for site-specific noise.
    let data = ContentCleaner::new()
        .remove(".author-note")          // CSS selector
        .remove_tag("aside")             // tag name
        .clean(&content)?;

    Ok(ChapterContent { data })
}
```

See [Section 10](#10-content-cleaning) for the full `ContentCleaner` API.

---

## 6. Simple Search (`simple_search`)

Simple search takes a plain-text query string and an optional page number.

```extensions/mysite/src/lib.rs
fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
    use eyre::{Context, OptionExt, eyre};

    // query.page() returns 1 if page is None.
    let current_page = query.page();

    let doc = Request::get(format!("{BASE_URL}/search"))
        .param("q", &query.query)
        .param("page", current_page.to_string())
        .html(&self.client)
        .map_err(|e| eyre!(e))
        .wrap_err("Failed to fetch search results")?;

    let mut novels = Vec::new();

    for card in doc.select(".novel-card")? {
        let title = card.select_first(".card-title a")?.text_or_empty();
        let url = card
            .select_first(".card-title a")?
            .attr_opt("href")
            .map(|h| make_absolute_url(&h, BASE_URL))
            .ok_or_eyre("Missing href on search result")?;

        let cover = card
            .select_first_opt("img")?
            .and_then(|img| img.attr_opt("src"))
            .map(|src| make_absolute_url(&src, BASE_URL));

        novels.push(BasicNovel { title, url, cover });
    }

    // Parse total pages from pagination, fall back to current page.
    let total_pages = doc
        .select(".pagination li")?
        .into_iter()
        .filter_map(|li| li.text_opt())
        .filter_map(|s| s.parse::<u32>().ok())
        .max()
        .unwrap_or(current_page);

    Ok(SearchResult {
        novels,
        total_count: None,      // set if the site exposes a total count
        current_page,
        total_pages: Some(total_pages),
        has_next_page: current_page < total_pages,
        has_previous_page: current_page > 1,
    })
}
```

Declare simple search support in your metadata:

```extensions/mysite/src/lib.rs
capabilities: SourceCapabilities {
    search: Some(SearchCapabilities {
        supports_simple_search: true,
        ..Default::default()
    }),
},
```

---

## 7. Complex Search (`complex_search`)

Complex search exposes a rich filter UI to the user. It requires more setup but gives full
control over filter types, sort options, and request building.

### 7.1 Declare Filters and Sort Options

Split the filter/sort definitions into their own file `src/filters.rs` for clarity.

```extensions/mysite/src/filters.rs
use quelle_extension::prelude::*;
use std::str::FromStr;

// Define a typed enum for your filter IDs — avoids magic strings.
pub enum FilterId {
    TitleContains,
    Genre,
    Status,
    MinChapters,
    MaxChapters,
    LastUpdated,
}

impl FilterId {
    pub fn as_str(&self) -> &'static str {
        match self {
            FilterId::TitleContains => "title_contains",
            FilterId::Genre        => "genre",
            FilterId::Status       => "status",
            FilterId::MinChapters  => "min_chapters",
            FilterId::MaxChapters  => "max_chapters",
            FilterId::LastUpdated  => "last_updated",
        }
    }
}

impl AsRef<str> for FilterId {
    fn as_ref(&self) -> &str { self.as_str() }
}

impl From<FilterId> for String {
    fn from(id: FilterId) -> Self { id.as_str().to_string() }
}

impl From<&FilterId> for String {
    fn from(id: &FilterId) -> Self { id.as_str().to_string() }
}

pub fn create_filter_definitions() -> Vec<FilterDefinition> {
    vec![
        // Plain text input
        FilterBuilder::new(FilterId::TitleContains, "Title Contains")
            .description("Only show novels whose title contains this text")
            .text_with_options(Some("Enter title..."), Some(255)),

        // Single-value dropdown
        FilterBuilder::new(FilterId::Status, "Status")
            .description("Filter by completion status")
            .select(vec![
                FilterOption::new("all",       "All"),
                FilterOption::new("ongoing",   "Ongoing"),
                FilterOption::new("completed", "Completed"),
                FilterOption::new("hiatus",    "Hiatus"),
            ]),

        // Tristate: include / exclude / ignore per option
        FilterBuilder::new(FilterId::Genre, "Genres")
            .description("Include or exclude genres")
            .tri_state(vec![
                FilterOption::new("fantasy",  "Fantasy"),
                FilterOption::new("romance",  "Romance"),
                FilterOption::new("sci_fi",   "Sci-Fi"),
                FilterOption::new("horror",   "Horror"),
            ]),

        // Numeric range
        FilterBuilder::new(FilterId::MinChapters, "Min Chapters")
            .number_range(0.0, 10_000.0, Some(1.0), Some("chapters")),
        FilterBuilder::new(FilterId::MaxChapters, "Max Chapters")
            .number_range(0.0, 10_000.0, Some(1.0), Some("chapters")),

        // Date range
        FilterBuilder::new(FilterId::LastUpdated, "Last Updated")
            .description("Filter by last update date")
            .date_range("YYYY-MM-DD", None::<String>, None::<String>),
    ]
}

pub fn create_sort_options() -> Vec<SortOption> {
    vec![
        SortOptionBuilder::new("popularity", "Popularity")
            .description("Sort by total views")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("last_update", "Last Updated")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("chapters", "Chapters")
            .build(),
    ]
}
```

### 7.2 Declare Capabilities

```extensions/mysite/src/lib.rs
use crate::filters;

capabilities: SourceCapabilities {
    search: Some(SearchCapabilities {
        supports_simple_search: true,
        supports_complex_search: true,
        available_filters: filters::create_filter_definitions(),
        available_sort_options: filters::create_sort_options(),
    }),
},
```

### 7.3 Implement `complex_search`

```extensions/mysite/src/lib.rs
use quelle_extension::novel::ComplexSearchQuery;

fn complex_search(&self, query: ComplexSearchQuery) -> Result<SearchResult, eyre::Report> {
    use eyre::{Context, OptionExt, eyre};

    let current_page = query.page.unwrap_or(1);

    let filter_defs  = filters::create_filter_definitions();
    let sort_options = filters::create_sort_options();

    // Validate all applied filters against the declared definitions.
    // Returns ValidatedSearchParams or an error with a clear message.
    let validated = validate_search_query(&filter_defs, &sort_options, &query)?;

    // Build the form / query params for the site's search endpoint.
    let form = validated
        .into_form()
        // Map each FilterId to the site's own parameter name(s).
        .with_mapping(filters::FilterId::TitleContains, "title")
        .with_mapping(filters::FilterId::Status,        "status")
        // Tristate filters need include + exclude param names.
        .with_mapping_tristate(
            filters::FilterId::Genre,
            "genres_include",
            "genres_exclude",
        )
        // Numeric range filters map to two separate params.
        .with_mapping_range(
            filters::FilterId::MinChapters,
            "min_chapters",
            "max_chapters",
        )
        // Date range filters map to two separate params.
        .with_mapping_date_range(
            filters::FilterId::LastUpdated,
            "updated_after",
            "updated_before",
        )
        .with_pagination("page")
        .with_sort("sort", "order")
        .with_default_sort("sort", "popularity")
        .build();

    let doc = Request::post(format!("{BASE_URL}/search"))
        .body(form.build())
        .html(&self.client)
        .map_err(|e| eyre!(e))
        .wrap_err("Failed to perform complex search")?;

    // --- Parse results (same as simple search) ---
    let mut novels = Vec::new();
    for card in doc.select(".novel-card")? {
        let title = card.select_first(".card-title a")?.text_or_empty();
        let url   = card
            .select_first(".card-title a")?
            .attr_opt("href")
            .map(|h| make_absolute_url(&h, BASE_URL))
            .ok_or_eyre("Missing href")?;
        let cover = card
            .select_first_opt("img")?
            .and_then(|img| img.attr_opt("src"))
            .map(|src| make_absolute_url(&src, BASE_URL));
        novels.push(BasicNovel { title, url, cover });
    }

    let total_pages = doc
        .select(".pagination li")?
        .into_iter()
        .filter_map(|li| li.text_opt())
        .filter_map(|s| s.parse::<u32>().ok())
        .max()
        .unwrap_or(current_page);

    Ok(SearchResult {
        novels,
        total_count: None,
        current_page,
        total_pages: Some(total_pages),
        has_next_page: current_page < total_pages,
        has_previous_page: current_page > 1,
    })
}
```

### Filter Types Reference

| Builder method | WIT type | Use case |
|---|---|---|
| `.text()` / `.text_with_options()` | `text` | Free-text input |
| `.select(options)` | `select` | Single-choice dropdown |
| `.multi_select(options)` | `multi-select` | Multi-choice checkbox list |
| `.tri_state(options)` | `tri-state` | Include / Exclude / Ignore per option |
| `.number_range(min, max, step, unit)` | `number-range` | Numeric slider/input |
| `.date_range(format, min, max)` | `date-range` | Date picker |
| `.boolean(default, true_label, false_label)` | `boolean` | Toggle switch |

---

## 8. HTTP Client

All networking goes through the `Client` and `Request` types from the prelude.

### GET Request

```extensions/mysite/src/lib.rs
// Fetch an HTML page directly into a queryable Html document.
let doc = Request::get("https://www.mysite.com/novel/my-novel")
    .html(&self.client)?;

// Fetch raw bytes and handle the response yourself.
let response = Request::get("https://www.mysite.com/api/data")
    .param("page", "1")
    .header("Accept", "application/json")
    .send(&self.client)?
    .error_for_status()?;

let text = response.text()?; // Option<String>
```

### POST Request (Form)

```extensions/mysite/src/lib.rs
let body = RequestFormBuilder::new()
    .param("action", "get_chapters")
    .param("novel_id", "42")
    .build();

let response = Request::post("https://www.mysite.com/ajax")
    .body(body)
    .send(&self.client)?
    .error_for_status()?;
```

### POST Request (JSON / Raw bytes)

```extensions/mysite/src/lib.rs
let json_bytes = serde_json::to_vec(&my_struct)?;

let response = Request::post("https://api.mysite.com/graphql")
    .body(quelle_extension::http::RequestBody::Raw(json_bytes))
    .header("Content-Type", "application/json")
    .send(&self.client)?
    .error_for_status()?;
```

### Query Parameters

```extensions/mysite/src/lib.rs
// One at a time:
Request::get(url).param("key", "value").param("page", "2")

// Or all at once:
Request::get(url).params(vec![
    ("key".to_string(), "value".to_string()),
])
```

### Waiting for a Dynamic Element (Chrome-backed requests)

```extensions/mysite/src/lib.rs
let doc = Request::get(url)
    .wait_for_element("#chapter-body")   // CSS selector
    .wait_timeout(15_000)                // milliseconds
    .html(&self.client)?;
```

---

## 9. HTML Scraping

The `Html` (document) and `Element` (node) types give you a CSS-selector-based API.

### Selecting Elements

```extensions/mysite/src/lib.rs
// Returns all matches as ElementList, error on bad selector.
let items: ElementList = doc.select("ul.chapters li")?;

// Returns the first match, error if none found.
let title: Element = doc.select_first("h1.title")?;

// Returns the first match as Some, None if absent.
let maybe_cover: Option<Element> = doc.select_first_opt("img.cover")?;
```

The same methods are available on `Element` to search within subtrees:

```extensions/mysite/src/lib.rs
for card in doc.select(".novel-card")? {
    let link  = card.select_first("a")?;
    let title = link.text_or_empty();
}
```

### Extracting Text

```extensions/mysite/src/lib.rs
element.text_or_empty()       // -> String  (trimmed, never panics)
element.text_opt()            // -> Option<String>  (None if whitespace-only)

// On Result<Element> via ElementExt:
doc.select_first("h1").text_or_empty()  // -> Result<String>
doc.select_first("h1").text_opt()       // -> Result<Option<String>>

// On Result<ElementList> via ElementListExt:
doc.select(".author a").text_all()?     // -> Vec<String>
```

### Extracting Attributes

```extensions/mysite/src/lib.rs
element.attr_opt("href")    // -> Option<String>
element.attr("href")        // -> Result<String>  (error if absent)
element.has_attr("data-id") // -> bool
element.attr_names()        // -> Vec<String>

// On Result<Element> via ElementExt:
doc.select_first("a").attr("href")?
doc.select_first("a").attr_opt("href")?
```

### HTML Output

```extensions/mysite/src/lib.rs
element.inner_html_opt()  // -> Option<String>  (children only)
element.html_opt()        // -> Option<String>  (outer HTML including own tag)
```

### Walking the Tree Manually

```extensions/mysite/src/lib.rs
for child in element.children() {
    match child {
        ChildNode::Element(el) => { /* recurse */ }
        ChildNode::Text(text_node) => {
            let raw = text_node.text();
            // text_node.set_text("replacement") mutates in place
        }
    }
}
```

### Removing Nodes

```extensions/mysite/src/lib.rs
let unwanted = doc.select(".ad-banner")?;
for el in unwanted {
    el.detach(); // removes from the tree
}
```

---

## 10. Content Cleaning

`ContentCleaner` strips noise from raw chapter HTML and returns clean inner HTML.

### Default Cleaner

`ContentCleaner::new()` removes scripts, styles, iframes, ad containers, social widgets,
cookie banners, inline `display:none` / `visibility:hidden` elements, empty block elements,
and common event-handler/tracking attributes. Use it as the starting point for most sites:

```extensions/mysite/src/lib.rs
let content = doc.select_first("#chapter-text")?;
let html = ContentCleaner::new().clean(&content)?;
Ok(ChapterContent { data: html })
```

### Adding Site-Specific Rules

```extensions/mysite/src/lib.rs
let html = ContentCleaner::new()
    .remove(".author-thoughts")          // remove by CSS selector
    .remove("[data-type='ad']")
    .remove_tag("aside")                 // remove all <aside> elements
    .remove_empty_tag("span")            // remove <span> when whitespace-only
    .strip_attr("data-chapter-id")       // strip a specific attribute
    .clean(&content)?;
```

### Empty Cleaner (Full Manual Control)

```extensions/mysite/src/lib.rs
// Start with nothing; add only what you need.
let html = ContentCleaner::empty()
    .remove_tag("script")
    .remove_tag("style")
    .remove(r#"div[align="left"]"#)
    .keep_attrs(&["src", "href", "alt"])  // allowlist — strip everything else
    .clean(&content)?;
```

### Attribute Strategy

| Method | Behaviour |
|---|---|
| `.strip_attr("name")` | Remove this one attribute from every element (denylist mode) |
| `.keep_attrs(&["a", "b"])` | Keep only these attributes, strip the rest (allowlist mode) |

`ContentCleaner::new()` starts in denylist mode with a built-in set of tracking/style
attributes already included. Calling `.keep_attrs()` switches permanently to allowlist mode.

---

## 11. Tracing & Logging

Use the standard `tracing` crate. All log events are forwarded to the host automatically
by the subscriber installed via `register_extension!`.

```extensions/mysite/src/lib.rs
tracing::debug!("Parsed {} chapters", count);
tracing::info!("Using endpoint: {}", url);
tracing::warn!("Author element missing; defaulting to empty");
tracing::error!("Unexpected status: {}", response.status);
```

Structured fields work too:

```extensions/mysite/src/lib.rs
tracing::info!(novel_id = %id, page = current_page, "Fetching chapters");
```

---

## 12. Utilities

### `make_absolute_url`

Converts relative paths to absolute URLs using a base:

```extensions/mysite/src/lib.rs
make_absolute_url("/novel/my-book", "https://www.mysite.com")
// => "https://www.mysite.com/novel/my-book"

make_absolute_url("//cdn.mysite.com/cover.jpg", "https://www.mysite.com")
// => "https://cdn.mysite.com/cover.jpg"

make_absolute_url("https://other.com/thing", "https://www.mysite.com")
// => "https://other.com/thing"  (already absolute — returned unchanged)
```

### `SimpleSearchQuery::page()`

Returns `query.page.unwrap_or(1)` — use this instead of unwrapping `page` directly to
always get a valid 1-based page number.

```extensions/mysite/src/lib.rs
let current_page = query.page(); // u32, always >= 1
```

### Date Parsing (`common::time`)

```extensions/mysite/src/lib.rs
use quelle_extension::common::time::{
    parse_date_or_relative_time,
    parse_date_time_or_relative_time,
    parse_relative_time,
};

// Parse "Mar 14, 2024" or "2 days ago"
let dt = parse_date_or_relative_time("Mar 14, 2024", "%b %d, %Y")?;
let rfc = dt.and_utc().to_rfc3339();

// Parse "2024-03-14 18:30:00" or "yesterday"
let dt = parse_date_time_or_relative_time("2024-03-14 18:30:00", "%Y-%m-%d %H:%M:%S")?;

// Parse only relative expressions: "3 hours ago", "last Monday"
let dt = parse_relative_time("3 hours ago")?;
```

---

## 13. Protobuf / gRPC APIs

For sites with a gRPC or Protobuf JSON API, enable the `protobuf` feature and use a
`build.rs` to compile `.proto` files.

### `Cargo.toml`

```extensions/mysite/Cargo.toml
[dependencies]
quelle_extension = { path = "../../crates/extension", features = ["protobuf"] }
prost       = { workspace = true }
prost-types = { workspace = true }

[build-dependencies]
prost-build = { workspace = true }
```

### `build.rs`

```extensions/mysite/build.rs
fn main() -> std::io::Result<()> {
    prost_build::compile_protos(&["src/mysite.proto"], &["src/"])?;
    Ok(())
}
```

### Using gRPC in `lib.rs`

```extensions/mysite/src/lib.rs
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/mysite.api.rs"));
}

// Send a gRPC request (automatically adds Content-Type and X-Grpc-Web headers):
let response = Request::post("https://api.mysite.com/MyService/GetNovel")
    .grpc(&proto::GetNovelRequest { slug: slug.to_string() })?
    .send(&self.client)?;

// Decode the response:
let novel_data = response.grpc::<proto::GetNovelResponse>()?;

// For plain Protobuf (non-gRPC):
let response = Request::post(url)
    .protobuf(&my_proto_message)?
    .send(&self.client)?;

let decoded = response.protobuf::<proto::MyResponse>()?;
```

---

## 14. Complete Minimal Example

Below is a self-contained extension for a fictional site with no search support:

```extensions/mysite/src/lib.rs
use eyre::{Context, OptionExt, eyre};
use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

register_extension!(Extension);

const BASE_URL: &str = "https://www.mysite.com";

static META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.mysite"),
    name: String::from("My Site"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![BASE_URL.to_string()],
    rds: vec![ReadingDirection::Ltr],
    attrs: vec![],
    capabilities: SourceCapabilities { search: None },
});

pub struct Extension {
    client: Client,
}

impl QuelleExtension for Extension {
    fn new() -> Self {
        Self { client: Client::new() }
    }

    fn meta(&self) -> SourceMeta {
        META.clone()
    }

    fn fetch_novel_info(&self, url: String) -> Result<Novel, eyre::Report> {
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch novel page")?;

        let title   = doc.select_first("h1.title")?.text_or_empty();
        let cover   = doc.select_first_opt(".cover img")?
            .and_then(|img| img.attr_opt("src"))
            .map(|src| make_absolute_url(&src, BASE_URL));
        let authors = doc.select(".author a")?.into_iter()
            .filter_map(|a| a.text_opt()).collect();
        let description = doc.select(".synopsis p")?.into_iter()
            .filter_map(|p| p.text_opt()).collect();

        let mut volume = Volume::default();
        for (index, li) in doc.select("ul.chapters li")?.into_iter().enumerate() {
            let link = match li.select_first_opt("a[href]")? {
                Some(el) => el,
                None => continue,
            };
            volume.chapters.push(Chapter {
                index: index as i32,
                title: link.text_or_empty(),
                url: link.attr_opt("href")
                    .map(|h| make_absolute_url(&h, BASE_URL))
                    .ok_or_eyre("Missing chapter href")?,
                updated_at: None,
            });
        }

        Ok(Novel {
            url,
            title,
            authors,
            cover,
            description,
            status: NovelStatus::Unknown,
            volumes: vec![volume],
            metadata: vec![],
            langs: META.langs.clone(),
        })
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch chapter page")?;

        let content = doc
            .select_first_opt("#chapter-body")?
            .ok_or_eyre("Chapter body not found")?;

        Ok(ChapterContent {
            data: ContentCleaner::new().clean(&content)?,
        })
    }
}
```

---

## Quick-Reference Checklist

- [ ] `crate-type = ["cdylib"]` in `Cargo.toml`
- [ ] `register_extension!(YourStruct)` at crate root
- [ ] Unique `id` in `SourceMeta` using `"<lang>.<sitename>"` format
- [ ] `version = env!("CARGO_PKG_VERSION")`
- [ ] All `base_urls` variants listed (www/non-www, http/https)
- [ ] `fetch_novel_info` returns a fully-populated `Novel`
- [ ] Chapters have sequential `index` values starting from `0`
- [ ] `fetch_chapter` returns cleaned HTML via `ContentCleaner`
- [ ] Search capabilities in `meta` match which methods are implemented
- [ ] Relative URLs resolved with `make_absolute_url`
- [ ] Errors wrapped with `.wrap_err("…")` for context