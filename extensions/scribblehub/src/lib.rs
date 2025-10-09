use eyre::{OptionExt, eyre};
use once_cell::sync::Lazy;
use quelle_extension::novel::{ComplexSearchQuery, FilterValue, TriState};
use quelle_extension::source::SortOrder;
use quelle_extension::{common::time::parse_date_or_relative_time, prelude::*};

mod filters;

register_extension!(Extension);

const BASE_URL: &str = "https://www.scribblehub.com";

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("en.scribblehub"),
    name: String::from("ScribbleHub"),
    langs: vec![String::from("en")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![BASE_URL.to_string(), "https://scribblehub.com".to_string()],
    rds: vec![ReadingDirection::Ltr],
    attrs: vec![],
    capabilities: SourceCapabilities {
        search: Some(SearchCapabilities {
            supports_simple_search: true,
            supports_complex_search: true,
            available_filters: filters::create_filter_definitions(),
            available_sort_options: filters::create_sort_options(),
        }),
    },
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
                .map(|node| {
                    let text = node.text_or_empty();
                    text.split_once("-")
                        .map(|(status, _)| status.trim().to_string())
                        .unwrap_or(text)
                })
                .map(|text| NovelStatus::from_str(&text))
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

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        let current_page = query.page();

        let response = Request::get("https://www.scribblehub.com/series-finder")
            .param("sf", "1")
            .param("sh", query.query)
            .param("order", "desc")
            .param("pg", current_page.to_string())
            .send(&self.client)
            .map_err(|e| eyre!(e))?
            .error_for_status()?;

        let text = response
            .text()?
            .ok_or_else(|| eyre!("Failed to get search data"))?;

        let doc = Html::new(&text);

        let mut novels = Vec::new();

        for element in doc.select(".search_main_box")? {
            let title = element
                .select_first(".search_title > a")?
                .text_opt()
                .ok_or_eyre("Failed to get title")?;

            let url = element
                .select_first(".search_title > a")?
                .attr_opt("href")
                .map(|href| make_absolute_url(&href, BASE_URL))
                .ok_or_eyre("Failed to get url")?;

            let cover = element
                .select_first(".search_img > img")?
                .attr_opt("src")
                .map(|src| make_absolute_url(&src, BASE_URL));

            novels.push(BasicNovel { title, cover, url });
        }

        let total_pages = doc
            .select(".simple-pagination > li")?
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

    fn complex_search(&self, query: ComplexSearchQuery) -> Result<SearchResult, eyre::Report> {
        let current_page = query.page.unwrap_or(1);

        let mut request =
            Request::post("https://www.scribblehub.com/series-finder/").param("sf", "1");

        let mut form_builder = RequestFormBuilder::new();

        // Process filters
        for filter in &query.filters {
            match filter.filter_id.as_str() {
                "title_contains" => {
                    if let FilterValue::Text(value) = &filter.value {
                        form_builder = form_builder.param("seriescontains", value.as_str());
                    }
                }
                "chapters" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_chapters", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_chapters", max.to_string());
                        }
                    }
                }
                "releases_perweek" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder =
                                form_builder.param("min_releases_perweek", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder =
                                form_builder.param("max_releases_perweek", max.to_string());
                        }
                    }
                }
                "favorites" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_favorites", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_favorites", max.to_string());
                        }
                    }
                }
                "ratings" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_ratings", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_ratings", max.to_string());
                        }
                    }
                }
                "num_ratings" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_num_ratings", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_num_ratings", max.to_string());
                        }
                    }
                }
                "readers" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_readers", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_readers", max.to_string());
                        }
                    }
                }
                "reviews" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_reviews", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_reviews", max.to_string());
                        }
                    }
                }
                "pages" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_pages", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_pages", max.to_string());
                        }
                    }
                }
                "pageviews" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_pageviews", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_pageviews", max.to_string());
                        }
                    }
                }
                "totalwords" => {
                    if let FilterValue::NumberRange(range) = &filter.value {
                        if let Some(min) = range.min {
                            form_builder = form_builder.param("min_totalwords", min.to_string());
                        }
                        if let Some(max) = range.max {
                            form_builder = form_builder.param("max_totalwords", max.to_string());
                        }
                    }
                }
                "genre_mode" => {
                    if let FilterValue::Select(value) = &filter.value {
                        form_builder = form_builder.param("gi_mm", value.as_str());
                    }
                }
                "genres" => {
                    if let FilterValue::TriState(tristate_values) = &filter.value {
                        for (option_id, state) in tristate_values {
                            match state {
                                TriState::MustInclude => {
                                    form_builder =
                                        form_builder.param("genreselected[]", option_id.as_str());
                                }
                                TriState::MustExclude => {
                                    form_builder =
                                        form_builder.param("genreexcluded[]", option_id.as_str());
                                }
                                TriState::DontCare => {
                                    // Don't add to form - ignore this option
                                }
                            }
                        }
                    }
                }
                "tags" => {
                    if let FilterValue::TriState(tristate_values) = &filter.value {
                        for (option_id, state) in tristate_values {
                            match state {
                                TriState::MustInclude => {
                                    form_builder = form_builder
                                        .param("tagsalledit_include[]", option_id.as_str());
                                }
                                TriState::MustExclude => {
                                    form_builder = form_builder
                                        .param("tagsalledit_exclude[]", option_id.as_str());
                                }
                                TriState::DontCare => {
                                    // Don't add to form - ignore this option
                                }
                            }
                        }
                    }
                }
                "content_warnings" => {
                    if let FilterValue::TriState(tristate_values) = &filter.value {
                        for (option_id, state) in tristate_values {
                            match state {
                                TriState::MustInclude => {
                                    form_builder =
                                        form_builder.param("ctselected[]", option_id.as_str());
                                }
                                TriState::MustExclude => {
                                    form_builder =
                                        form_builder.param("ctexcluded[]", option_id.as_str());
                                }
                                TriState::DontCare => {
                                    // Don't add to form - ignore this option
                                }
                            }
                        }
                    }
                }
                "story_status" => {
                    if let FilterValue::Select(value) = &filter.value {
                        form_builder = form_builder.param("storystatus", value.as_str());
                    }
                }
                "fandom" => {
                    if let FilterValue::Text(value) = &filter.value {
                        form_builder = form_builder.param("fandomsearch", value.as_str());
                    }
                }
                "last_update" => {
                    if let FilterValue::DateRange(range) = &filter.value {
                        if let Some(start) = &range.start {
                            form_builder = form_builder.param("dp_release_min", start.as_str());
                        }
                        if let Some(end) = &range.end {
                            form_builder = form_builder.param("dp_release_max", end.as_str());
                        }
                    }
                }
                _ => {}
            }
        }

        // Handle sorting
        if let Some(sort_by) = &query.sort_by {
            form_builder = form_builder.param("sortby", sort_by.as_str());
        } else {
            form_builder = form_builder.param("sortby", "pageviews");
        }

        let sort_order = query.sort_order.unwrap_or(SortOrder::Desc);
        let order_str = match sort_order {
            SortOrder::Asc => "asc",
            SortOrder::Desc => "desc",
        };
        form_builder = form_builder.param("order", order_str);

        // Add pagination
        form_builder = form_builder.param("pg", current_page.to_string());

        request = request.body(form_builder.build());

        let response = request
            .send(&self.client)
            .map_err(|e| eyre!(e))?
            .error_for_status()?;

        let text = response
            .text()?
            .ok_or_else(|| eyre!("Failed to get search data"))?;

        let doc = Html::new(&text);

        let mut novels = Vec::new();

        for element in doc.select(".search_main_box")? {
            let title = element
                .select_first(".search_title > a")?
                .text_opt()
                .ok_or_eyre("Failed to get title")?;

            let url = element
                .select_first(".search_title > a")?
                .attr_opt("href")
                .map(|href| make_absolute_url(&href, BASE_URL))
                .ok_or_eyre("Failed to get url")?;

            let cover = element
                .select_first(".search_img > img")?
                .attr_opt("src")
                .map(|src| make_absolute_url(&src, BASE_URL));

            novels.push(BasicNovel { title, cover, url });
        }

        let total_pages = doc
            .select(".simple-pagination > li")?
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
                .param("pagenum", "-1")
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

    let elements = doc
        .select("li.toc_w")?
        .into_iter()
        .collect::<Vec<_>>()
        .into_iter()
        .rev();

    for element in elements {
        let Some(a) = element.select_first_opt("a")? else {
            continue;
        };

        let Some(href) = a.attr_opt("href") else {
            continue;
        };

        let time = element
            .select_first_opt(".fic_date_pub")?
            .and_then(|e| e.attr_opt("title"));

        let updated_at = time
            .and_then(|time| parse_date_or_relative_time(&time, "%b %d, %Y").ok())
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
