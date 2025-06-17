use eyre::eyre;
use scraper::{ElementRef, Html, Selector};

/// Represents an entire HTML document after it has been parsed.
/// Think of this as your starting point for querying any web page.
pub struct Doc {
    pub doc: Html,
}

/// A collection of HTML elements, typically the result of a selection query
/// that matches multiple items. You can iterate over these to process each element.
pub struct ElementList<'a> {
    pub elements: Vec<ElementRef<'a>>,
}

/// An iterator that allows you to loop through each `Element` within an `ElementList`.
pub struct ElementListIntoIter<'a> {
    pub elements: std::vec::IntoIter<ElementRef<'a>>,
}

impl<'a> IntoIterator for ElementList<'a> {
    type Item = Element<'a>;
    type IntoIter = ElementListIntoIter<'a>;

    /// Allows `ElementList` to be used in a `for` loop, making it easy to process
    /// each found element.
    fn into_iter(self) -> Self::IntoIter {
        ElementListIntoIter {
            elements: self.elements.into_iter(),
        }
    }
}

impl<'a> Iterator for ElementListIntoIter<'a> {
    type Item = Element<'a>;

    /// Retrieves the next `Element` from the list.
    fn next(&mut self) -> Option<Self::Item> {
        self.elements.next().map(|element| Element { element })
    }
}

/// Represents a single HTML element within the parsed document.
/// You can perform further selections or extract data from this individual element.
pub struct Element<'a> {
    pub element: ElementRef<'a>,
}

/// A helper function to parse a CSS selector string into a `Selector`.
fn compile_selector(pattern: &str) -> Result<Selector, eyre::Report> {
    Selector::parse(pattern).map_err(|e| eyre!("Failed to compile selector: {e}"))
}

impl Doc {
    /// Creates a new `Doc` from an HTML string.
    ///
    /// Use this to load your HTML content and prepare it for querying.
    ///
    /// # Example
    /// ```rust
    /// # use extension::prelude::{Doc, ElementListExt};
    /// # fn main() -> Result<(), eyre::Report> {
    /// let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
    /// let doc = Doc::new(html);
    /// let title = doc.select_first("h1")?.text()?;
    /// assert_eq!(title, "Hello");
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(html: &str) -> Self {
        let doc = Html::parse_document(html);
        Doc { doc }
    }

    /// Finds all HTML elements in the document that match your **CSS selector**.
    ///
    /// Use this when you expect multiple items (e.g., all list items, all product cards).
    /// Returns an `ElementList` containing all matches, or an error if the selector is malformed.
    pub fn select(&self, pattern: &str) -> Result<ElementList<'_>, eyre::Report> {
        let selector = compile_selector(pattern)?;
        let elements = self.doc.select(&selector).collect();
        Ok(ElementList { elements })
    }

    /// Finds the **first** HTML element in the document that matches your **CSS selector**.
    ///
    /// This is useful when you're looking for a unique element, like a page title or a single
    /// primary image. It returns the first matching `Element`, or an error if nothing is found
    /// or the selector is malformed.
    pub fn select_first(&self, pattern: &str) -> Result<Element<'_>, eyre::Report> {
        self.select_first_opt(pattern)?
            .ok_or_else(|| eyre!("Element not found: {pattern}"))
    }

    /// Optionally finds the **first** HTML element in the document matching your **CSS selector**.
    ///
    /// Use this when an element might or might not be present on the page, allowing you to
    /// gracefully handle its absence. It returns `Some(Element)` if a match is found,
    /// `None` if no match, or an error if the selector is malformed.
    pub fn select_first_opt(&self, pattern: &str) -> Result<Option<Element<'_>>, eyre::Report> {
        let selector = compile_selector(pattern)?;
        Ok(self
            .doc
            .select(&selector)
            .next()
            .map(|element| Element { element }))
    }
}

/// Extends `Result<ElementList, ...>` to easily extract text from multiple elements.
pub trait ElementListExt<'a>: Sized {
    /// Gathers the trimmed text content from *all* elements in the list.
    ///
    /// This is helpful for quickly collecting data points like all news headlines or
    /// all items in a category. It returns a `Vec<String>` where each string is
    /// the text from one element, or an error if the initial selection failed.
    fn text_all(self) -> Result<Vec<String>, eyre::Report>;
}

impl<'a> ElementListExt<'a> for Result<ElementList<'a>, eyre::Report> {
    fn text_all(self) -> Result<Vec<String>, eyre::Report> {
        self.map(|list| {
            list.elements
                .into_iter()
                .map(|element| element.text().collect::<String>().trim().to_string())
                .collect()
        })
    }
}

impl<'a> Element<'a> {
    /// Finds all **descendant** HTML elements *within this specific element* that match the **CSS selector**.
    ///
    /// Use this to narrow your search. For example, find all links (`<a>`) *only within* a specific
    /// navigation bar (`<nav>`). It returns an `ElementList` of matching descendants,
    /// or an error if the selector is malformed.
    pub fn select(&self, pattern: &str) -> Result<ElementList<'a>, eyre::Report> {
        let selector = compile_selector(pattern)?;
        let elements: Vec<ElementRef<'_>> = self.element.select(&selector).collect();
        Ok(ElementList { elements })
    }

    /// Finds the **first descendant** HTML element *within this specific element* that matches the **CSS selector**.
    ///
    /// Ideal for drilling down to a unique sub-element, like finding the price (`<span>`)
    /// *inside* a particular product listing (`<div>`). It returns the first matching `Element`,
    /// or an error if nothing is found or the selector is malformed.
    pub fn select_first(&self, pattern: &str) -> Result<Element<'a>, eyre::Report> {
        self.select_first_opt(pattern)?
            .ok_or_else(|| eyre!("Element not found: {pattern}"))
    }

    /// Optionally finds the **first descendant** HTML element *within this specific element*
    /// matching the **CSS selector**.
    ///
    /// Use this to try and find an optional sub-element without causing an error if it's missing.
    /// It returns `Some(Element)` if a match is found, `None` if no match,
    /// or an error if the selector is malformed.
    pub fn select_first_opt(&self, pattern: &str) -> Result<Option<Element<'a>>, eyre::Report> {
        let selector = compile_selector(pattern)?;
        Ok(self
            .element
            .select(&selector)
            .next()
            .map(|element| Element { element }))
    }

    /// Retrieves the value of a specific **HTML attribute** (e.g., `href`, `src`, `id`).
    ///
    /// Use this to extract data stored in attributes, like the URL from a link or the image source.
    /// It returns the attribute's value as a `String`, or an error if the attribute doesn't exist.
    pub fn attr(&self, name: &str) -> Result<String, eyre::Report> {
        self.attr_opt(name)
            .ok_or_else(|| eyre!("attribute '{name}' not found in element"))
    }

    /// Optionally retrieves the value of a specific **HTML attribute**.
    ///
    /// Use this when an attribute might or might not be present on an element.
    /// It returns `Some(String)` with the attribute's value, or `None` if the attribute is missing.
    pub fn attr_opt(&self, name: &str) -> Option<String> {
        self.element.value().attr(name).map(|s| s.to_string())
    }

    /// Retrieves the **trimmed visible text content** of this element and all its children.
    ///
    /// This is your go-to method for extracting the human-readable content, like the title of an article,
    /// or the text inside a paragraph. It returns a `String` containing the combined, trimmed text.
    pub fn text_or_empty(&self) -> String {
        self.element.text().collect::<String>().trim().to_string()
    }
}

/// Extends `Result<Element, ...>` to easily extract text, attributes, or HTML from a single element.
pub trait ElementExt<'a>: Sized {
    /// Extracts the **trimmed visible text content** of the element.
    ///
    /// Use this to get the text from an element if you are sure it contains text.
    /// It returns the element's text as a `String`, or an error if no text is found.
    fn text(self) -> Result<String, eyre::Report> {
        self.text_opt()?
            .ok_or_else(|| eyre!("text not found in element"))
    }

    /// Retrieves the **trimmed visible text content** of the element.
    ///
    /// This method always returns a `String`. If no text is found, it provides an
    /// **empty string** (`""`) instead of an error or `None`, making it convenient
    /// for cases where you always expect a string result.
    fn text_or_empty(self) -> Result<String, eyre::Report> {
        self.text_opt().map(|opt| opt.unwrap_or_default())
    }

    /// Optionally extracts the **trimmed visible text content** of the element.
    ///
    /// Use this when an element might not have any text, avoiding an error in such cases.
    /// It returns `Some(String)` with the element's text, `None` if no text, or an error if
    /// the initial element selection failed.
    fn text_opt(self) -> Result<Option<String>, eyre::Report>;

    /// Retrieves the value of a specific **HTML attribute** (e.g., `href`, `src`, `id`).
    ///
    /// Use this when you are certain the attribute exists. It returns the attribute's value
    /// as a `String`, or an error if the attribute is not found.
    fn attr(self, name: &str) -> Result<String, eyre::Report> {
        self.attr_opt(name)?
            .ok_or_else(|| eyre!("attribute '{name}' not found in element"))
    }

    /// Optionally retrieves the value of a specific **HTML attribute**.
    ///
    /// Use this when an attribute might be missing from an element. It returns
    /// `Some(String)` with the attribute's value, `None` if missing, or an error if
    /// the initial element selection failed.
    fn attr_opt(self, name: &str) -> Result<Option<String>, eyre::Report>;

    /// Retrieves the **trimmed outer HTML content** of the element, including its own tag and all its children.
    ///
    /// Use this when you need the complete HTML structure of a section, not just its plain text or inner content.
    /// It returns the outer HTML as a `String`, or an error if the initial element selection failed.
    fn html(self) -> Result<String, eyre::Report>;
}

impl<'a> ElementExt<'a> for Result<Element<'a>, eyre::Report> {
    fn text_opt(self) -> Result<Option<String>, eyre::Report> {
        self.map(|element| {
            let text = element.element.text().collect::<String>();
            if text.is_empty() {
                None
            } else {
                Some(text.trim().to_string())
            }
        })
    }

    fn attr_opt(self, name: &str) -> Result<Option<String>, eyre::Report> {
        self.map(|element| element.element.value().attr(name).map(|s| s.to_string()))
    }

    fn html(self) -> Result<String, eyre::Report> {
        self.map(|element| element.element.html().trim().to_string())
    }
}
