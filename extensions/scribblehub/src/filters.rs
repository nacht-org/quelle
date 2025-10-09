use quelle_extension::filters::{FilterBuilder, SortOptionBuilder};
use quelle_extension::prelude::*;
use std::str::FromStr;

/// Strongly typed filter IDs for compile-time safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterId {
    TitleContains,
    Fandom,
    Chapters,
    ReleasesPerweek,
    Favorites,
    Ratings,
    NumRatings,
    Readers,
    Reviews,
    Pages,
    Pageviews,
    TotalWords,
    LastUpdate,
    StoryStatus,
    Genres,
    Tags,
    ContentWarnings,
    GenreMode,
}

impl FilterId {
    pub fn as_str(self) -> &'static str {
        match self {
            FilterId::TitleContains => "title_contains",
            FilterId::Fandom => "fandom",
            FilterId::Chapters => "chapters",
            FilterId::ReleasesPerweek => "releases_perweek",
            FilterId::Favorites => "favorites",
            FilterId::Ratings => "ratings",
            FilterId::NumRatings => "num_ratings",
            FilterId::Readers => "readers",
            FilterId::Reviews => "reviews",
            FilterId::Pages => "pages",
            FilterId::Pageviews => "pageviews",
            FilterId::TotalWords => "total_words",
            FilterId::LastUpdate => "last_update",
            FilterId::StoryStatus => "story_status",
            FilterId::Genres => "genres",
            FilterId::Tags => "tags",
            FilterId::ContentWarnings => "content_warnings",
            FilterId::GenreMode => "genre_mode",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "title_contains" => Some(FilterId::TitleContains),
            "fandom" => Some(FilterId::Fandom),
            "chapters" => Some(FilterId::Chapters),
            "releases_perweek" => Some(FilterId::ReleasesPerweek),
            "favorites" => Some(FilterId::Favorites),
            "ratings" => Some(FilterId::Ratings),
            "num_ratings" => Some(FilterId::NumRatings),
            "readers" => Some(FilterId::Readers),
            "reviews" => Some(FilterId::Reviews),
            "pages" => Some(FilterId::Pages),
            "pageviews" => Some(FilterId::Pageviews),
            "total_words" => Some(FilterId::TotalWords),
            "last_update" => Some(FilterId::LastUpdate),
            "story_status" => Some(FilterId::StoryStatus),
            "genres" => Some(FilterId::Genres),
            "tags" => Some(FilterId::Tags),
            "content_warnings" => Some(FilterId::ContentWarnings),
            "genre_mode" => Some(FilterId::GenreMode),
            _ => None,
        }
    }
}

impl Into<String> for FilterId {
    fn into(self) -> String {
        self.as_str().to_string()
    }
}

impl FromStr for FilterId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s).ok_or(())
    }
}

pub fn create_filter_definitions() -> Vec<FilterDefinition> {
    let mut filters = vec![
        // Text filters
        FilterBuilder::new(FilterId::TitleContains, "Title Contains")
            .description("Search for series with titles containing this text")
            .text_with_options(Some("Title contains..."), Some(255)),
        FilterBuilder::new(FilterId::Fandom, "Fandom")
            .description("Search within a specific fandom")
            .text_with_options(Some("Fandom name..."), Some(255)),
        // Numeric range filters
        FilterBuilder::new(FilterId::Chapters, "Chapters")
            .description("Filter by number of chapters")
            .number_range(0.0, 10000.0, Some(1.0), Some("chapters")),
        FilterBuilder::new(FilterId::ReleasesPerweek, "Chapters per Week")
            .description("Filter by release frequency")
            .number_range(0.0, 50.0, Some(0.1), Some("per week")),
        FilterBuilder::new(FilterId::Favorites, "Favorites")
            .description("Filter by number of favorites")
            .number_range(0.0, 100000.0, Some(1.0), Some("favorites")),
        FilterBuilder::new(FilterId::Ratings, "Ratings")
            .description("Filter by rating score")
            .number_range(0.0, 5.0, Some(0.1), Some("stars")),
        FilterBuilder::new(FilterId::NumRatings, "Number of Ratings")
            .description("Filter by number of ratings")
            .number_range(0.0, 100000.0, Some(1.0), Some("ratings")),
        FilterBuilder::new(FilterId::Readers, "Readers")
            .description("Filter by number of readers")
            .number_range(0.0, 1000000.0, Some(1.0), Some("readers")),
        FilterBuilder::new(FilterId::Reviews, "Reviews")
            .description("Filter by number of reviews")
            .number_range(0.0, 10000.0, Some(1.0), Some("reviews")),
        FilterBuilder::new(FilterId::Pages, "Pages")
            .description("Filter by number of pages")
            .number_range(0.0, 50000.0, Some(1.0), Some("pages")),
        FilterBuilder::new(FilterId::Pageviews, "Pageviews (k)")
            .description("Filter by pageviews in thousands")
            .number_range(0.0, 10000.0, Some(1.0), Some("k views")),
        FilterBuilder::new(FilterId::TotalWords, "Total Words (k)")
            .description("Filter by total word count")
            .number_range(0.0, 10000.0, Some(1000.0), Some("k words")),
        // Date range filter
        FilterBuilder::new(FilterId::LastUpdate, "Last Update")
            .description("Filter by when the novel was last updated")
            .date_range("YYYY-MM-DD", None::<String>, None::<String>),
        // Status filter
        FilterBuilder::new(FilterId::StoryStatus, "Story Status")
            .description("Filter by novel completion status")
            .select(vec![
                FilterOption::new("all", "All"),
                FilterOption::new("completed", "Completed"),
                FilterOption::new("ongoing", "Ongoing"),
                FilterOption::new("hiatus", "Hiatus"),
                FilterOption::new("dropped", "Dropped"),
            ]),
        // Genre mode filter (for AND/OR logic)
        FilterBuilder::new(FilterId::GenreMode, "Genre Mode")
            .description("How to combine genre filters")
            .select(vec![
                FilterOption::new("and", "AND (must have all selected)"),
                FilterOption::new("or", "OR (must have any selected)"),
            ]),
    ];

    // Add tristate genre filters
    let genres = create_genre_options();
    filters.push(
        FilterBuilder::new(FilterId::Genres, "Genres")
            .description("Genre preferences (include/exclude/ignore)")
            .tri_state(genres),
    );

    // Add tristate tag filters
    let tags = create_tag_options();
    filters.push(
        FilterBuilder::new(FilterId::Tags, "Tags")
            .description("Tag preferences (include/exclude/ignore)")
            .tri_state(tags),
    );

    // Add content warning filters
    let warnings = vec![
        FilterOption::new("gore", "Gore"),
        FilterOption::new("sexual_content", "Sexual Content"),
        FilterOption::new("strong_language", "Strong Language"),
        FilterOption::new("violence", "Violence"),
        FilterOption::new("disturbing_content", "Disturbing Content"),
    ];
    filters.push(
        FilterBuilder::new(FilterId::ContentWarnings, "Content Warnings")
            .description("Content warning preferences (include/exclude/ignore)")
            .tri_state(warnings),
    );

    filters
}

pub fn create_sort_options() -> Vec<SortOption> {
    vec![
        SortOptionBuilder::new("pageviews", "Pageviews")
            .description("Sort by total pageviews")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("chapters", "Chapters")
            .description("Sort by number of chapters")
            .build(),
        SortOptionBuilder::new("favorites", "Favorites")
            .description("Sort by number of favorites")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("last_update", "Last Update")
            .description("Sort by when last updated")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("ratings", "Ratings")
            .description("Sort by average rating")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("readers", "Readers")
            .description("Sort by number of readers")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("reviews", "Reviews")
            .description("Sort by number of reviews")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("total_words", "Total Words")
            .description("Sort by total word count")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("pages", "Pages")
            .description("Sort by total pages")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("num_ratings", "Number of Ratings")
            .description("Sort by number of ratings received")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("releases_perweek", "Release Frequency")
            .description("Sort by chapters per week")
            .default_order(SortOrder::Desc)
            .build(),
        SortOptionBuilder::new("date_added", "Date Added")
            .description("Sort by when added to the site")
            .default_order(SortOrder::Desc)
            .build(),
    ]
}

pub fn create_genre_options() -> Vec<FilterOption> {
    vec![
        FilterOption::new("9", "Action"),
        FilterOption::new("902", "Adult"),
        FilterOption::new("8", "Adventure"),
        FilterOption::new("891", "Boys Love"),
        FilterOption::new("7", "Comedy"),
        FilterOption::new("903", "Drama"),
        FilterOption::new("904", "Ecchi"),
        FilterOption::new("38", "Fanfiction"),
        FilterOption::new("19", "Fantasy"),
        FilterOption::new("905", "Gender Bender"),
        FilterOption::new("892", "Girls Love"),
        FilterOption::new("1015", "Harem"),
        FilterOption::new("21", "Historical"),
        FilterOption::new("22", "Horror"),
        FilterOption::new("37", "Isekai"),
        FilterOption::new("912", "Josei"),
        FilterOption::new("906", "LitRPG"),
        FilterOption::new("907", "Martial Arts"),
        FilterOption::new("908", "Mature"),
        FilterOption::new("24", "Mecha"),
        FilterOption::new("25", "Mystery"),
        FilterOption::new("28", "Psychological"),
        FilterOption::new("29", "Romance"),
        FilterOption::new("30", "School Life"),
        FilterOption::new("31", "Sci-fi"),
        FilterOption::new("913", "Seinen"),
        FilterOption::new("914", "Shoujo"),
        FilterOption::new("915", "Shoujo Ai"),
        FilterOption::new("916", "Shounen"),
        FilterOption::new("917", "Shounen Ai"),
        FilterOption::new("32", "Slice of Life"),
        FilterOption::new("33", "Sports"),
        FilterOption::new("34", "Supernatural"),
        FilterOption::new("35", "Tragedy"),
        FilterOption::new("918", "Wuxia"),
        FilterOption::new("919", "Xianxia"),
        FilterOption::new("920", "Xuanhuan"),
        FilterOption::new("921", "Yaoi"),
        FilterOption::new("922", "Yuri"),
    ]
}

pub fn create_tag_options() -> Vec<FilterOption> {
    vec![
        // Popular character types
        FilterOption::new(
            "Protagonist Strong from the Start",
            "Protagonist Strong from the Start",
        ),
        FilterOption::new("Overpowered Protagonist", "Overpowered Protagonist"),
        FilterOption::new("Weak to Strong", "Weak to Strong"),
        FilterOption::new("Male Protagonist", "Male Protagonist"),
        FilterOption::new("Female Protagonist", "Female Protagonist"),
        FilterOption::new("Clever Protagonist", "Clever Protagonist"),
        FilterOption::new("Ruthless Protagonist", "Ruthless Protagonist"),
        FilterOption::new("Anti-Hero Protagonist", "Anti-Hero Protagonist"),
        FilterOption::new("Calm Protagonist", "Calm Protagonist"),
        FilterOption::new("Cold Protagonist", "Cold Protagonist"),
        // System and progression
        FilterOption::new("System", "System"),
        FilterOption::new("Game Elements", "Game Elements"),
        FilterOption::new("Virtual Reality", "Virtual Reality"),
        FilterOption::new("Reincarnation", "Reincarnation"),
        FilterOption::new("Transmigration", "Transmigration"),
        FilterOption::new("Level System", "Level System"),
        FilterOption::new("Skill Books", "Skill Books"),
        FilterOption::new("Cultivation", "Cultivation"),
        FilterOption::new("Magic", "Magic"),
        FilterOption::new("Mage", "Mage"),
        // World building
        FilterOption::new("Alternate World", "Alternate World"),
        FilterOption::new("Another World", "Another World"),
        FilterOption::new("Modern Day", "Modern Day"),
        FilterOption::new("Medieval", "Medieval"),
        FilterOption::new("Futuristic Setting", "Futuristic Setting"),
        FilterOption::new("Post-apocalyptic", "Post-apocalyptic"),
        FilterOption::new("Academy", "Academy"),
        FilterOption::new("School Life", "School Life"),
        FilterOption::new("Nobles", "Nobles"),
        FilterOption::new("Royalty", "Royalty"),
        // Romance and relationships
        FilterOption::new("Romance", "Romance"),
        FilterOption::new("Harem", "Harem"),
        FilterOption::new("Polygamy", "Polygamy"),
        FilterOption::new("Beautiful Female Lead", "Beautiful Female Lead"),
        FilterOption::new("Handsome Male Lead", "Handsome Male Lead"),
        FilterOption::new(
            "Love Interest Falls in Love First",
            "Love Interest Falls in Love First",
        ),
        FilterOption::new("Jealousy", "Jealousy"),
        FilterOption::new("Dense Protagonist", "Dense Protagonist"),
        FilterOption::new("Childhood Friends", "Childhood Friends"),
        FilterOption::new("Arranged Marriage", "Arranged Marriage"),
        // Action and conflict
        FilterOption::new("Wars", "Wars"),
        FilterOption::new("Army Building", "Army Building"),
        FilterOption::new("Kingdom Building", "Kingdom Building"),
        FilterOption::new("Politics", "Politics"),
        FilterOption::new("Revenge", "Revenge"),
        FilterOption::new("Assassins", "Assassins"),
        FilterOption::new("Mercenaries", "Mercenaries"),
        FilterOption::new("Sword And Magic", "Sword And Magic"),
        FilterOption::new("Monsters", "Monsters"),
        FilterOption::new("Demons", "Demons"),
        // Family and social
        FilterOption::new("Family", "Family"),
        FilterOption::new("Friendship", "Friendship"),
        FilterOption::new("Siblings", "Siblings"),
        FilterOption::new("Parent-Child Relationship", "Parent-Child Relationship"),
        FilterOption::new("Loyal Subordinates", "Loyal Subordinates"),
        FilterOption::new("Master-Servant Relationship", "Master-Servant Relationship"),
        FilterOption::new(
            "Teacher-Student Relationship",
            "Teacher-Student Relationship",
        ),
        FilterOption::new("Teamwork", "Teamwork"),
        // Special abilities and powers
        FilterOption::new("Special Abilities", "Special Abilities"),
        FilterOption::new(
            "Unique Cultivation Technique",
            "Unique Cultivation Technique",
        ),
        FilterOption::new("Time Manipulation", "Time Manipulation"),
        FilterOption::new("Space Manipulation", "Space Manipulation"),
        FilterOption::new("Mind Control", "Mind Control"),
        FilterOption::new("Soul Power", "Soul Power"),
        FilterOption::new("Bloodlines", "Bloodlines"),
        FilterOption::new("Ancient Times", "Ancient Times"),
        FilterOption::new("Immortals", "Immortals"),
        FilterOption::new("Gods", "Gods"),
        // Business and economics
        FilterOption::new("Business Management", "Business Management"),
        FilterOption::new("Economics", "Economics"),
        FilterOption::new("Poor to Rich", "Poor to Rich"),
        FilterOption::new("Wealthy Characters", "Wealthy Characters"),
        FilterOption::new("Shop Owner", "Shop Owner"),
        FilterOption::new("Blacksmith", "Blacksmith"),
        FilterOption::new("Alchemist", "Alchemist"),
        // Personality traits
        FilterOption::new("Shameless Protagonist", "Shameless Protagonist"),
        FilterOption::new("Cunning Protagonist", "Cunning Protagonist"),
        FilterOption::new("Lazy Protagonist", "Lazy Protagonist"),
        FilterOption::new("Caring Protagonist", "Caring Protagonist"),
        FilterOption::new("Loner Protagonist", "Loner Protagonist"),
        FilterOption::new("Mysterious Past", "Mysterious Past"),
        FilterOption::new("Hidden Abilities", "Hidden Abilities"),
        FilterOption::new("Confident Protagonist", "Confident Protagonist"),
        // Story elements
        FilterOption::new("Multiple POV", "Multiple POV"),
        FilterOption::new("Fast Paced", "Fast Paced"),
        FilterOption::new("Slow Romance", "Slow Romance"),
        FilterOption::new("Comedy", "Comedy"),
        FilterOption::new("Tragedy", "Tragedy"),
        FilterOption::new("Mystery", "Mystery"),
        FilterOption::new("Supernatural", "Supernatural"),
        FilterOption::new("Slice of Life", "Slice of Life"),
        FilterOption::new("Coming of Age", "Coming of Age"),
        FilterOption::new("Character Growth", "Character Growth"),
        // Technology and science
        FilterOption::new("Scientists", "Scientists"),
        FilterOption::new("Artificial Intelligence", "Artificial Intelligence"),
        FilterOption::new("Genetic Modifications", "Genetic Modifications"),
        FilterOption::new("Hackers", "Hackers"),
        FilterOption::new("Zombies", "Zombies"),
        FilterOption::new("Survival", "Survival"),
        FilterOption::new("Dystopia", "Dystopia"),
        FilterOption::new("Utopia", "Utopia"),
        // Miscellaneous
        FilterOption::new("Cooking", "Cooking"),
        FilterOption::new("Music", "Music"),
        FilterOption::new("Artists", "Artists"),
        FilterOption::new("Writers", "Writers"),
        FilterOption::new("Gamers", "Gamers"),
        FilterOption::new(
            "Transported to Another World",
            "Transported to Another World",
        ),
        FilterOption::new("Second Chance", "Second Chance"),
        FilterOption::new("Time Travel", "Time Travel"),
        FilterOption::new("Parallel Worlds", "Parallel Worlds"),
        FilterOption::new("Dimensional Travel", "Dimensional Travel"),
    ]
}
