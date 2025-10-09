use quelle_extension::source::{
    DateRangeFilter, FilterDefinition, FilterOption, FilterType, MultiSelectFilter,
    NumberRangeFilter, SelectFilter, SortOption, SortOrder, TextFilter,
};

pub fn create_filter_definitions() -> Vec<FilterDefinition> {
    vec![
        FilterDefinition {
            id: "title_contains".to_string(),
            name: "Title Contains".to_string(),
            description: Some("Search for series with titles containing this text".to_string()),
            filter_type: FilterType::Text(TextFilter {
                placeholder: Some("Title contains...".to_string()),
                max_length: Some(255),
            }),
            required: false,
        },
        FilterDefinition {
            id: "chapters".to_string(),
            name: "Chapters".to_string(),
            description: Some("Filter by number of chapters".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 10000.0,
                step: Some(1.0),
                unit: Some("chapters".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "releases_perweek".to_string(),
            name: "Chapters per Week".to_string(),
            description: Some("Filter by release frequency".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 50.0,
                step: Some(0.1),
                unit: Some("per week".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "favorites".to_string(),
            name: "Favorites".to_string(),
            description: Some("Filter by number of favorites".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 100000.0,
                step: Some(1.0),
                unit: Some("favorites".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "ratings".to_string(),
            name: "Ratings".to_string(),
            description: Some("Filter by rating score".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 5.0,
                step: Some(0.1),
                unit: Some("stars".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "num_ratings".to_string(),
            name: "Number of Ratings".to_string(),
            description: Some("Filter by number of ratings".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 100000.0,
                step: Some(1.0),
                unit: Some("ratings".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "readers".to_string(),
            name: "Readers".to_string(),
            description: Some("Filter by number of readers".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 1000000.0,
                step: Some(1.0),
                unit: Some("readers".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "reviews".to_string(),
            name: "Reviews".to_string(),
            description: Some("Filter by number of reviews".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 10000.0,
                step: Some(1.0),
                unit: Some("reviews".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "pages".to_string(),
            name: "Pages".to_string(),
            description: Some("Filter by number of pages".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 100000.0,
                step: Some(1.0),
                unit: Some("pages".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "pageviews".to_string(),
            name: "Pageviews (Thousands)".to_string(),
            description: Some("Filter by pageviews in thousands".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 100000.0,
                step: Some(1.0),
                unit: Some("thousand views".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "totalwords".to_string(),
            name: "Total Words (Thousands)".to_string(),
            description: Some("Filter by total word count in thousands".to_string()),
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min: 0.0,
                max: 10000.0,
                step: Some(1.0),
                unit: Some("thousand words".to_string()),
            }),
            required: false,
        },
        FilterDefinition {
            id: "last_update".to_string(),
            name: "Last Update".to_string(),
            description: Some("Filter by last update date".to_string()),
            filter_type: FilterType::DateRange(DateRangeFilter {
                min_date: None,
                max_date: None,
                format: "%Y-%m-%d".to_string(),
            }),
            required: false,
        },
        FilterDefinition {
            id: "genre_mode".to_string(),
            name: "Genre Matching".to_string(),
            description: Some("How to match selected genres".to_string()),
            filter_type: FilterType::Select(SelectFilter {
                options: vec![
                    FilterOption {
                        value: "and".to_string(),
                        label: "AND (must have all selected genres)".to_string(),
                        description: None,
                    },
                    FilterOption {
                        value: "or".to_string(),
                        label: "OR (must have any selected genre)".to_string(),
                        description: None,
                    },
                ],
            }),
            required: false,
        },
        FilterDefinition {
            id: "genres".to_string(),
            name: "Genres".to_string(),
            description: Some("Select genres to include or exclude".to_string()),
            filter_type: FilterType::MultiSelect(MultiSelectFilter {
                options: create_genre_options(),
                max_selections: None,
            }),
            required: false,
        },
        FilterDefinition {
            id: "tags_mode".to_string(),
            name: "Tag Matching".to_string(),
            description: Some("How to match selected tags".to_string()),
            filter_type: FilterType::Select(SelectFilter {
                options: vec![
                    FilterOption {
                        value: "and".to_string(),
                        label: "AND (must have all selected tags)".to_string(),
                        description: None,
                    },
                    FilterOption {
                        value: "or".to_string(),
                        label: "OR (must have any selected tag)".to_string(),
                        description: None,
                    },
                ],
            }),
            required: false,
        },
        FilterDefinition {
            id: "tags_include".to_string(),
            name: "Include Tags".to_string(),
            description: Some("Tags that must be present".to_string()),
            filter_type: FilterType::MultiSelect(MultiSelectFilter {
                options: create_tag_options(),
                max_selections: None,
            }),
            required: false,
        },
        FilterDefinition {
            id: "tags_exclude".to_string(),
            name: "Exclude Tags".to_string(),
            description: Some("Tags that must not be present".to_string()),
            filter_type: FilterType::MultiSelect(MultiSelectFilter {
                options: create_tag_options(),
                max_selections: None,
            }),
            required: false,
        },
        FilterDefinition {
            id: "content_warning_mode".to_string(),
            name: "Content Warning Matching".to_string(),
            description: Some("How to match content warnings".to_string()),
            filter_type: FilterType::Select(SelectFilter {
                options: vec![
                    FilterOption {
                        value: "and".to_string(),
                        label: "AND (must have all selected warnings)".to_string(),
                        description: None,
                    },
                    FilterOption {
                        value: "or".to_string(),
                        label: "OR (must have any selected warning)".to_string(),
                        description: None,
                    },
                ],
            }),
            required: false,
        },
        FilterDefinition {
            id: "content_warnings".to_string(),
            name: "Content Warnings".to_string(),
            description: Some("Select content warnings to include or exclude".to_string()),
            filter_type: FilterType::MultiSelect(MultiSelectFilter {
                options: vec![
                    FilterOption {
                        value: "gore".to_string(),
                        label: "Gore".to_string(),
                        description: None,
                    },
                    FilterOption {
                        value: "sexual_content".to_string(),
                        label: "Sexual Content".to_string(),
                        description: None,
                    },
                    FilterOption {
                        value: "strong_language".to_string(),
                        label: "Strong Language".to_string(),
                        description: None,
                    },
                ],
                max_selections: None,
            }),
            required: false,
        },
        FilterDefinition {
            id: "story_status".to_string(),
            name: "Story Status".to_string(),
            description: Some("Filter by completion status".to_string()),
            filter_type: FilterType::Select(SelectFilter {
                options: vec![
                    FilterOption {
                        value: "all".to_string(),
                        label: "All".to_string(),
                        description: None,
                    },
                    FilterOption {
                        value: "completed".to_string(),
                        label: "Completed".to_string(),
                        description: None,
                    },
                    FilterOption {
                        value: "ongoing".to_string(),
                        label: "Ongoing".to_string(),
                        description: None,
                    },
                    FilterOption {
                        value: "hiatus".to_string(),
                        label: "Hiatus".to_string(),
                        description: None,
                    },
                ],
            }),
            required: false,
        },
        FilterDefinition {
            id: "fandom".to_string(),
            name: "Fandom".to_string(),
            description: Some("Search for specific fandom/franchise".to_string()),
            filter_type: FilterType::Text(TextFilter {
                placeholder: Some("Type for suggestions".to_string()),
                max_length: Some(255),
            }),
            required: false,
        },
    ]
}

pub fn create_sort_options() -> Vec<SortOption> {
    vec![
        SortOption {
            id: "chapters".to_string(),
            name: "Chapters".to_string(),
            description: Some("Sort by number of chapters".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "frequency".to_string(),
            name: "Chapters per Week".to_string(),
            description: Some("Sort by release frequency".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "dateadded".to_string(),
            name: "Date Added".to_string(),
            description: Some("Sort by when the series was added".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "favorites".to_string(),
            name: "Favorites".to_string(),
            description: Some("Sort by number of favorites".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "lastchpdate".to_string(),
            name: "Last Update".to_string(),
            description: Some("Sort by last chapter update".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "numofrate".to_string(),
            name: "Number of Ratings".to_string(),
            description: Some("Sort by number of ratings".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "pages".to_string(),
            name: "Pages".to_string(),
            description: Some("Sort by number of pages".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "pageviews".to_string(),
            name: "Pageviews".to_string(),
            description: Some("Sort by number of pageviews".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "ratings".to_string(),
            name: "Ratings".to_string(),
            description: Some("Sort by average rating".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "readers".to_string(),
            name: "Readers".to_string(),
            description: Some("Sort by number of readers".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "reviews".to_string(),
            name: "Reviews".to_string(),
            description: Some("Sort by number of reviews".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
        SortOption {
            id: "totalwords".to_string(),
            name: "Total Words".to_string(),
            description: Some("Sort by total word count".to_string()),
            supports_asc: true,
            supports_desc: true,
            default_order: Some(SortOrder::Desc),
        },
    ]
}

pub fn create_genre_options() -> Vec<FilterOption> {
    vec![
        FilterOption {
            value: "9".to_string(),
            label: "Action".to_string(),
            description: None,
        },
        FilterOption {
            value: "902".to_string(),
            label: "Adult".to_string(),
            description: None,
        },
        FilterOption {
            value: "8".to_string(),
            label: "Adventure".to_string(),
            description: None,
        },
        FilterOption {
            value: "891".to_string(),
            label: "Boys Love".to_string(),
            description: None,
        },
        FilterOption {
            value: "7".to_string(),
            label: "Comedy".to_string(),
            description: None,
        },
        FilterOption {
            value: "903".to_string(),
            label: "Drama".to_string(),
            description: None,
        },
        FilterOption {
            value: "904".to_string(),
            label: "Ecchi".to_string(),
            description: None,
        },
        FilterOption {
            value: "38".to_string(),
            label: "Fanfiction".to_string(),
            description: None,
        },
        FilterOption {
            value: "19".to_string(),
            label: "Fantasy".to_string(),
            description: None,
        },
        FilterOption {
            value: "905".to_string(),
            label: "Gender Bender".to_string(),
            description: None,
        },
        FilterOption {
            value: "892".to_string(),
            label: "Girls Love".to_string(),
            description: None,
        },
        FilterOption {
            value: "1015".to_string(),
            label: "Harem".to_string(),
            description: None,
        },
        FilterOption {
            value: "21".to_string(),
            label: "Historical".to_string(),
            description: None,
        },
        FilterOption {
            value: "22".to_string(),
            label: "Horror".to_string(),
            description: None,
        },
        FilterOption {
            value: "37".to_string(),
            label: "Isekai".to_string(),
            description: None,
        },
        FilterOption {
            value: "906".to_string(),
            label: "Josei".to_string(),
            description: None,
        },
        FilterOption {
            value: "1180".to_string(),
            label: "LitRPG".to_string(),
            description: None,
        },
        FilterOption {
            value: "907".to_string(),
            label: "Martial Arts".to_string(),
            description: None,
        },
        FilterOption {
            value: "20".to_string(),
            label: "Mature".to_string(),
            description: None,
        },
        FilterOption {
            value: "908".to_string(),
            label: "Mecha".to_string(),
            description: None,
        },
        FilterOption {
            value: "909".to_string(),
            label: "Mystery".to_string(),
            description: None,
        },
        FilterOption {
            value: "910".to_string(),
            label: "Psychological".to_string(),
            description: None,
        },
        FilterOption {
            value: "6".to_string(),
            label: "Romance".to_string(),
            description: None,
        },
        FilterOption {
            value: "911".to_string(),
            label: "School Life".to_string(),
            description: None,
        },
        FilterOption {
            value: "912".to_string(),
            label: "Sci-fi".to_string(),
            description: None,
        },
        FilterOption {
            value: "913".to_string(),
            label: "Seinen".to_string(),
            description: None,
        },
        FilterOption {
            value: "914".to_string(),
            label: "Slice of Life".to_string(),
            description: None,
        },
        FilterOption {
            value: "915".to_string(),
            label: "Smut".to_string(),
            description: None,
        },
        FilterOption {
            value: "916".to_string(),
            label: "Sports".to_string(),
            description: None,
        },
        FilterOption {
            value: "5".to_string(),
            label: "Supernatural".to_string(),
            description: None,
        },
        FilterOption {
            value: "901".to_string(),
            label: "Tragedy".to_string(),
            description: None,
        },
    ]
}

pub fn create_tag_options() -> Vec<FilterOption> {
    vec![
        FilterOption {
            value: "119".to_string(),
            label: "Abandoned Children".to_string(),
            description: None,
        },
        FilterOption {
            value: "120".to_string(),
            label: "Ability Steal".to_string(),
            description: None,
        },
        FilterOption {
            value: "121".to_string(),
            label: "Absent Parents".to_string(),
            description: None,
        },
        FilterOption {
            value: "122".to_string(),
            label: "Abusive Characters".to_string(),
            description: None,
        },
        FilterOption {
            value: "123".to_string(),
            label: "Academy".to_string(),
            description: None,
        },
        FilterOption {
            value: "124".to_string(),
            label: "Accelerated Growth".to_string(),
            description: None,
        },
        FilterOption {
            value: "125".to_string(),
            label: "Acting".to_string(),
            description: None,
        },
        FilterOption {
            value: "137".to_string(),
            label: "Adopted Children".to_string(),
            description: None,
        },
        FilterOption {
            value: "138".to_string(),
            label: "Adopted Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "145".to_string(),
            label: "Alchemy".to_string(),
            description: None,
        },
        FilterOption {
            value: "146".to_string(),
            label: "Aliens".to_string(),
            description: None,
        },
        FilterOption {
            value: "148".to_string(),
            label: "Alternate World".to_string(),
            description: None,
        },
        FilterOption {
            value: "149".to_string(),
            label: "Amnesia".to_string(),
            description: None,
        },
        FilterOption {
            value: "152".to_string(),
            label: "Ancient China".to_string(),
            description: None,
        },
        FilterOption {
            value: "153".to_string(),
            label: "Ancient Times".to_string(),
            description: None,
        },
        FilterOption {
            value: "156".to_string(),
            label: "Angels".to_string(),
            description: None,
        },
        FilterOption {
            value: "160".to_string(),
            label: "Anti-social Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "161".to_string(),
            label: "Antihero Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "164".to_string(),
            label: "Apathetic Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "165".to_string(),
            label: "Apocalypse".to_string(),
            description: None,
        },
        FilterOption {
            value: "169".to_string(),
            label: "Aristocracy".to_string(),
            description: None,
        },
        FilterOption {
            value: "171".to_string(),
            label: "Army".to_string(),
            description: None,
        },
        FilterOption {
            value: "172".to_string(),
            label: "Army Building".to_string(),
            description: None,
        },
        FilterOption {
            value: "173".to_string(),
            label: "Arranged Marriage".to_string(),
            description: None,
        },
        FilterOption {
            value: "174".to_string(),
            label: "Arrogant Characters".to_string(),
            description: None,
        },
        FilterOption {
            value: "175".to_string(),
            label: "Artifact Crafting".to_string(),
            description: None,
        },
        FilterOption {
            value: "176".to_string(),
            label: "Artifacts".to_string(),
            description: None,
        },
        FilterOption {
            value: "177".to_string(),
            label: "Artificial Intelligence".to_string(),
            description: None,
        },
        FilterOption {
            value: "179".to_string(),
            label: "Assassins".to_string(),
            description: None,
        },
        FilterOption {
            value: "183".to_string(),
            label: "Average-looking Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "193".to_string(),
            label: "Battle Academy".to_string(),
            description: None,
        },
        FilterOption {
            value: "194".to_string(),
            label: "Battle Competition".to_string(),
            description: None,
        },
        FilterOption {
            value: "196".to_string(),
            label: "Beast Companions".to_string(),
            description: None,
        },
        FilterOption {
            value: "197".to_string(),
            label: "Beastkin".to_string(),
            description: None,
        },
        FilterOption {
            value: "198".to_string(),
            label: "Beasts".to_string(),
            description: None,
        },
        FilterOption {
            value: "200".to_string(),
            label: "Beautiful Female Lead".to_string(),
            description: None,
        },
        FilterOption {
            value: "202".to_string(),
            label: "Betrayal".to_string(),
            description: None,
        },
        FilterOption {
            value: "207".to_string(),
            label: "Black Belly".to_string(),
            description: None,
        },
        FilterOption {
            value: "209".to_string(),
            label: "Blacksmith".to_string(),
            description: None,
        },
        FilterOption {
            value: "213".to_string(),
            label: "Bloodlines".to_string(),
            description: None,
        },
        FilterOption {
            value: "215".to_string(),
            label: "Body Tempering".to_string(),
            description: None,
        },
        FilterOption {
            value: "217".to_string(),
            label: "Bodyguards".to_string(),
            description: None,
        },
        FilterOption {
            value: "219".to_string(),
            label: "Bookworm".to_string(),
            description: None,
        },
        FilterOption {
            value: "225".to_string(),
            label: "Brotherhood".to_string(),
            description: None,
        },
        FilterOption {
            value: "227".to_string(),
            label: "Bullying".to_string(),
            description: None,
        },
        FilterOption {
            value: "228".to_string(),
            label: "Business Management".to_string(),
            description: None,
        },
        FilterOption {
            value: "231".to_string(),
            label: "Calm Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "235".to_string(),
            label: "Caring Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "236".to_string(),
            label: "Cautious Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "237".to_string(),
            label: "Celebrities".to_string(),
            description: None,
        },
        FilterOption {
            value: "238".to_string(),
            label: "Character Growth".to_string(),
            description: None,
        },
        FilterOption {
            value: "239".to_string(),
            label: "Charismatic Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "242".to_string(),
            label: "Cheats".to_string(),
            description: None,
        },
        FilterOption {
            value: "245".to_string(),
            label: "Child Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "247".to_string(),
            label: "Childhood Friends".to_string(),
            description: None,
        },
        FilterOption {
            value: "252".to_string(),
            label: "Clan Building".to_string(),
            description: None,
        },
        FilterOption {
            value: "254".to_string(),
            label: "Clever Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "262".to_string(),
            label: "Cold Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "264".to_string(),
            label: "College/University".to_string(),
            description: None,
        },
        FilterOption {
            value: "267".to_string(),
            label: "Coming of Age".to_string(),
            description: None,
        },
        FilterOption {
            value: "270".to_string(),
            label: "Confident Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "273".to_string(),
            label: "Conspiracies".to_string(),
            description: None,
        },
        FilterOption {
            value: "275".to_string(),
            label: "Cooking".to_string(),
            description: None,
        },
        FilterOption {
            value: "276".to_string(),
            label: "Corruption".to_string(),
            description: None,
        },
        FilterOption {
            value: "283".to_string(),
            label: "Crafting".to_string(),
            description: None,
        },
        FilterOption {
            value: "284".to_string(),
            label: "Crime".to_string(),
            description: None,
        },
        FilterOption {
            value: "290".to_string(),
            label: "Cultivation".to_string(),
            description: None,
        },
        FilterOption {
            value: "292".to_string(),
            label: "Cunning Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "294".to_string(),
            label: "Curses".to_string(),
            description: None,
        },
        FilterOption {
            value: "296".to_string(),
            label: "Cute Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "302".to_string(),
            label: "Dark".to_string(),
            description: None,
        },
        FilterOption {
            value: "304".to_string(),
            label: "Death".to_string(),
            description: None,
        },
        FilterOption {
            value: "309".to_string(),
            label: "Demi-Humans".to_string(),
            description: None,
        },
        FilterOption {
            value: "310".to_string(),
            label: "Demon Lord".to_string(),
            description: None,
        },
        FilterOption {
            value: "311".to_string(),
            label: "Demonic Cultivation Technique".to_string(),
            description: None,
        },
        FilterOption {
            value: "312".to_string(),
            label: "Demons".to_string(),
            description: None,
        },
        FilterOption {
            value: "313".to_string(),
            label: "Dense Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "315".to_string(),
            label: "Depression".to_string(),
            description: None,
        },
        FilterOption {
            value: "316".to_string(),
            label: "Destiny".to_string(),
            description: None,
        },
        FilterOption {
            value: "317".to_string(),
            label: "Detectives".to_string(),
            description: None,
        },
        FilterOption {
            value: "318".to_string(),
            label: "Determined Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "320".to_string(),
            label: "Different Social Status".to_string(),
            description: None,
        },
        FilterOption {
            value: "322".to_string(),
            label: "Discrimination".to_string(),
            description: None,
        },
        FilterOption {
            value: "324".to_string(),
            label: "Dishonest Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "327".to_string(),
            label: "Divine Protection".to_string(),
            description: None,
        },
        FilterOption {
            value: "329".to_string(),
            label: "Doctors".to_string(),
            description: None,
        },
        FilterOption {
            value: "335".to_string(),
            label: "Dragon Riders".to_string(),
            description: None,
        },
        FilterOption {
            value: "336".to_string(),
            label: "Dragon Slayers".to_string(),
            description: None,
        },
        FilterOption {
            value: "337".to_string(),
            label: "Dragons".to_string(),
            description: None,
        },
        FilterOption {
            value: "338".to_string(),
            label: "Dreams".to_string(),
            description: None,
        },
        FilterOption {
            value: "341".to_string(),
            label: "Dungeon Master".to_string(),
            description: None,
        },
        FilterOption {
            value: "342".to_string(),
            label: "Dungeons".to_string(),
            description: None,
        },
        FilterOption {
            value: "343".to_string(),
            label: "Dwarfs".to_string(),
            description: None,
        },
        FilterOption {
            value: "344".to_string(),
            label: "Dystopia".to_string(),
            description: None,
        },
        FilterOption {
            value: "346".to_string(),
            label: "Early Romance".to_string(),
            description: None,
        },
        FilterOption {
            value: "353".to_string(),
            label: "Elemental Magic".to_string(),
            description: None,
        },
        FilterOption {
            value: "354".to_string(),
            label: "Elves".to_string(),
            description: None,
        },
        FilterOption {
            value: "356".to_string(),
            label: "Empires".to_string(),
            description: None,
        },
        FilterOption {
            value: "357".to_string(),
            label: "Enemies Become Allies".to_string(),
            description: None,
        },
        FilterOption {
            value: "358".to_string(),
            label: "Enemies Become Lovers".to_string(),
            description: None,
        },
        FilterOption {
            value: "361".to_string(),
            label: "Enlightenment".to_string(),
            description: None,
        },
        FilterOption {
            value: "365".to_string(),
            label: "Evil Gods".to_string(),
            description: None,
        },
        FilterOption {
            value: "366".to_string(),
            label: "Evil Organizations".to_string(),
            description: None,
        },
        FilterOption {
            value: "367".to_string(),
            label: "Evil Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "369".to_string(),
            label: "Evolution".to_string(),
            description: None,
        },
        FilterOption {
            value: "372".to_string(),
            label: "Eye Powers".to_string(),
            description: None,
        },
        FilterOption {
            value: "373".to_string(),
            label: "Fairies".to_string(),
            description: None,
        },
        FilterOption {
            value: "374".to_string(),
            label: "Fallen Angels".to_string(),
            description: None,
        },
        FilterOption {
            value: "375".to_string(),
            label: "Fallen Nobility".to_string(),
            description: None,
        },
        FilterOption {
            value: "377".to_string(),
            label: "Familiars".to_string(),
            description: None,
        },
        FilterOption {
            value: "378".to_string(),
            label: "Family".to_string(),
            description: None,
        },
        FilterOption {
            value: "382".to_string(),
            label: "Famous Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "385".to_string(),
            label: "Fantasy Creatures".to_string(),
            description: None,
        },
        FilterOption {
            value: "386".to_string(),
            label: "Fantasy World".to_string(),
            description: None,
        },
        FilterOption {
            value: "387".to_string(),
            label: "Farming".to_string(),
            description: None,
        },
        FilterOption {
            value: "388".to_string(),
            label: "Fast Cultivation".to_string(),
            description: None,
        },
        FilterOption {
            value: "389".to_string(),
            label: "Fast Learner".to_string(),
            description: None,
        },
        FilterOption {
            value: "391".to_string(),
            label: "Fat to Fit".to_string(),
            description: None,
        },
        FilterOption {
            value: "396".to_string(),
            label: "Female Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "400".to_string(),
            label: "First Love".to_string(),
            description: None,
        },
        FilterOption {
            value: "407".to_string(),
            label: "Forced Marriage".to_string(),
            description: None,
        },
        FilterOption {
            value: "409".to_string(),
            label: "Former Hero".to_string(),
            description: None,
        },
        FilterOption {
            value: "410".to_string(),
            label: "Fox Spirits".to_string(),
            description: None,
        },
        FilterOption {
            value: "412".to_string(),
            label: "Friendship".to_string(),
            description: None,
        },
        FilterOption {
            value: "415".to_string(),
            label: "Futuristic Setting".to_string(),
            description: None,
        },
        FilterOption {
            value: "418".to_string(),
            label: "Game Elements".to_string(),
            description: None,
        },
        FilterOption {
            value: "420".to_string(),
            label: "Gamers".to_string(),
            description: None,
        },
        FilterOption {
            value: "422".to_string(),
            label: "Gate to Another World".to_string(),
            description: None,
        },
        FilterOption {
            value: "424".to_string(),
            label: "Generals".to_string(),
            description: None,
        },
        FilterOption {
            value: "425".to_string(),
            label: "Genetic Modifications".to_string(),
            description: None,
        },
        FilterOption {
            value: "427".to_string(),
            label: "Genius Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "428".to_string(),
            label: "Ghosts".to_string(),
            description: None,
        },
        FilterOption {
            value: "429".to_string(),
            label: "Gladiators".to_string(),
            description: None,
        },
        FilterOption {
            value: "432".to_string(),
            label: "Goblins".to_string(),
            description: None,
        },
        FilterOption {
            value: "433".to_string(),
            label: "God Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "435".to_string(),
            label: "Goddesses".to_string(),
            description: None,
        },
        FilterOption {
            value: "436".to_string(),
            label: "Godly Powers".to_string(),
            description: None,
        },
        FilterOption {
            value: "437".to_string(),
            label: "Gods".to_string(),
            description: None,
        },
        FilterOption {
            value: "443".to_string(),
            label: "Guilds".to_string(),
            description: None,
        },
        FilterOption {
            value: "445".to_string(),
            label: "Hackers".to_string(),
            description: None,
        },
        FilterOption {
            value: "448".to_string(),
            label: "Handsome Male Lead".to_string(),
            description: None,
        },
        FilterOption {
            value: "449".to_string(),
            label: "Hard-Working Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "453".to_string(),
            label: "Healers".to_string(),
            description: None,
        },
        FilterOption {
            value: "455".to_string(),
            label: "Heaven".to_string(),
            description: None,
        },
        FilterOption {
            value: "456".to_string(),
            label: "Heavenly Tribulation".to_string(),
            description: None,
        },
        FilterOption {
            value: "457".to_string(),
            label: "Hell".to_string(),
            description: None,
        },
        FilterOption {
            value: "460".to_string(),
            label: "Heroes".to_string(),
            description: None,
        },
        FilterOption {
            value: "462".to_string(),
            label: "Hidden Abilities".to_string(),
            description: None,
        },
        FilterOption {
            value: "463".to_string(),
            label: "Hiding True Abilities".to_string(),
            description: None,
        },
        FilterOption {
            value: "464".to_string(),
            label: "Hiding True Identity".to_string(),
            description: None,
        },
        FilterOption {
            value: "467".to_string(),
            label: "Honest Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "472".to_string(),
            label: "Human-Nonhuman Relationship".to_string(),
            description: None,
        },
        FilterOption {
            value: "474".to_string(),
            label: "Hunters".to_string(),
            description: None,
        },
        FilterOption {
            value: "478".to_string(),
            label: "Immortals".to_string(),
            description: None,
        },
        FilterOption {
            value: "479".to_string(),
            label: "Imperial Harem".to_string(),
            description: None,
        },
        FilterOption {
            value: "481".to_string(),
            label: "Incubus".to_string(),
            description: None,
        },
        FilterOption {
            value: "489".to_string(),
            label: "Interdimensional Travel".to_string(),
            description: None,
        },
        FilterOption {
            value: "490".to_string(),
            label: "Introverted Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "493".to_string(),
            label: "Jack of All Trades".to_string(),
            description: None,
        },
        FilterOption {
            value: "500".to_string(),
            label: "Kingdom Building".to_string(),
            description: None,
        },
        FilterOption {
            value: "501".to_string(),
            label: "Kingdoms".to_string(),
            description: None,
        },
        FilterOption {
            value: "502".to_string(),
            label: "Knights".to_string(),
            description: None,
        },
        FilterOption {
            value: "508".to_string(),
            label: "Lazy Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "509".to_string(),
            label: "Leadership".to_string(),
            description: None,
        },
        FilterOption {
            value: "511".to_string(),
            label: "Level System".to_string(),
            description: None,
        },
        FilterOption {
            value: "516".to_string(),
            label: "Loli".to_string(),
            description: None,
        },
        FilterOption {
            value: "518".to_string(),
            label: "Loner Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "523".to_string(),
            label: "Love at First Sight".to_string(),
            description: None,
        },
        FilterOption {
            value: "525".to_string(),
            label: "Love Rivals".to_string(),
            description: None,
        },
        FilterOption {
            value: "526".to_string(),
            label: "Love Triangles".to_string(),
            description: None,
        },
        FilterOption {
            value: "530".to_string(),
            label: "Lucky Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "531".to_string(),
            label: "Magic".to_string(),
            description: None,
        },
        FilterOption {
            value: "532".to_string(),
            label: "Magic Beasts".to_string(),
            description: None,
        },
        FilterOption {
            value: "534".to_string(),
            label: "Magical Girls".to_string(),
            description: None,
        },
        FilterOption {
            value: "537".to_string(),
            label: "Maids".to_string(),
            description: None,
        },
        FilterOption {
            value: "538".to_string(),
            label: "Male Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "543".to_string(),
            label: "Manipulative Characters".to_string(),
            description: None,
        },
        FilterOption {
            value: "545".to_string(),
            label: "Marriage".to_string(),
            description: None,
        },
        FilterOption {
            value: "549".to_string(),
            label: "Master-Disciple Relationship".to_string(),
            description: None,
        },
        FilterOption {
            value: "553".to_string(),
            label: "Mature Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "554".to_string(),
            label: "Medical Knowledge".to_string(),
            description: None,
        },
        FilterOption {
            value: "555".to_string(),
            label: "Medieval".to_string(),
            description: None,
        },
        FilterOption {
            value: "556".to_string(),
            label: "Mercenaries".to_string(),
            description: None,
        },
        FilterOption {
            value: "557".to_string(),
            label: "Merchants".to_string(),
            description: None,
        },
        FilterOption {
            value: "558".to_string(),
            label: "Military".to_string(),
            description: None,
        },
        FilterOption {
            value: "560".to_string(),
            label: "Mind Control".to_string(),
            description: None,
        },
        FilterOption {
            value: "563".to_string(),
            label: "Misunderstandings".to_string(),
            description: None,
        },
        FilterOption {
            value: "564".to_string(),
            label: "MMORPG".to_string(),
            description: None,
        },
        FilterOption {
            value: "567".to_string(),
            label: "Modern Day".to_string(),
            description: None,
        },
        FilterOption {
            value: "568".to_string(),
            label: "Modern Knowledge".to_string(),
            description: None,
        },
        FilterOption {
            value: "570".to_string(),
            label: "Monster Girls".to_string(),
            description: None,
        },
        FilterOption {
            value: "572".to_string(),
            label: "Monster Tamer".to_string(),
            description: None,
        },
        FilterOption {
            value: "573".to_string(),
            label: "Monsters".to_string(),
            description: None,
        },
        FilterOption {
            value: "576".to_string(),
            label: "Multiple Identities".to_string(),
            description: None,
        },
        FilterOption {
            value: "578".to_string(),
            label: "Multiple POV".to_string(),
            description: None,
        },
        FilterOption {
            value: "579".to_string(),
            label: "Multiple Protagonists".to_string(),
            description: None,
        },
        FilterOption {
            value: "581".to_string(),
            label: "Multiple Reincarnated Individuals".to_string(),
            description: None,
        },
        FilterOption {
            value: "583".to_string(),
            label: "Multiple Transported Individuals".to_string(),
            description: None,
        },
        FilterOption {
            value: "585".to_string(),
            label: "Music".to_string(),
            description: None,
        },
        FilterOption {
            value: "589".to_string(),
            label: "Mysterious Family Background".to_string(),
            description: None,
        },
        FilterOption {
            value: "591".to_string(),
            label: "Mysterious Past".to_string(),
            description: None,
        },
        FilterOption {
            value: "592".to_string(),
            label: "Mystery Solving".to_string(),
            description: None,
        },
        FilterOption {
            value: "593".to_string(),
            label: "Mythical Beasts".to_string(),
            description: None,
        },
        FilterOption {
            value: "595".to_string(),
            label: "Naive Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "598".to_string(),
            label: "Near-Death Experience".to_string(),
            description: None,
        },
        FilterOption {
            value: "599".to_string(),
            label: "Necromancer".to_string(),
            description: None,
        },
        FilterOption {
            value: "600".to_string(),
            label: "Neet".to_string(),
            description: None,
        },
        FilterOption {
            value: "605".to_string(),
            label: "Ninjas".to_string(),
            description: None,
        },
        FilterOption {
            value: "606".to_string(),
            label: "Nobles".to_string(),
            description: None,
        },
        FilterOption {
            value: "621".to_string(),
            label: "Orphans".to_string(),
            description: None,
        },
        FilterOption {
            value: "622".to_string(),
            label: "Otaku".to_string(),
            description: None,
        },
        FilterOption {
            value: "623".to_string(),
            label: "Otome Game".to_string(),
            description: None,
        },
        FilterOption {
            value: "626".to_string(),
            label: "Outer Space".to_string(),
            description: None,
        },
        FilterOption {
            value: "627".to_string(),
            label: "Overpowered Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "631".to_string(),
            label: "Parallel Worlds".to_string(),
            description: None,
        },
        FilterOption {
            value: "636".to_string(),
            label: "Past Plays a Big Role".to_string(),
            description: None,
        },
        FilterOption {
            value: "637".to_string(),
            label: "Past Trauma".to_string(),
            description: None,
        },
        FilterOption {
            value: "641".to_string(),
            label: "Pets".to_string(),
            description: None,
        },
        FilterOption {
            value: "645".to_string(),
            label: "Phoenixes".to_string(),
            description: None,
        },
        FilterOption {
            value: "647".to_string(),
            label: "Pill Based Cultivation".to_string(),
            description: None,
        },
        FilterOption {
            value: "648".to_string(),
            label: "Pill Concocting".to_string(),
            description: None,
        },
        FilterOption {
            value: "650".to_string(),
            label: "Pirates".to_string(),
            description: None,
        },
        FilterOption {
            value: "654".to_string(),
            label: "Poisons".to_string(),
            description: None,
        },
        FilterOption {
            value: "657".to_string(),
            label: "Politics".to_string(),
            description: None,
        },
        FilterOption {
            value: "660".to_string(),
            label: "Poor Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "661".to_string(),
            label: "Poor to Rich".to_string(),
            description: None,
        },
        FilterOption {
            value: "663".to_string(),
            label: "Possession".to_string(),
            description: None,
        },
        FilterOption {
            value: "664".to_string(),
            label: "Possessive Characters".to_string(),
            description: None,
        },
        FilterOption {
            value: "665".to_string(),
            label: "Post-apocalyptic".to_string(),
            description: None,
        },
        FilterOption {
            value: "667".to_string(),
            label: "Power Struggle".to_string(),
            description: None,
        },
        FilterOption {
            value: "668".to_string(),
            label: "Pragmatic Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "669".to_string(),
            label: "Precognition".to_string(),
            description: None,
        },
        FilterOption {
            value: "670".to_string(),
            label: "Pregnancy".to_string(),
            description: None,
        },
        FilterOption {
            value: "672".to_string(),
            label: "Previous Life Talent".to_string(),
            description: None,
        },
        FilterOption {
            value: "673".to_string(),
            label: "Priestesses".to_string(),
            description: None,
        },
        FilterOption {
            value: "674".to_string(),
            label: "Priests".to_string(),
            description: None,
        },
        FilterOption {
            value: "676".to_string(),
            label: "Proactive Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "678".to_string(),
            label: "Prophecies".to_string(),
            description: None,
        },
        FilterOption {
            value: "682".to_string(),
            label: "Protagonist Strong from the Start".to_string(),
            description: None,
        },
        FilterOption {
            value: "684".to_string(),
            label: "Psychic Powers".to_string(),
            description: None,
        },
        FilterOption {
            value: "685".to_string(),
            label: "Psychopaths".to_string(),
            description: None,
        },
        FilterOption {
            value: "692".to_string(),
            label: "Racism".to_string(),
            description: None,
        },
        FilterOption {
            value: "695".to_string(),
            label: "Rebellion".to_string(),
            description: None,
        },
        FilterOption {
            value: "696".to_string(),
            label: "Reincarnated as a Monster".to_string(),
            description: None,
        },
        FilterOption {
            value: "698".to_string(),
            label: "Reincarnated into a Game World".to_string(),
            description: None,
        },
        FilterOption {
            value: "699".to_string(),
            label: "Reincarnated into Another World".to_string(),
            description: None,
        },
        FilterOption {
            value: "700".to_string(),
            label: "Reincarnation".to_string(),
            description: None,
        },
        FilterOption {
            value: "701".to_string(),
            label: "Religions".to_string(),
            description: None,
        },
        FilterOption {
            value: "705".to_string(),
            label: "Resurrection".to_string(),
            description: None,
        },
        FilterOption {
            value: "706".to_string(),
            label: "Returning from Another World".to_string(),
            description: None,
        },
        FilterOption {
            value: "707".to_string(),
            label: "Revenge".to_string(),
            description: None,
        },
        FilterOption {
            value: "708".to_string(),
            label: "Reverse Harem".to_string(),
            description: None,
        },
        FilterOption {
            value: "711".to_string(),
            label: "Righteous Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "712".to_string(),
            label: "Rivalry".to_string(),
            description: None,
        },
        FilterOption {
            value: "715".to_string(),
            label: "Royalty".to_string(),
            description: None,
        },
        FilterOption {
            value: "716".to_string(),
            label: "Ruthless Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "718".to_string(),
            label: "Saints".to_string(),
            description: None,
        },
        FilterOption {
            value: "721".to_string(),
            label: "Saving the World".to_string(),
            description: None,
        },
        FilterOption {
            value: "722".to_string(),
            label: "Scheming".to_string(),
            description: None,
        },
        FilterOption {
            value: "724".to_string(),
            label: "Scientists".to_string(),
            description: None,
        },
        FilterOption {
            value: "726".to_string(),
            label: "Sealed Power".to_string(),
            description: None,
        },
        FilterOption {
            value: "727".to_string(),
            label: "Second Chance".to_string(),
            description: None,
        },
        FilterOption {
            value: "729".to_string(),
            label: "Secret Identity".to_string(),
            description: None,
        },
        FilterOption {
            value: "730".to_string(),
            label: "Secret Organizations".to_string(),
            description: None,
        },
        FilterOption {
            value: "734".to_string(),
            label: "Sect Development".to_string(),
            description: None,
        },
        FilterOption {
            value: "737".to_string(),
            label: "Selfish Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "738".to_string(),
            label: "Selfless Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "743".to_string(),
            label: "Serial Killers".to_string(),
            description: None,
        },
        FilterOption {
            value: "744".to_string(),
            label: "Servants".to_string(),
            description: None,
        },
        FilterOption {
            value: "751".to_string(),
            label: "Shameless Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "752".to_string(),
            label: "Shapeshifters".to_string(),
            description: None,
        },
        FilterOption {
            value: "755".to_string(),
            label: "Shield User".to_string(),
            description: None,
        },
        FilterOption {
            value: "758".to_string(),
            label: "Shota".to_string(),
            description: None,
        },
        FilterOption {
            value: "761".to_string(),
            label: "Showbiz".to_string(),
            description: None,
        },
        FilterOption {
            value: "762".to_string(),
            label: "Shy Characters".to_string(),
            description: None,
        },
        FilterOption {
            value: "765".to_string(),
            label: "Siblings".to_string(),
            description: None,
        },
        FilterOption {
            value: "772".to_string(),
            label: "Skill Assimilation".to_string(),
            description: None,
        },
        FilterOption {
            value: "774".to_string(),
            label: "Skill Creation".to_string(),
            description: None,
        },
        FilterOption {
            value: "775".to_string(),
            label: "Slave Harem".to_string(),
            description: None,
        },
        FilterOption {
            value: "776".to_string(),
            label: "Slave Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "777".to_string(),
            label: "Slaves".to_string(),
            description: None,
        },
        FilterOption {
            value: "779".to_string(),
            label: "Slow Growth at Start".to_string(),
            description: None,
        },
        FilterOption {
            value: "780".to_string(),
            label: "Slow Romance".to_string(),
            description: None,
        },
        FilterOption {
            value: "783".to_string(),
            label: "Soldiers".to_string(),
            description: None,
        },
        FilterOption {
            value: "784".to_string(),
            label: "Soul Power".to_string(),
            description: None,
        },
        FilterOption {
            value: "785".to_string(),
            label: "Souls".to_string(),
            description: None,
        },
        FilterOption {
            value: "787".to_string(),
            label: "Spear Wielder".to_string(),
            description: None,
        },
        FilterOption {
            value: "788".to_string(),
            label: "Special Abilities".to_string(),
            description: None,
        },
        FilterOption {
            value: "792".to_string(),
            label: "Spirits".to_string(),
            description: None,
        },
        FilterOption {
            value: "800".to_string(),
            label: "Strategist".to_string(),
            description: None,
        },
        FilterOption {
            value: "803".to_string(),
            label: "Strong to Stronger".to_string(),
            description: None,
        },
        FilterOption {
            value: "812".to_string(),
            label: "Summoned Hero".to_string(),
            description: None,
        },
        FilterOption {
            value: "813".to_string(),
            label: "Summoning Magic".to_string(),
            description: None,
        },
        FilterOption {
            value: "814".to_string(),
            label: "Survival".to_string(),
            description: None,
        },
        FilterOption {
            value: "816".to_string(),
            label: "Sword And Magic".to_string(),
            description: None,
        },
        FilterOption {
            value: "817".to_string(),
            label: "Sword Wielder".to_string(),
            description: None,
        },
        FilterOption {
            value: "819".to_string(),
            label: "Teachers".to_string(),
            description: None,
        },
        FilterOption {
            value: "820".to_string(),
            label: "Teamwork".to_string(),
            description: None,
        },
        FilterOption {
            value: "828".to_string(),
            label: "Time Loop".to_string(),
            description: None,
        },
        FilterOption {
            value: "832".to_string(),
            label: "Time Travel".to_string(),
            description: None,
        },
        FilterOption {
            value: "835".to_string(),
            label: "Torture".to_string(),
            description: None,
        },
        FilterOption {
            value: "839".to_string(),
            label: "Transmigration".to_string(),
            description: None,
        },
        FilterOption {
            value: "841".to_string(),
            label: "Transported into a Game World".to_string(),
            description: None,
        },
        FilterOption {
            value: "842".to_string(),
            label: "Transported into Another World".to_string(),
            description: None,
        },
        FilterOption {
            value: "853".to_string(),
            label: "Underestimated Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "861".to_string(),
            label: "Vampires".to_string(),
            description: None,
        },
        FilterOption {
            value: "862".to_string(),
            label: "Villainess Noble Girls".to_string(),
            description: None,
        },
        FilterOption {
            value: "863".to_string(),
            label: "Virtual Reality".to_string(),
            description: None,
        },
        FilterOption {
            value: "869".to_string(),
            label: "Wars".to_string(),
            description: None,
        },
        FilterOption {
            value: "870".to_string(),
            label: "Weak Protagonist".to_string(),
            description: None,
        },
        FilterOption {
            value: "871".to_string(),
            label: "Weak to Strong".to_string(),
            description: None,
        },
        FilterOption {
            value: "872".to_string(),
            label: "Wealthy Characters".to_string(),
            description: None,
        },
        FilterOption {
            value: "875".to_string(),
            label: "Witches".to_string(),
            description: None,
        },
        FilterOption {
            value: "876".to_string(),
            label: "Wizards".to_string(),
            description: None,
        },
        FilterOption {
            value: "877".to_string(),
            label: "World Hopping".to_string(),
            description: None,
        },
        FilterOption {
            value: "880".to_string(),
            label: "Writers".to_string(),
            description: None,
        },
        FilterOption {
            value: "1141".to_string(),
            label: "Xuanhuan".to_string(),
            description: None,
        },
        FilterOption {
            value: "1142".to_string(),
            label: "Xianxia".to_string(),
            description: None,
        },
        FilterOption {
            value: "1143".to_string(),
            label: "Wuxia".to_string(),
            description: None,
        },
        FilterOption {
            value: "881".to_string(),
            label: "Yandere".to_string(),
            description: None,
        },
        FilterOption {
            value: "886".to_string(),
            label: "Zombies".to_string(),
            description: None,
        },
    ]
}
