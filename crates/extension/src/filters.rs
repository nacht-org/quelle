//! Filter utilities for creating search filters more easily and with better type safety.
//!
//! This module provides builders and utilities for creating various types of search filters,
//! including the new tristate filters that allow users to specify include/exclude/ignore
//! preferences for multi-option filters like genres, tags, and content warnings.
//!
//! # Tristate Filters
//!
//! Tristate filters are a new type of filter that allows users to specify three states
//! for each option:
//! - **Must Include**: The option must be present in the search results
//! - **Must Exclude**: The option must not be present in the search results
//! - **Don't Care**: The option is ignored in the search
//!
//! This is particularly useful for complex filtering scenarios like genre selection,
//! where users might want to include some genres, exclude others, and ignore the rest.
//!
//! ## Example
//!
//! ```rust
//! use quelle_extension::prelude::*;
//!
//! // Create a tristate filter for genres
//! let genres = vec![
//!     FilterOption::new("1", "Fantasy"),
//!     FilterOption::new("2", "Romance"),
//!     FilterOption::new("3", "Horror"),
//! ];
//!
//! let filter = FilterBuilder::new("genres", "Genres")
//!     .description("Select genre preferences")
//!     .tri_state(genres);
//! ```
//!
//! The client can then send filter values like:
//! ```rust
//! use quelle_extension::novel::{FilterValue, TriState};
//!
//! let filter_value = FilterValue::TriState(vec![
//!     ("1".to_string(), TriState::MustInclude),  // Must have Fantasy
//!     ("2".to_string(), TriState::MustExclude),  // Must not have Romance
//!     ("3".to_string(), TriState::DontCare),     // Ignore Horror
//! ]);
//! ```

use crate::source::{
    BooleanFilter, DateRangeFilter, FilterDefinition, FilterOption, FilterType, MultiSelectFilter,
    NumberRangeFilter, SelectFilter, SortOption, SortOrder, TextFilter, TriStateFilter,
};

/// Tristate enum for filters that can be included, excluded, or ignored
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriState {
    /// Include this option (must have)
    Include,
    /// Exclude this option (must not have)
    Exclude,
    /// Don't care about this option
    Ignore,
}

impl TriState {
    /// Convert tristate to a string value for use in filter values
    pub fn to_filter_value(&self) -> &'static str {
        match self {
            TriState::Include => "include",
            TriState::Exclude => "exclude",
            TriState::Ignore => "ignore",
        }
    }

    /// Parse a string value into a tristate
    pub fn from_filter_value(value: &str) -> Option<Self> {
        match value {
            "include" => Some(TriState::Include),
            "exclude" => Some(TriState::Exclude),
            "ignore" => Some(TriState::Ignore),
            _ => None,
        }
    }
}

impl FilterOption {
    /// Create a new FilterOption with just value and label
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: None,
        }
    }

    /// Create a new FilterOption with value, label, and description
    pub fn with_description(
        value: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: Some(description.into()),
        }
    }

    /// Create tristate options for a multi-select filter (include/exclude/ignore)
    pub fn tristate_options() -> Vec<FilterOption> {
        vec![
            FilterOption::new("include", "Include (must have)"),
            FilterOption::new("exclude", "Exclude (must not have)"),
            FilterOption::new("ignore", "Don't care"),
        ]
    }

    /// Create boolean-like options (yes/no)
    pub fn boolean_options() -> Vec<FilterOption> {
        vec![
            FilterOption::new("true", "Yes"),
            FilterOption::new("false", "No"),
        ]
    }

    /// Create AND/OR logic options for multi-select filters
    pub fn logic_options() -> Vec<FilterOption> {
        vec![
            FilterOption::new("and", "AND (must have all selected)"),
            FilterOption::new("or", "OR (must have any selected)"),
        ]
    }
}

/// Builder for creating FilterDefinition instances more easily
pub struct FilterBuilder {
    id: String,
    name: String,
    description: Option<String>,
    required: bool,
}

impl FilterBuilder {
    /// Create a new filter builder
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            required: false,
        }
    }

    /// Set the description for this filter
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Mark this filter as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Build a text filter
    pub fn text(self) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::Text(TextFilter {
                placeholder: None,
                max_length: None,
            }),
            required: self.required,
        }
    }

    /// Build a text filter with placeholder and max length
    pub fn text_with_options(
        self,
        placeholder: Option<impl Into<String>>,
        max_length: Option<u32>,
    ) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::Text(TextFilter {
                placeholder: placeholder.map(|p| p.into()),
                max_length,
            }),
            required: self.required,
        }
    }

    /// Build a select filter
    pub fn select(self, options: Vec<FilterOption>) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::Select(SelectFilter { options }),
            required: self.required,
        }
    }

    /// Build a multi-select filter
    pub fn multi_select(self, options: Vec<FilterOption>) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::MultiSelect(MultiSelectFilter {
                options,
                max_selections: None,
            }),
            required: self.required,
        }
    }

    /// Build a multi-select filter with max selections limit
    pub fn multi_select_with_limit(
        self,
        options: Vec<FilterOption>,
        max_selections: u32,
    ) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::MultiSelect(MultiSelectFilter {
                options,
                max_selections: Some(max_selections),
            }),
            required: self.required,
        }
    }

    /// Build a number range filter
    pub fn number_range(
        self,
        min: f64,
        max: f64,
        step: Option<f64>,
        unit: Option<impl Into<String>>,
    ) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::NumberRange(NumberRangeFilter {
                min,
                max,
                step,
                unit: unit.map(|u| u.into()),
            }),
            required: self.required,
        }
    }

    /// Build a date range filter
    pub fn date_range(
        self,
        format: impl Into<String>,
        min_date: Option<impl Into<String>>,
        max_date: Option<impl Into<String>>,
    ) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::DateRange(DateRangeFilter {
                min_date: min_date.map(|d| d.into()),
                max_date: max_date.map(|d| d.into()),
                format: format.into(),
            }),
            required: self.required,
        }
    }

    /// Build a boolean filter
    pub fn boolean(
        self,
        default_value: Option<bool>,
        true_label: Option<impl Into<String>>,
        false_label: Option<impl Into<String>>,
    ) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::Boolean(BooleanFilter {
                default_value,
                true_label: true_label.map(|l| l.into()),
                false_label: false_label.map(|l| l.into()),
            }),
            required: self.required,
        }
    }

    /// Build a tri-state filter that allows include/exclude/ignore for each option
    ///
    /// Tristate filters are ideal for complex filtering scenarios where users need
    /// fine-grained control over multiple options. Each option can be set to:
    /// - Must Include: Results must contain this option
    /// - Must Exclude: Results must not contain this option
    /// - Don't Care: This option is ignored in filtering
    ///
    /// # Example
    /// ```rust
    /// use quelle_extension::prelude::*;
    ///
    /// let genres = vec![
    ///     FilterOption::new("fantasy", "Fantasy"),
    ///     FilterOption::new("romance", "Romance"),
    ///     FilterOption::new("horror", "Horror"),
    /// ];
    ///
    /// let filter = FilterBuilder::new("genres", "Genres")
    ///     .description("Genre preferences")
    ///     .tri_state(genres);
    /// ```
    pub fn tri_state(self, options: Vec<FilterOption>) -> FilterDefinition {
        FilterDefinition {
            id: self.id,
            name: self.name,
            description: self.description,
            filter_type: FilterType::TriState(TriStateFilter { options }),
            required: self.required,
        }
    }
}

/// Builder for creating SortOption instances more easily
pub struct SortOptionBuilder {
    id: String,
    name: String,
    description: Option<String>,
    supports_asc: bool,
    supports_desc: bool,
    default_order: Option<SortOrder>,
}

impl SortOptionBuilder {
    /// Create a new sort option builder
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            supports_asc: true,
            supports_desc: true,
            default_order: None,
        }
    }

    /// Set the description for this sort option
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set whether ascending order is supported
    pub fn supports_asc(mut self, supports: bool) -> Self {
        self.supports_asc = supports;
        self
    }

    /// Set whether descending order is supported
    pub fn supports_desc(mut self, supports: bool) -> Self {
        self.supports_desc = supports;
        self
    }

    /// Set the default sort order
    pub fn default_order(mut self, order: SortOrder) -> Self {
        self.default_order = Some(order);
        self
    }

    /// Build the sort option
    pub fn build(self) -> SortOption {
        SortOption {
            id: self.id,
            name: self.name,
            description: self.description,
            supports_asc: self.supports_asc,
            supports_desc: self.supports_desc,
            default_order: self.default_order,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_option_new() {
        let option = FilterOption::new("test_value", "Test Label");
        assert_eq!(option.value, "test_value");
        assert_eq!(option.label, "Test Label");
        assert_eq!(option.description, None);
    }

    #[test]
    fn test_filter_option_with_description() {
        let option = FilterOption::with_description("test_value", "Test Label", "Test Description");
        assert_eq!(option.value, "test_value");
        assert_eq!(option.label, "Test Label");
        assert_eq!(option.description, Some("Test Description".to_string()));
    }

    #[test]
    fn test_tristate_conversion() {
        assert_eq!(TriState::Include.to_filter_value(), "include");
        assert_eq!(TriState::Exclude.to_filter_value(), "exclude");
        assert_eq!(TriState::Ignore.to_filter_value(), "ignore");

        assert_eq!(
            TriState::from_filter_value("include"),
            Some(TriState::Include)
        );
        assert_eq!(
            TriState::from_filter_value("exclude"),
            Some(TriState::Exclude)
        );
        assert_eq!(
            TriState::from_filter_value("ignore"),
            Some(TriState::Ignore)
        );
        assert_eq!(TriState::from_filter_value("invalid"), None);
    }

    #[test]
    fn test_filter_builder() {
        let filter = FilterBuilder::new("test_id", "Test Name")
            .description("Test Description")
            .required()
            .text();

        assert_eq!(filter.id, "test_id");
        assert_eq!(filter.name, "Test Name");
        assert_eq!(filter.description, Some("Test Description".to_string()));
        assert_eq!(filter.required, true);
    }

    #[test]
    fn test_sort_option_builder() {
        let sort_option = SortOptionBuilder::new("test_sort", "Test Sort")
            .description("Test sort description")
            .supports_asc(false)
            .default_order(SortOrder::Desc)
            .build();

        assert_eq!(sort_option.id, "test_sort");
        assert_eq!(sort_option.name, "Test Sort");
        assert_eq!(
            sort_option.description,
            Some("Test sort description".to_string())
        );
        assert_eq!(sort_option.supports_asc, false);
        assert_eq!(sort_option.supports_desc, true);
        assert_eq!(sort_option.default_order, Some(SortOrder::Desc));
    }
}
