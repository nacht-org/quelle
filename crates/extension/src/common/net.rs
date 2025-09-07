/// Constructs an absolute URL from a relative URL and a base URL.
///
/// # Examples
///
/// ```
/// let abs = make_absolute_url("/foo/bar", "https://example.com/base");
/// assert_eq!(abs, "https://example.com/foo/bar");
/// ```
pub fn make_absolute_url(relative_url: &str, base_url: &str) -> String {
    if relative_url.starts_with("http://") || relative_url.starts_with("https://") {
        relative_url.to_string()
    } else if relative_url.starts_with("//") {
        format!("https:{}", relative_url)
    } else if relative_url.starts_with('/') {
        let base = base_url.trim_end_matches('/');
        if let Some(domain_end) = base[8..].find('/').map(|i| i + 8) {
            format!("{}{}", &base[..domain_end], relative_url)
        } else {
            format!("{}{}", base, relative_url)
        }
    } else {
        let base = base_url.trim_end_matches('/');
        format!("{}/{}", base, relative_url)
    }
}
