use eyre::{OptionExt, WrapErr, eyre};
use once_cell::sync::Lazy;
use quelle_extension::novel::ComplexSearchQuery;
use quelle_extension::{common::time::parse_date_or_relative_time, prelude::*};

pub mod filters;

use filters::FilterId;

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
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch novel info")?;

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
        let doc = Request::get(&url)
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to fetch chapter")?;

        Ok(ChapterContent {
            data: doc.select_first("#chp_raw").html()?,
        })
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        let current_page = query.page();

        let doc = Request::get("https://www.scribblehub.com/series-finder")
            .param("sf", "1")
            .param("sh", query.query)
            .param("order", "desc")
            .param("pg", current_page.to_string())
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to perform simple search")?;

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

        // Get filter definitions and sort options
        let filter_definitions = filters::create_filter_definitions();
        let sort_options = filters::create_sort_options();

        // Validate the entire search query (filters, pagination, sorting)
        // This returns validated data back or an error
        let validated_query = validate_search_query(&filter_definitions, &sort_options, &query)?;

        let mut request =
            Request::post("https://www.scribblehub.com/series-finder/").param("sf", "1");

        let form_builder = validated_query
            .into_form()
            .with_mapping(FilterId::TitleContains, "seriescontains")
            .with_mapping(FilterId::Fandom, "fandomsearch")
            .with_mapping(FilterId::StoryStatus, "storystatus")
            .with_mapping(FilterId::GenreMode, "gi_mm")
            .with_mapping_range(FilterId::Chapters, "min_chapters", "max_chapters")
            .with_mapping_range(
                FilterId::ReleasesPerweek,
                "min_releases_perweek",
                "max_releases_perweek",
            )
            .with_mapping_range(FilterId::Favorites, "min_favorites", "max_favorites")
            .with_mapping_range(FilterId::Ratings, "min_ratings", "max_ratings")
            .with_mapping_range(FilterId::NumRatings, "min_num_ratings", "max_num_ratings")
            .with_mapping_range(FilterId::Readers, "min_readers", "max_readers")
            .with_mapping_range(FilterId::Reviews, "min_reviews", "max_reviews")
            .with_mapping_range(FilterId::Pages, "min_pages", "max_pages")
            .with_mapping_range(FilterId::Pageviews, "min_pageviews", "max_pageviews")
            .with_mapping_range(FilterId::TotalWords, "min_totalwords", "max_totalwords")
            .with_mapping_date_range(FilterId::LastUpdate, "dp_release_min", "dp_release_max")
            .with_mapping_tristate(FilterId::Genres, "genreselected", "genreexcluded")
            .with_mapping_tristate(FilterId::Tags, "tagsalledit_include", "tagsalledit_exclude")
            .with_mapping_tristate(FilterId::ContentWarnings, "ctselected", "ctexcluded")
            .with_pagination("pg")
            .with_sort("sortby", "order")
            .with_default_sort("sortby", "pageviews")
            .with_custom_field("sf", "1");

        request = request.body(form_builder.build().build());

        let doc = request
            .html(&self.client)
            .map_err(|e| eyre!(e))
            .wrap_err("Failed to perform complex search")?;

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
