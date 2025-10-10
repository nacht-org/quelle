//! Generic validation module for search filter values.
//!
//! This module provides reusable validation logic that can be used by any extension
//! to validate filter values against their definitions and custom business rules.

use crate::novel::FilterValue;
use crate::source::FilterType;

/// Validation error types for filter values
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid filter type: expected {expected}, got {actual}")]
    InvalidType { expected: String, actual: String },

    #[error("Value out of range: {value} not in range [{min}, {max}]")]
    OutOfRange { value: f64, min: f64, max: f64 },

    #[error("Text too long: {length} characters (max {max})")]
    TextTooLong { length: usize, max: usize },

    #[error("Invalid option value: {value} not in allowed options")]
    InvalidOption { value: String },

    #[error("Too many selections: {count} selected (max {max})")]
    TooManySelections { count: usize, max: usize },

    #[error("Invalid date format: {value} (expected format: {format})")]
    InvalidDateFormat { value: String, format: String },

    #[error("Missing required filter value")]
    MissingValue,

    #[error("Custom validation error: {message}")]
    Custom { message: String },
}

/// Generic validator for filter values using filter definitions as source of truth
pub struct FilterValidator;

impl FilterValidator {
    /// Validate a filter value against a filter definition
    pub fn validate_filter(
        filter_definition: &crate::source::FilterDefinition,
        value: &FilterValue,
    ) -> Result<(), ValidationError> {
        // Check if required filter has a value
        if filter_definition.required {
            match value {
                FilterValue::Text(text) if text.trim().is_empty() => {
                    return Err(ValidationError::MissingValue);
                }
                FilterValue::MultiSelect(selections) if selections.is_empty() => {
                    return Err(ValidationError::MissingValue);
                }
                FilterValue::TriState(selections) if selections.is_empty() => {
                    return Err(ValidationError::MissingValue);
                }
                _ => {}
            }
        }

        // Validate against filter type definition
        Self::validate_against_type(&filter_definition.filter_type, value)
    }

    /// Validate a filter value against its type definition
    pub fn validate_against_type(
        filter_type: &FilterType,
        value: &FilterValue,
    ) -> Result<(), ValidationError> {
        match (filter_type, value) {
            // Text filter validation
            (FilterType::Text(text_filter), FilterValue::Text(text)) => {
                if let Some(max_length) = text_filter.max_length {
                    if text.len() > max_length as usize {
                        return Err(ValidationError::TextTooLong {
                            length: text.len(),
                            max: max_length as usize,
                        });
                    }
                }
                Ok(())
            }

            // Select filter validation
            (FilterType::Select(select_filter), FilterValue::Select(selected)) => {
                Self::validate_option_against_list(selected, &select_filter.options)
            }

            // Multi-select filter validation
            (FilterType::MultiSelect(multi_filter), FilterValue::MultiSelect(selected)) => {
                // Check max selections limit
                if let Some(max_selections) = multi_filter.max_selections {
                    if selected.len() > max_selections as usize {
                        return Err(ValidationError::TooManySelections {
                            count: selected.len(),
                            max: max_selections as usize,
                        });
                    }
                }

                // Check all selected values are valid options
                for selection in selected {
                    Self::validate_option_against_list(selection, &multi_filter.options)?;
                }
                Ok(())
            }

            // TriState filter validation
            (FilterType::TriState(tristate_filter), FilterValue::TriState(tristate_values)) => {
                for (option_id, _state) in tristate_values {
                    Self::validate_option_against_list(option_id, &tristate_filter.options)?;
                }
                Ok(())
            }

            // Number range filter validation
            (FilterType::NumberRange(range_filter), FilterValue::NumberRange(range)) => {
                if let Some(min_val) = range.min {
                    if min_val < range_filter.min || min_val > range_filter.max {
                        return Err(ValidationError::OutOfRange {
                            value: min_val,
                            min: range_filter.min,
                            max: range_filter.max,
                        });
                    }
                }

                if let Some(max_val) = range.max {
                    if max_val < range_filter.min || max_val > range_filter.max {
                        return Err(ValidationError::OutOfRange {
                            value: max_val,
                            min: range_filter.min,
                            max: range_filter.max,
                        });
                    }
                }

                // Ensure min <= max if both are provided
                if let (Some(min_val), Some(max_val)) = (range.min, range.max) {
                    if min_val > max_val {
                        return Err(ValidationError::Custom {
                            message: format!(
                                "Min value {} cannot be greater than max value {}",
                                min_val, max_val
                            ),
                        });
                    }
                }
                Ok(())
            }

            // Date range filter validation
            (FilterType::DateRange(date_filter), FilterValue::DateRange(date_range)) => {
                Self::validate_date_range(date_filter, date_range)
            }

            // Boolean filter validation (always valid)
            (FilterType::Boolean(_), FilterValue::Boolean(_)) => Ok(()),

            // Type mismatch
            _ => Err(ValidationError::InvalidType {
                expected: Self::filter_type_name(filter_type),
                actual: Self::filter_value_type_name(value),
            }),
        }
    }

    /// Validate an option against a list of valid filter options
    fn validate_option_against_list(
        value: &str,
        options: &[crate::source::FilterOption],
    ) -> Result<(), ValidationError> {
        let valid_values: Vec<&str> = options.iter().map(|opt| opt.value.as_str()).collect();
        if !valid_values.contains(&value) {
            return Err(ValidationError::InvalidOption {
                value: value.to_string(),
            });
        }
        Ok(())
    }

    /// Validate date range using filter definition
    fn validate_date_range(
        date_filter: &crate::source::DateRangeFilter,
        date_range: &crate::novel::DateRangeValue,
    ) -> Result<(), ValidationError> {
        let validate_date_format = |date_str: &str| -> bool {
            // Simple YYYY-MM-DD validation based on filter format
            if date_filter.format == "YYYY-MM-DD" {
                Self::validate_yyyy_mm_dd_format(date_str)
            } else {
                // For other formats, just check it's not empty
                !date_str.is_empty()
            }
        };

        if let Some(start) = &date_range.start {
            if !validate_date_format(start) {
                return Err(ValidationError::InvalidDateFormat {
                    value: start.clone(),
                    format: date_filter.format.clone(),
                });
            }
        }

        if let Some(end) = &date_range.end {
            if !validate_date_format(end) {
                return Err(ValidationError::InvalidDateFormat {
                    value: end.clone(),
                    format: date_filter.format.clone(),
                });
            }
        }

        // Validate date boundaries if specified in filter definition
        if let (Some(min_date), Some(start)) = (&date_filter.min_date, &date_range.start) {
            if start < min_date {
                return Err(ValidationError::Custom {
                    message: format!(
                        "Start date {} is before minimum allowed date {}",
                        start, min_date
                    ),
                });
            }
        }

        if let (Some(max_date), Some(end)) = (&date_filter.max_date, &date_range.end) {
            if end > max_date {
                return Err(ValidationError::Custom {
                    message: format!(
                        "End date {} is after maximum allowed date {}",
                        end, max_date
                    ),
                });
            }
        }

        // Validate start <= end if both provided
        if let (Some(start), Some(end)) = (&date_range.start, &date_range.end) {
            if start > end {
                return Err(ValidationError::Custom {
                    message: format!("Start date {} cannot be after end date {}", start, end),
                });
            }
        }

        Ok(())
    }

    /// Validate YYYY-MM-DD date format
    fn validate_yyyy_mm_dd_format(date_str: &str) -> bool {
        if date_str.len() != 10 {
            return false;
        }

        let parts: Vec<&str> = date_str.split('-').collect();
        if parts.len() != 3 {
            return false;
        }

        // Check year (4 digits)
        if parts[0].len() != 4 || !parts[0].chars().all(|c| c.is_ascii_digit()) {
            return false;
        }

        // Check month (2 digits, 01-12)
        if parts[1].len() != 2 || !parts[1].chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if let Ok(month) = parts[1].parse::<u32>() {
            if month < 1 || month > 12 {
                return false;
            }
        } else {
            return false;
        }

        // Check day (2 digits, 01-31)
        if parts[2].len() != 2 || !parts[2].chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if let Ok(day) = parts[2].parse::<u32>() {
            if day < 1 || day > 31 {
                return false;
            }
        } else {
            return false;
        }

        true
    }

    /// Get human-readable filter type name
    fn filter_type_name(filter_type: &FilterType) -> String {
        match filter_type {
            FilterType::Text(_) => "text".to_string(),
            FilterType::Select(_) => "select".to_string(),
            FilterType::MultiSelect(_) => "multi-select".to_string(),
            FilterType::TriState(_) => "tri-state".to_string(),
            FilterType::NumberRange(_) => "number-range".to_string(),
            FilterType::DateRange(_) => "date-range".to_string(),
            FilterType::Boolean(_) => "boolean".to_string(),
        }
    }

    /// Get human-readable filter value type name
    fn filter_value_type_name(value: &FilterValue) -> String {
        match value {
            FilterValue::Text(_) => "text".to_string(),
            FilterValue::Select(_) => "select".to_string(),
            FilterValue::MultiSelect(_) => "multi-select".to_string(),
            FilterValue::TriState(_) => "tri-state".to_string(),
            FilterValue::NumberRange(_) => "number-range".to_string(),
            FilterValue::DateRange(_) => "date-range".to_string(),
            FilterValue::Boolean(_) => "boolean".to_string(),
        }
    }

    /// Helper method to validate text length
    pub fn validate_text_length(text: &str, max_length: usize) -> Result<(), ValidationError> {
        if text.len() > max_length {
            Err(ValidationError::TextTooLong {
                length: text.len(),
                max: max_length,
            })
        } else {
            Ok(())
        }
    }

    /// Helper method to validate numeric range
    pub fn validate_numeric_range(value: f64, min: f64, max: f64) -> Result<(), ValidationError> {
        if value < min || value > max {
            Err(ValidationError::OutOfRange { value, min, max })
        } else {
            Ok(())
        }
    }

    /// Helper method to validate option against allowed values
    pub fn validate_option(value: &str, allowed: &[&str]) -> Result<(), ValidationError> {
        if allowed.contains(&value) {
            Ok(())
        } else {
            Err(ValidationError::InvalidOption {
                value: value.to_string(),
            })
        }
    }
}

/// Simple validation function that extensions can call to get validated data back
///
/// This is the main entry point for filter validation. Extensions just need to:
/// 1. Get their filter definitions (single source of truth)
/// 2. Pass the definitions and user input to this function
/// 3. Get back validated data or an error
///
/// # Example
/// ```ignore
/// let definitions = create_filter_definitions();
/// let validated_filters = validate_filters(&definitions, &query.filters)?;
/// // validated_filters is guaranteed to be valid - safe to process
/// ```
pub fn validate_filters(
    definitions: &[crate::source::FilterDefinition],
    applied_filters: &[crate::novel::AppliedFilter],
) -> Result<Vec<crate::novel::AppliedFilter>, ValidationError> {
    use std::collections::HashMap;

    // Create a map for quick lookup of filter definitions
    let definition_map: HashMap<&str, &crate::source::FilterDefinition> = definitions
        .iter()
        .map(|def| (def.id.as_str(), def))
        .collect();

    // Validate each applied filter
    for filter in applied_filters {
        // Check if filter ID exists in definitions
        let definition = definition_map
            .get(filter.filter_id.as_str())
            .ok_or_else(|| ValidationError::Custom {
                message: format!("Unknown filter ID: {}", filter.filter_id),
            })?;

        // Validate the filter value against its definition
        FilterValidator::validate_filter(definition, &filter.value)?;
    }

    // If all validation passes, return the original data
    // The caller now knows this data is validated and safe to use
    Ok(applied_filters.to_vec())
}

/// Advanced validation function with custom business rules
///
/// This allows extensions to provide additional validation logic beyond
/// what's captured in the filter definitions.
///
/// # Example
/// ```ignore
/// let definitions = create_filter_definitions();
/// let validated_filters = validate_filters_with_business_rules(
///     &definitions,
///     &query.filters,
///     |filter_id, value| {
///         // Custom business rules for this extension
///         match filter_id {
///             "ratings" => {
///                 if let FilterValue::NumberRange(range) = value {
///                     if let Some(max) = range.max {
///                         if max > 5.0 {
///                             return Err(ValidationError::Custom {
///                                 message: "Ratings must be 0-5".to_string(),
///                             });
///                         }
///                     }
///                 }
///                 Ok(())
///             }
///             _ => Ok(()),
///         }
///     },
/// )?;
/// ```
pub fn validate_filters_with_business_rules<F>(
    definitions: &[crate::source::FilterDefinition],
    applied_filters: &[crate::novel::AppliedFilter],
    business_rule_validator: F,
) -> Result<Vec<crate::novel::AppliedFilter>, ValidationError>
where
    F: Fn(&str, &crate::novel::FilterValue) -> Result<(), ValidationError>,
{
    use std::collections::HashMap;

    // Create a map for quick lookup of filter definitions
    let definition_map: HashMap<&str, &crate::source::FilterDefinition> = definitions
        .iter()
        .map(|def| (def.id.as_str(), def))
        .collect();

    // Validate each applied filter
    for filter in applied_filters {
        // Check if filter ID exists in definitions
        let definition = definition_map
            .get(filter.filter_id.as_str())
            .ok_or_else(|| ValidationError::Custom {
                message: format!("Unknown filter ID: {}", filter.filter_id),
            })?;

        // First, validate against filter definition (generic validation)
        FilterValidator::validate_filter(definition, &filter.value)?;

        // Then, apply extension-specific business rules
        business_rule_validator(&filter.filter_id, &filter.value)?;
    }

    // If all validation passes, return the original data
    Ok(applied_filters.to_vec())
}

/// Validated search parameters with fluent form mapping capabilities
#[derive(Debug, Clone)]
pub struct ValidatedSearchParams {
    pub filters: Vec<crate::novel::AppliedFilter>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<String>,
    pub sort_order: Option<crate::novel::SortOrder>,
}

impl ValidatedSearchParams {
    /// Start building a form
    pub fn into_form(self) -> FormBuilder {
        FormBuilder::new(self)
    }

    /// Get filter by ID
    pub fn get_filter_by_id(&self, filter_id: &str) -> Option<&crate::novel::AppliedFilter> {
        self.filters.iter().find(|f| f.filter_id == filter_id)
    }
}

/// Form builder for mapping validated search params
pub struct FormBuilder {
    params: ValidatedSearchParams,
    mappings: std::collections::HashMap<String, String>,
    form_data: std::collections::HashMap<String, Vec<String>>,
}

impl FormBuilder {
    fn new(params: ValidatedSearchParams) -> Self {
        Self {
            params,
            mappings: std::collections::HashMap::new(),
            form_data: std::collections::HashMap::new(),
        }
    }

    /// Map a filter to a form field
    pub fn with_mapping<T: AsRef<str>>(mut self, filter_id: T, form_field: &str) -> Self {
        let filter_id_str = filter_id.as_ref();
        self.mappings
            .insert(filter_id_str.to_string(), form_field.to_string());

        // Process the filter if it exists - automatically handle all types
        let filter_data = self
            .params
            .get_filter_by_id(filter_id_str)
            .map(|f| f.value.clone());

        if let Some(filter_value) = filter_data {
            self.process_filter_value(&filter_value, form_field);
        }

        self
    }

    /// Map a number range filter with custom min/max field names
    pub fn with_mapping_range<T: AsRef<str>>(
        mut self,
        filter_id: T,
        min_field: &str,
        max_field: &str,
    ) -> Self {
        let filter_data = self
            .params
            .get_filter_by_id(filter_id.as_ref())
            .map(|f| f.value.clone());
        if let Some(crate::novel::FilterValue::NumberRange(range)) = filter_data {
            if let Some(min) = range.min {
                self.form_data
                    .entry(min_field.to_string())
                    .or_default()
                    .push(min.to_string());
            }
            if let Some(max) = range.max {
                self.form_data
                    .entry(max_field.to_string())
                    .or_default()
                    .push(max.to_string());
            }
        }
        self
    }

    /// Map a date range filter with custom field names
    pub fn with_mapping_date_range<T: AsRef<str>>(
        mut self,
        filter_id: T,
        start_field: &str,
        end_field: &str,
    ) -> Self {
        let filter_data = self
            .params
            .get_filter_by_id(filter_id.as_ref())
            .map(|f| f.value.clone());
        if let Some(crate::novel::FilterValue::DateRange(range)) = filter_data {
            if let Some(start) = &range.start {
                self.form_data
                    .entry(start_field.to_string())
                    .or_default()
                    .push(start.clone());
            }
            if let Some(end) = &range.end {
                self.form_data
                    .entry(end_field.to_string())
                    .or_default()
                    .push(end.clone());
            }
        }
        self
    }

    /// Map a tristate filter with custom include/exclude field names
    pub fn with_mapping_tristate<T: AsRef<str>>(
        mut self,
        filter_id: T,
        include_field: &str,
        exclude_field: &str,
    ) -> Self {
        let filter_data = self
            .params
            .get_filter_by_id(filter_id.as_ref())
            .map(|f| f.value.clone());

        if let Some(crate::novel::FilterValue::TriState(tristate_values)) = filter_data {
            use crate::novel::TriState;
            for (option_id, state) in tristate_values {
                match state {
                    TriState::MustInclude => {
                        let field_name = format!("{}[]", include_field);
                        self.form_data
                            .entry(field_name)
                            .or_default()
                            .push(option_id.clone());
                    }
                    TriState::MustExclude => {
                        let field_name = format!("{}[]", exclude_field);
                        self.form_data
                            .entry(field_name)
                            .or_default()
                            .push(option_id.clone());
                    }
                    TriState::DontCare => {}
                }
            }
        }
        self
    }

    /// Add pagination field
    pub fn with_pagination(mut self, page_field: &str) -> Self {
        if let Some(page) = self.params.page {
            self.form_data
                .entry(page_field.to_string())
                .or_default()
                .push(page.to_string());
        }
        self
    }

    /// Add sorting fields
    pub fn with_sort(mut self, sort_field: &str, order_field: &str) -> Self {
        if let Some(sort_by) = &self.params.sort_by {
            self.form_data
                .entry(sort_field.to_string())
                .or_default()
                .push(sort_by.clone());
        }

        if let Some(sort_order) = &self.params.sort_order {
            let order_str = match sort_order {
                crate::novel::SortOrder::Asc => "asc",
                crate::novel::SortOrder::Desc => "desc",
            };
            self.form_data
                .entry(order_field.to_string())
                .or_default()
                .push(order_str.to_string());
        }

        self
    }

    /// Add default sort value
    pub fn with_default_sort(mut self, sort_field: &str, default_sort: &str) -> Self {
        if self.params.sort_by.is_none() {
            self.form_data
                .entry(sort_field.to_string())
                .or_default()
                .push(default_sort.to_string());
        }
        self
    }

    /// Add custom form field
    pub fn with_custom_field(mut self, field_name: &str, value: &str) -> Self {
        self.form_data
            .entry(field_name.to_string())
            .or_default()
            .push(value.to_string());
        self
    }

    /// Build RequestFormBuilder
    pub fn build(self) -> crate::RequestFormBuilder {
        let mut form_builder = crate::RequestFormBuilder::new();

        for (field_name, values) in self.form_data {
            for value in values {
                form_builder = form_builder.param(field_name.clone(), value);
            }
        }

        form_builder
    }

    fn process_filter_value(&mut self, filter_value: &crate::novel::FilterValue, form_field: &str) {
        use crate::novel::{FilterValue, TriState};

        match filter_value {
            FilterValue::Text(text) => {
                self.form_data
                    .entry(form_field.to_string())
                    .or_default()
                    .push(text.clone());
            }
            FilterValue::Select(value) => {
                self.form_data
                    .entry(form_field.to_string())
                    .or_default()
                    .push(value.clone());
            }
            FilterValue::MultiSelect(values) => {
                for value in values {
                    self.form_data
                        .entry(form_field.to_string())
                        .or_default()
                        .push(value.clone());
                }
            }
            FilterValue::Boolean(val) => {
                self.form_data
                    .entry(form_field.to_string())
                    .or_default()
                    .push(val.to_string());
            }
            FilterValue::NumberRange(range) => {
                if let Some(min) = range.min {
                    let min_field = format!("min_{}", form_field);
                    self.form_data
                        .entry(min_field)
                        .or_default()
                        .push(min.to_string());
                }
                if let Some(max) = range.max {
                    let max_field = format!("max_{}", form_field);
                    self.form_data
                        .entry(max_field)
                        .or_default()
                        .push(max.to_string());
                }
            }
            FilterValue::DateRange(range) => {
                if let Some(start) = &range.start {
                    let start_field = format!("{}_min", form_field);
                    self.form_data
                        .entry(start_field)
                        .or_default()
                        .push(start.clone());
                }
                if let Some(end) = &range.end {
                    let end_field = format!("{}_max", form_field);
                    self.form_data
                        .entry(end_field)
                        .or_default()
                        .push(end.clone());
                }
            }
            FilterValue::TriState(tristate_values) => {
                for (option_id, state) in tristate_values {
                    match state {
                        TriState::MustInclude => {
                            let field_name = format!("{}[]", form_field);
                            self.form_data
                                .entry(field_name)
                                .or_default()
                                .push(option_id.clone());
                        }
                        TriState::MustExclude => {
                            let field_name = format!("{}_excluded[]", form_field);
                            self.form_data
                                .entry(field_name)
                                .or_default()
                                .push(option_id.clone());
                        }
                        TriState::DontCare => {}
                    }
                }
            }
        }
    }
}

/// Validate a complete search query including filters, pagination, and sorting
pub fn validate_search_query(
    definitions: &[crate::source::FilterDefinition],
    sort_options: &[crate::source::SortOption],
    query: &crate::novel::ComplexSearchQuery,
) -> Result<ValidatedSearchParams, ValidationError> {
    // Validate filters
    let validated_filters = validate_filters(definitions, &query.filters)?;

    // Validate pagination
    if let Some(page) = query.page {
        if page < 1 {
            return Err(ValidationError::Custom {
                message: format!("Page number must be at least 1, got {}", page),
            });
        }
        if page > 10000 {
            return Err(ValidationError::Custom {
                message: format!("Page number too high: {} (max 10000)", page),
            });
        }
    }

    if let Some(limit) = query.limit {
        if limit < 1 {
            return Err(ValidationError::Custom {
                message: format!("Limit must be at least 1, got {}", limit),
            });
        }
        if limit > 100 {
            return Err(ValidationError::Custom {
                message: format!("Limit too high: {} (max 100)", limit),
            });
        }
    }

    // Validate sort options
    if let Some(sort_by) = &query.sort_by {
        let valid_sort_ids: Vec<&str> = sort_options.iter().map(|opt| opt.id.as_str()).collect();
        if !valid_sort_ids.contains(&sort_by.as_str()) {
            return Err(ValidationError::Custom {
                message: format!("Invalid sort option: {}", sort_by),
            });
        }
    }

    // Return validated search params
    Ok(ValidatedSearchParams {
        filters: validated_filters,
        page: query.page,
        limit: query.limit,
        sort_by: query.sort_by.clone(),
        sort_order: query.sort_order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::novel::{
        AppliedFilter, ComplexSearchQuery, DateRangeValue, FilterValue, NumberRangeValue,
        SortOrder, TriState,
    };
    use crate::source::{
        FilterDefinition, FilterOption, FilterType, NumberRangeFilter, SelectFilter, SortOption,
        TextFilter, TriStateFilter,
    };

    // Helper function to create test filter definitions
    fn create_test_definitions() -> Vec<FilterDefinition> {
        vec![
            FilterDefinition {
                id: "genres".to_string(),
                name: "Genres".to_string(),
                description: None,
                filter_type: FilterType::TriState(TriStateFilter {
                    options: vec![
                        FilterOption::new("fantasy", "Fantasy"),
                        FilterOption::new("romance", "Romance"),
                        FilterOption::new("sci-fi", "Science Fiction"),
                    ],
                }),
                required: false,
            },
            FilterDefinition {
                id: "chapters".to_string(),
                name: "Chapter Count".to_string(),
                description: None,
                filter_type: FilterType::NumberRange(NumberRangeFilter {
                    min: 0.0,
                    max: 10000.0,
                    step: Some(1.0),
                    unit: Some("chapters".to_string()),
                }),
                required: false,
            },
            FilterDefinition {
                id: "title".to_string(),
                name: "Title".to_string(),
                description: None,
                filter_type: FilterType::Text(TextFilter {
                    placeholder: Some("Enter title...".to_string()),
                    max_length: Some(100),
                }),
                required: false,
            },
        ]
    }

    fn create_test_sort_options() -> Vec<SortOption> {
        vec![
            SortOption {
                id: "pageviews".to_string(),
                name: "Page Views".to_string(),
                description: None,
                supports_asc: true,
                supports_desc: true,
                default_order: Some(crate::source::SortOrder::Desc),
            },
            SortOption {
                id: "favorites".to_string(),
                name: "Favorites".to_string(),
                description: None,
                supports_asc: true,
                supports_desc: true,
                default_order: Some(crate::source::SortOrder::Desc),
            },
        ]
    }

    #[test]
    fn test_validate_filters_success() {
        let definitions = create_test_definitions();
        let applied_filters = vec![
            AppliedFilter {
                filter_id: "genres".to_string(),
                value: FilterValue::TriState(vec![
                    ("fantasy".to_string(), TriState::MustInclude),
                    ("romance".to_string(), TriState::MustExclude),
                ]),
            },
            AppliedFilter {
                filter_id: "chapters".to_string(),
                value: FilterValue::NumberRange(NumberRangeValue {
                    min: Some(10.0),
                    max: Some(500.0),
                }),
            },
        ];

        let result = validate_filters(&definitions, &applied_filters);
        assert!(result.is_ok(), "Valid filters should pass validation");

        let validated = result.unwrap();
        assert_eq!(validated.len(), 2);
        assert_eq!(validated[0].filter_id, "genres");
        assert_eq!(validated[1].filter_id, "chapters");
    }

    #[test]
    fn test_validate_filters_unknown_filter_id() {
        let definitions = create_test_definitions();
        let applied_filters = vec![AppliedFilter {
            filter_id: "unknown_filter".to_string(),
            value: FilterValue::Text("test".to_string()),
        }];

        let result = validate_filters(&definitions, &applied_filters);
        assert!(result.is_err(), "Unknown filter ID should fail validation");

        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Unknown filter ID: unknown_filter")
        );
    }

    #[test]
    fn test_validate_filters_invalid_tristate_option() {
        let definitions = create_test_definitions();
        let applied_filters = vec![AppliedFilter {
            filter_id: "genres".to_string(),
            value: FilterValue::TriState(vec![(
                "invalid_genre".to_string(),
                TriState::MustInclude,
            )]),
        }];

        let result = validate_filters(&definitions, &applied_filters);
        assert!(
            result.is_err(),
            "Invalid tristate option should fail validation"
        );
    }

    #[test]
    fn test_validate_filters_type_mismatch() {
        let definitions = create_test_definitions();
        let applied_filters = vec![AppliedFilter {
            filter_id: "genres".to_string(),
            value: FilterValue::Text("fantasy".to_string()), // Wrong type - should be TriState
        }];

        let result = validate_filters(&definitions, &applied_filters);
        assert!(result.is_err(), "Type mismatch should fail validation");
    }

    #[test]
    fn test_validate_filters_with_business_rules_success() {
        let definitions = create_test_definitions();
        let applied_filters = vec![AppliedFilter {
            filter_id: "chapters".to_string(),
            value: FilterValue::NumberRange(NumberRangeValue {
                min: Some(10.0),
                max: Some(50.0),
            }),
        }];

        let result = validate_filters_with_business_rules(
            &definitions,
            &applied_filters,
            |filter_id, value| {
                // Custom business rule: chapters max cannot exceed 100
                if filter_id == "chapters" {
                    if let FilterValue::NumberRange(range) = value {
                        if let Some(max) = range.max {
                            if max > 100.0 {
                                return Err(ValidationError::Custom {
                                    message: "Chapters max cannot exceed 100".to_string(),
                                });
                            }
                        }
                    }
                }
                Ok(())
            },
        );

        assert!(
            result.is_ok(),
            "Valid filters with business rules should pass"
        );
    }

    #[test]
    fn test_validate_filters_with_business_rules_failure() {
        let definitions = create_test_definitions();
        let applied_filters = vec![AppliedFilter {
            filter_id: "chapters".to_string(),
            value: FilterValue::NumberRange(NumberRangeValue {
                min: Some(10.0),
                max: Some(200.0), // Exceeds our business rule limit of 100
            }),
        }];

        let result = validate_filters_with_business_rules(
            &definitions,
            &applied_filters,
            |filter_id, value| {
                // Custom business rule: chapters max cannot exceed 100
                if filter_id == "chapters" {
                    if let FilterValue::NumberRange(range) = value {
                        if let Some(max) = range.max {
                            if max > 100.0 {
                                return Err(ValidationError::Custom {
                                    message: "Chapters max cannot exceed 100".to_string(),
                                });
                            }
                        }
                    }
                }
                Ok(())
            },
        );

        assert!(
            result.is_err(),
            "Business rule violation should fail validation"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Chapters max cannot exceed 100")
        );
    }

    #[test]
    fn test_validate_search_query_success() {
        let definitions = create_test_definitions();
        let sort_options = create_test_sort_options();
        let query = ComplexSearchQuery {
            filters: vec![
                AppliedFilter {
                    filter_id: "genres".to_string(),
                    value: FilterValue::TriState(vec![(
                        "fantasy".to_string(),
                        TriState::MustInclude,
                    )]),
                },
                AppliedFilter {
                    filter_id: "title".to_string(),
                    value: FilterValue::Text("test novel".to_string()),
                },
            ],
            page: Some(2),
            limit: Some(25),
            sort_by: Some("pageviews".to_string()),
            sort_order: Some(SortOrder::Desc),
        };

        let result = validate_search_query(&definitions, &sort_options, &query);
        assert!(result.is_ok(), "Valid search query should pass validation");

        let validated = result.unwrap();
        assert_eq!(validated.page, Some(2));
        assert_eq!(validated.limit, Some(25));
        assert_eq!(validated.sort_by, Some("pageviews".to_string()));
        assert_eq!(validated.filters.len(), 2);
    }

    #[test]
    fn test_validate_search_query_invalid_pagination() {
        let definitions = create_test_definitions();
        let sort_options = create_test_sort_options();

        // Invalid page number
        let query = ComplexSearchQuery {
            filters: vec![],
            page: Some(0), // Invalid - must be >= 1
            limit: Some(25),
            sort_by: None,
            sort_order: None,
        };

        let result = validate_search_query(&definitions, &sort_options, &query);
        assert!(
            result.is_err(),
            "Invalid page number should fail validation"
        );

        // Invalid limit
        let query2 = ComplexSearchQuery {
            filters: vec![],
            page: Some(1),
            limit: Some(0), // Invalid - must be >= 1
            sort_by: None,
            sort_order: None,
        };

        let result2 = validate_search_query(&definitions, &sort_options, &query2);
        assert!(result2.is_err(), "Invalid limit should fail validation");
    }

    #[test]
    fn test_validate_search_query_invalid_sort_option() {
        let definitions = create_test_definitions();
        let sort_options = create_test_sort_options();
        let query = ComplexSearchQuery {
            filters: vec![],
            page: Some(1),
            limit: Some(25),
            sort_by: Some("invalid_sort".to_string()), // Not in sort_options
            sort_order: Some(SortOrder::Desc),
        };

        let result = validate_search_query(&definitions, &sort_options, &query);
        assert!(
            result.is_err(),
            "Invalid sort option should fail validation"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid sort option: invalid_sort")
        );
    }

    #[test]
    fn test_simple_api_usage_example() {
        // This test demonstrates how simple it is for extensions to use the validation API

        // 1. Extension creates its filter definitions (single source of truth)
        let definitions = create_test_definitions();

        // 2. Extension receives user input
        let user_filters = vec![AppliedFilter {
            filter_id: "genres".to_string(),
            value: FilterValue::TriState(vec![
                ("fantasy".to_string(), TriState::MustInclude),
                ("sci-fi".to_string(), TriState::MustExclude),
            ]),
        }];

        // 3. Extension calls validate_filters and gets validated data back
        let validated_filters =
            validate_filters(&definitions, &user_filters).expect("This should be valid");

        // 4. Extension can now safely process the validated data
        assert_eq!(validated_filters.len(), 1);
        assert_eq!(validated_filters[0].filter_id, "genres");

        // The extension knows this data is guaranteed to be valid:
        // - All filter IDs exist in definitions
        // - All filter values match their expected types
        // - All tristate options are valid
        // - All constraints are satisfied

        match &validated_filters[0].value {
            FilterValue::TriState(tristate_values) => {
                // Safe to process - all options are guaranteed valid
                for (option_id, state) in tristate_values {
                    match state {
                        TriState::MustInclude => println!("Include: {}", option_id),
                        TriState::MustExclude => println!("Exclude: {}", option_id),
                        TriState::DontCare => println!("Ignore: {}", option_id),
                    }
                }
            }
            _ => panic!("Should be TriState based on our definition"),
        }
    }

    // Legacy FilterValidator tests for internal validation logic
    #[test]
    fn test_text_filter_validation() {
        let filter_type = FilterType::Text(TextFilter {
            placeholder: None,
            max_length: Some(10),
        });

        // Valid text
        let valid_value = FilterValue::Text("hello".to_string());
        assert!(FilterValidator::validate_against_type(&filter_type, &valid_value).is_ok());

        // Text too long
        let invalid_value = FilterValue::Text("this is too long".to_string());
        assert!(FilterValidator::validate_against_type(&filter_type, &invalid_value).is_err());
    }

    #[test]
    fn test_number_range_validation() {
        let filter_type = FilterType::NumberRange(NumberRangeFilter {
            min: 0.0,
            max: 100.0,
            step: Some(1.0),
            unit: None,
        });

        // Valid range
        let valid_value = FilterValue::NumberRange(NumberRangeValue {
            min: Some(10.0),
            max: Some(50.0),
        });
        assert!(FilterValidator::validate_against_type(&filter_type, &valid_value).is_ok());

        // Invalid range (out of bounds)
        let invalid_value = FilterValue::NumberRange(NumberRangeValue {
            min: Some(150.0),
            max: Some(200.0),
        });
        assert!(FilterValidator::validate_against_type(&filter_type, &invalid_value).is_err());
    }

    #[test]
    fn test_select_validation() {
        let filter_type = FilterType::Select(SelectFilter {
            options: vec![
                FilterOption::new("option1", "Option 1"),
                FilterOption::new("option2", "Option 2"),
            ],
        });

        // Valid selection
        let valid_value = FilterValue::Select("option1".to_string());
        assert!(FilterValidator::validate_against_type(&filter_type, &valid_value).is_ok());

        // Invalid selection
        let invalid_value = FilterValue::Select("invalid".to_string());
        assert!(FilterValidator::validate_against_type(&filter_type, &invalid_value).is_err());
    }

    #[test]
    fn test_tristate_validation() {
        let filter_type = FilterType::TriState(TriStateFilter {
            options: vec![
                FilterOption::new("fantasy", "Fantasy"),
                FilterOption::new("romance", "Romance"),
            ],
        });

        // Valid tristate
        let valid_value = FilterValue::TriState(vec![
            ("fantasy".to_string(), TriState::MustInclude),
            ("romance".to_string(), TriState::MustExclude),
        ]);
        assert!(FilterValidator::validate_against_type(&filter_type, &valid_value).is_ok());

        // Invalid option
        let invalid_value =
            FilterValue::TriState(vec![("horror".to_string(), TriState::MustInclude)]);
        assert!(FilterValidator::validate_against_type(&filter_type, &invalid_value).is_err());
    }

    #[test]
    fn test_date_validation() {
        // Valid dates
        assert!(
            FilterValidator::validate_against_type(
                &FilterType::DateRange(crate::source::DateRangeFilter {
                    min_date: None,
                    max_date: None,
                    format: "YYYY-MM-DD".to_string(),
                }),
                &FilterValue::DateRange(DateRangeValue {
                    start: Some("2023-01-15".to_string()),
                    end: Some("2023-12-31".to_string()),
                })
            )
            .is_ok()
        );

        // Invalid date format
        assert!(
            FilterValidator::validate_against_type(
                &FilterType::DateRange(crate::source::DateRangeFilter {
                    min_date: None,
                    max_date: None,
                    format: "YYYY-MM-DD".to_string(),
                }),
                &FilterValue::DateRange(DateRangeValue {
                    start: Some("2023/01/15".to_string()),
                    end: None,
                })
            )
            .is_err()
        );
    }

    #[test]
    fn test_helper_functions() {
        // Text length validation
        assert!(FilterValidator::validate_text_length("short", 10).is_ok());
        assert!(FilterValidator::validate_text_length("this is too long", 10).is_err());

        // Numeric range validation
        assert!(FilterValidator::validate_numeric_range(5.0, 0.0, 10.0).is_ok());
        assert!(FilterValidator::validate_numeric_range(15.0, 0.0, 10.0).is_err());

        // Option validation
        assert!(FilterValidator::validate_option("valid", &["valid", "also_valid"]).is_ok());
        assert!(FilterValidator::validate_option("invalid", &["valid", "also_valid"]).is_err());
    }
}
