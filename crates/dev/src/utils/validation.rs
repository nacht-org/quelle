//! Validation utilities for extension generation and development

use eyre::{Result, eyre};

/// Validate and normalize extension name
pub fn validate_extension_name(name: String) -> Result<String> {
    let extension_name = name.to_lowercase().replace("-", "_");

    if extension_name.is_empty() {
        return Err(eyre!("Extension name cannot be empty"));
    }

    if !extension_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(eyre!(
            "Extension name must contain only letters, numbers, and underscores"
        ));
    }

    if extension_name.starts_with('_') || extension_name.ends_with('_') {
        return Err(eyre!("Extension name cannot start or end with underscore"));
    }

    if extension_name.contains("__") {
        return Err(eyre!(
            "Extension name cannot contain consecutive underscores"
        ));
    }

    Ok(extension_name)
}

/// Validate base URL format
pub fn validate_base_url(url: String) -> Result<String> {
    if url.is_empty() {
        return Err(eyre!("Base URL cannot be empty"));
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(eyre!("Base URL must start with http:// or https://"));
    }

    // Remove trailing slash for consistency
    let normalized_url = url.trim_end_matches('/');

    // Basic URL validation
    if normalized_url.len() < 12 {
        // minimum: "https://a.b"
        return Err(eyre!("Base URL appears to be too short"));
    }

    Ok(normalized_url.to_string())
}

/// Validate language code (ISO 639-1)
pub fn validate_language(lang: String) -> Result<String> {
    let lang = lang.trim().to_string();

    if lang.is_empty() {
        return Err(eyre!("Language code cannot be empty"));
    }

    if lang.len() != 2 {
        return Err(eyre!(
            "Language code must be exactly 2 characters (ISO 639-1)"
        ));
    }

    if !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(eyre!(
            "Language code must contain only lowercase ASCII letters"
        ));
    }

    Ok(lang.to_lowercase())
}

/// Validate and normalize reading direction
pub fn validate_reading_direction(dir: String) -> Result<String> {
    let dir = dir.trim().to_lowercase();

    match dir.as_str() {
        "ltr" | "left-to-right" | "lefttoright" => Ok("Ltr".to_string()),
        "rtl" | "right-to-left" | "righttoleft" => Ok("Rtl".to_string()),
        "" => Err(eyre!("Reading direction cannot be empty")),
        _ => Err(eyre!(
            "Reading direction must be 'ltr' or 'rtl' (got '{}')",
            dir
        )),
    }
}

/// Validate display name
pub fn validate_display_name(name: String) -> Result<String> {
    let name = name.trim().to_string();

    if name.is_empty() {
        return Err(eyre!("Display name cannot be empty"));
    }

    if name.len() > 100 {
        return Err(eyre!("Display name must be 100 characters or less"));
    }

    // Check for reasonable characters (letters, numbers, spaces, basic punctuation)
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_whitespace() || ".-_()[]{}!?".contains(c))
    {
        return Err(eyre!(
            "Display name contains invalid characters. Only letters, numbers, spaces, and basic punctuation are allowed"
        ));
    }

    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_extension_name() {
        assert!(validate_extension_name("test_site".to_string()).is_ok());
        assert!(validate_extension_name("TestSite".to_string()).is_ok());
        assert!(validate_extension_name("test-site".to_string()).is_ok());

        assert!(validate_extension_name("".to_string()).is_err());
        assert!(validate_extension_name("_test".to_string()).is_err());
        assert!(validate_extension_name("test_".to_string()).is_err());
        assert!(validate_extension_name("test__site".to_string()).is_err());
        assert!(validate_extension_name("test-site!".to_string()).is_err());
    }

    #[test]
    fn test_validate_base_url() {
        assert!(validate_base_url("https://example.com".to_string()).is_ok());
        assert!(validate_base_url("http://test.org/".to_string()).is_ok());

        assert!(validate_base_url("".to_string()).is_err());
        assert!(validate_base_url("example.com".to_string()).is_err());
        assert!(validate_base_url("ftp://test.com".to_string()).is_err());
    }

    #[test]
    fn test_validate_language() {
        assert!(validate_language("en".to_string()).is_ok());
        assert!(validate_language("ja".to_string()).is_ok());

        assert!(validate_language("".to_string()).is_err());
        assert!(validate_language("eng".to_string()).is_err());
        assert!(validate_language("EN".to_string()).is_err());
        assert!(validate_language("e1".to_string()).is_err());
    }

    #[test]
    fn test_validate_reading_direction() {
        assert_eq!(
            validate_reading_direction("ltr".to_string()).unwrap(),
            "Ltr"
        );
        assert_eq!(
            validate_reading_direction("rtl".to_string()).unwrap(),
            "Rtl"
        );
        assert_eq!(
            validate_reading_direction("left-to-right".to_string()).unwrap(),
            "Ltr"
        );

        assert!(validate_reading_direction("".to_string()).is_err());
        assert!(validate_reading_direction("top-to-bottom".to_string()).is_err());
    }

    #[test]
    fn test_validate_display_name() {
        assert!(validate_display_name("Test Site".to_string()).is_ok());
        assert!(validate_display_name("Novel Reader (Beta)".to_string()).is_ok());

        assert!(validate_display_name("".to_string()).is_err());
        assert!(validate_display_name("   ".to_string()).is_err());
        assert!(validate_display_name("Test@Site".to_string()).is_err());
    }
}
