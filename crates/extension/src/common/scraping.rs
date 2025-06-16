use eyre::eyre;
use scraper::{ElementRef, Html, Selector};

pub fn select_first<'a>(doc: &'a Html, selector_str: &str) -> Result<ElementRef<'a>, eyre::Report> {
    let selector =
        Selector::parse(selector_str).map_err(|e| eyre!("Failed to compile selector: {e}"))?;

    doc.select(&selector)
        .next()
        .ok_or_else(|| eyre!("Element not found: {selector_str}"))
}

pub fn select_first_text(doc: &Html, selector_str: &str) -> Result<String, eyre::Report> {
    Ok(select_first(doc, selector_str)?.text().collect::<String>())
}

pub fn select<'a>(doc: &'a Html, selector_str: &str) -> Result<Vec<ElementRef<'a>>, eyre::Report> {
    let selector =
        Selector::parse(selector_str).map_err(|e| eyre!("Failed to compile selector: {e}"))?;
    Ok(doc.select(&selector).collect())
}

pub fn select_text(doc: &Html, selector_str: &str) -> Result<Vec<String>, eyre::Report> {
    Ok(select(doc, selector_str)?
        .iter()
        .map(|node| node.text().collect::<String>())
        .collect())
}

pub fn select_first_attr(
    doc: &Html,
    selector_str: &str,
    attr: &str,
) -> Result<String, eyre::Report> {
    let element = select_first(doc, selector_str)?;
    element
        .attr(attr)
        .map(|s| s.to_string())
        .ok_or_else(|| eyre!("attribute '{attr}' not found in element: {selector_str}"))
}

pub fn select_first_attr_opt(
    doc: &Html,
    selector_str: &str,
    attr: &str,
) -> Result<Option<String>, eyre::Report> {
    let element = select_first(doc, selector_str)?;
    Ok(element.attr(attr).map(|s| s.to_string()))
}
