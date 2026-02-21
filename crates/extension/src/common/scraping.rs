use eyre::eyre;

use crate::wit::quelle::extension::scraper as wit_scraper;

// ---------------------------------------------------------------------------
// ChildNode
// ---------------------------------------------------------------------------

/// A direct child of a node — either an element or a raw text node.
pub enum ChildNode {
    Element(Element),
    Text(TextNode),
}

// ---------------------------------------------------------------------------
// TextNode
// ---------------------------------------------------------------------------

/// An owned handle to a raw text node within the document tree.
/// Unlike [`Element`], a text node carries no tag or attributes — only a string value.
pub struct TextNode {
    pub(crate) node: wit_scraper::TextNode,
}

impl TextNode {
    /// Read the current text content of this text node.
    pub fn text(&self) -> String {
        self.node.text()
    }

    /// Overwrite the text content of this text node in-place.
    pub fn set_text(&self, content: impl Into<String>) {
        self.node.set_text(&content.into());
    }
}

// ---------------------------------------------------------------------------
// Html
// ---------------------------------------------------------------------------

/// Represents an entire HTML document after it has been parsed.
/// Think of this as your starting point for querying any web page.
pub struct Html {
    pub(crate) doc: wit_scraper::Document,
}

impl Html {
    /// Creates a new `Html` by parsing an HTML string.
    ///
    /// Use this to load your HTML content and prepare it for querying.
    ///
    /// # Example
    /// ```rust
    /// # use quelle_extension::prelude::{Html, ElementListExt};
    /// # fn main() -> Result<(), eyre::Report> {
    /// let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
    /// let doc = Html::new(html);
    /// let title = doc.select_first("h1")?.text_or_empty();
    /// assert_eq!(title, "Hello");
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(html: &str) -> Self {
        Html {
            doc: wit_scraper::Document::new(html),
        }
    }

    /// Finds all HTML elements in the document that match your **CSS selector**.
    ///
    /// Use this when you expect multiple items (e.g., all list items, all product cards).
    /// Returns an `ElementList` containing all matches, or an error if the selector is malformed.
    pub fn select(&self, pattern: &str) -> Result<ElementList, eyre::Report> {
        let nodes = self
            .doc
            .select(pattern)
            .map_err(|e| eyre!("Failed to compile selector `{pattern}`: {}", e.message))?;
        Ok(ElementList {
            elements: nodes.into_iter().map(|node| Element { node }).collect(),
        })
    }

    /// Finds the **first** HTML element in the document that matches your **CSS selector**.
    ///
    /// This is useful when you're looking for a unique element, like a page title or a single
    /// primary image. It returns the first matching `Element`, or an error if nothing is found
    /// or the selector is malformed.
    pub fn select_first(&self, pattern: &str) -> Result<Element, eyre::Report> {
        self.select_first_opt(pattern)?
            .ok_or_else(|| eyre!("Element not found: {pattern}"))
    }

    /// Optionally finds the **first** HTML element in the document matching your **CSS selector**.
    ///
    /// Use this when an element might or might not be present on the page, allowing you to
    /// gracefully handle its absence. It returns `Some(Element)` if a match is found,
    /// `None` if no match, or an error if the selector is malformed.
    pub fn select_first_opt(&self, pattern: &str) -> Result<Option<Element>, eyre::Report> {
        let node = self
            .doc
            .select_first(pattern)
            .map_err(|e| eyre!("Failed to compile selector `{pattern}`: {}", e.message))?;
        Ok(node.map(|node| Element { node }))
    }
}

// ---------------------------------------------------------------------------
// ElementList
// ---------------------------------------------------------------------------

/// A collection of HTML elements, typically the result of a selection query
/// that matches multiple items. You can iterate over these to process each element.
pub struct ElementList {
    pub(crate) elements: Vec<Element>,
}

impl ElementList {
    /// Creates an iterator over the elements in the list.
    pub fn iter(&self) -> impl Iterator<Item = &Element> + '_ {
        self.elements.iter()
    }

    /// Returns the number of elements in the list.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Returns whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

impl IntoIterator for ElementList {
    type Item = Element;
    type IntoIter = std::vec::IntoIter<Element>;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

// ---------------------------------------------------------------------------
// Element
// ---------------------------------------------------------------------------

/// Represents a single HTML element within the parsed document.
/// You can perform further selections or extract data from this individual element.
pub struct Element {
    pub(crate) node: wit_scraper::Node,
}

impl Element {
    /// Finds all **descendant** HTML elements *within this specific element* that match the **CSS selector**.
    ///
    /// Use this to narrow your search. For example, find all links (`<a>`) *only within* a specific
    /// navigation bar (`<nav>`). It returns an `ElementList` of matching descendants,
    /// or an error if the selector is malformed.
    pub fn select(&self, pattern: &str) -> Result<ElementList, eyre::Report> {
        let nodes = self
            .node
            .select(pattern)
            .map_err(|e| eyre!("Failed to compile selector `{pattern}`: {}", e.message))?;
        Ok(ElementList {
            elements: nodes.into_iter().map(|node| Element { node }).collect(),
        })
    }

    /// Finds the **first descendant** HTML element *within this specific element* that matches the **CSS selector**.
    ///
    /// Ideal for drilling down to a unique sub-element, like finding the price (`<span>`)
    /// *inside* a particular product listing (`<div>`). It returns the first matching `Element`,
    /// or an error if nothing is found or the selector is malformed.
    pub fn select_first(&self, pattern: &str) -> Result<Element, eyre::Report> {
        self.select_first_opt(pattern)?
            .ok_or_else(|| eyre!("Element not found: {pattern}"))
    }

    /// Optionally finds the **first descendant** HTML element *within this specific element*
    /// matching the **CSS selector**.
    ///
    /// Use this to try and find an optional sub-element without causing an error if it's missing.
    /// It returns `Some(Element)` if a match is found, `None` if no match,
    /// or an error if the selector is malformed.
    pub fn select_first_opt(&self, pattern: &str) -> Result<Option<Element>, eyre::Report> {
        let node = self
            .node
            .select_first(pattern)
            .map_err(|e| eyre!("Failed to compile selector `{pattern}`: {}", e.message))?;
        Ok(node.map(|node| Element { node }))
    }

    /// Retrieves the value of a specific **HTML attribute** (e.g., `href`, `src`, `id`).
    ///
    /// Use this to extract data stored in attributes, like the URL from a link or the image source.
    /// It returns the attribute's value as a `String`, or an error if the attribute doesn't exist.
    pub fn attr(&self, name: &str) -> Result<String, eyre::Report> {
        self.attr_opt(name)
            .ok_or_else(|| eyre!("attribute '{name}' not found in element"))
    }

    /// Use this when an attribute might or might not be present on an element.
    /// It returns `Some(String)` with the attribute's value, or `None` if the attribute is missing.
    pub fn attr_opt(&self, name: &str) -> Option<String> {
        self.node.attr(name)
    }

    /// Extracts the **trimmed visible text content** of this element and all its children.
    ///
    /// Returns `None` if the element contains no text.
    pub fn text_opt(&self) -> Option<String> {
        let value = self.node.text();
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    /// Retrieves the **trimmed visible text content** of this element and all its children.
    ///
    /// This is your go-to method for extracting the human-readable content, like the title of an article,
    /// or the text inside a paragraph. It returns a `String` containing the combined, trimmed text.
    /// Returns an empty string if no text is found.
    pub fn text_or_empty(&self) -> String {
        self.node.text().trim().to_string()
    }

    /// Returns the **outer HTML** of this element, including its own tag and all its children.
    ///
    /// This is useful when you want to extract the full HTML markup for a section of the page,
    /// such as a card, a div, or any container element.
    pub fn html_opt(&self) -> Option<String> {
        let value = self.node.outer_html();
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    /// Returns the **inner HTML** of this element (all children as an HTML string, excluding
    /// this element's own tag).
    pub fn inner_html_opt(&self) -> Option<String> {
        let value = self.node.inner_html();
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    /// Detaches this element from the document tree.
    ///
    /// After calling this, the element is no longer part of the document and will not appear
    /// in any subsequent selections on the document or its ancestors.
    pub fn detach(self) {
        self.node.detach();
    }

    /// Returns the direct children of this element in document order.
    ///
    /// Each child is either an [`Element`] or a [`TextNode`], giving you the
    /// building blocks for any tree traversal — pre-order, post-order, BFS, etc. —
    /// implemented entirely in extension code.
    pub fn children(&self) -> Vec<ChildNode> {
        self.node
            .children()
            .into_iter()
            .map(|child| match child {
                wit_scraper::ChildNode::Element(node) => ChildNode::Element(Element { node }),
                wit_scraper::ChildNode::Text(node) => ChildNode::Text(TextNode { node }),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ElementListExt — extends Result<ElementList, _>
// ---------------------------------------------------------------------------

/// Extends `Result<ElementList, ...>` to easily extract text from multiple elements.
pub trait ElementListExt: Sized {
    /// Gathers the trimmed text content from *all* elements in the list.
    ///
    /// Returns a `Vec<String>` where each string is the text from one element,
    /// or an error if the initial selection failed.
    fn text_all(self) -> Result<Vec<String>, eyre::Report>;
}

impl ElementListExt for Result<ElementList, eyre::Report> {
    fn text_all(self) -> Result<Vec<String>, eyre::Report> {
        self.map(|list| {
            list.elements
                .into_iter()
                .map(|element| element.text_or_empty())
                .collect()
        })
    }
}

// ---------------------------------------------------------------------------
// ElementExt — extends Result<Element, _>
// ---------------------------------------------------------------------------

/// Extends `Result<Element, ...>` to easily extract text, attributes, or HTML from a single element.
pub trait ElementExt: Sized {
    /// Extracts the **trimmed visible text content** of the element.
    ///
    /// Returns the element's text as a `String`, or an error if no text is found.
    fn text(self) -> Result<String, eyre::Report> {
        self.text_opt()?
            .ok_or_else(|| eyre!("text not found in element"))
    }

    /// Retrieves the **trimmed visible text content** of the element.
    ///
    /// Always returns a `String`. If no text is found, returns an empty string.
    fn text_or_empty(self) -> Result<String, eyre::Report> {
        self.text_opt().map(|opt| opt.unwrap_or_default())
    }

    /// Optionally extracts the **trimmed visible text content** of the element.
    ///
    /// Returns `Some(String)` with the element's text, `None` if no text, or an error if
    /// the initial element selection failed.
    fn text_opt(self) -> Result<Option<String>, eyre::Report>;

    /// Retrieves the value of a specific **HTML attribute** (e.g., `href`, `src`, `id`).
    ///
    /// Returns the attribute's value as a `String`, or an error if the attribute is not found.
    fn attr(self, name: &str) -> Result<String, eyre::Report> {
        self.attr_opt(name)?
            .ok_or_else(|| eyre!("attribute '{name}' not found in element"))
    }

    /// Optionally retrieves the value of a specific **HTML attribute**.
    ///
    /// Returns `Some(String)` with the attribute's value, `None` if missing, or an error if
    /// the initial element selection failed.
    fn attr_opt(self, name: &str) -> Result<Option<String>, eyre::Report>;

    /// Retrieves the **trimmed outer HTML content** of the element, including its own tag
    /// and all its children.
    ///
    /// Returns the outer HTML as a `String`, or an error if no HTML content is found.
    fn html(self) -> Result<String, eyre::Report> {
        self.html_opt()?
            .ok_or_else(|| eyre!("HTML content not found in element"))
    }

    /// Optionally retrieves the **trimmed outer HTML content** of the element.
    ///
    /// Returns `Some(String)` with the outer HTML if it exists, or `None` if empty.
    fn html_opt(self) -> Result<Option<String>, eyre::Report>;
}

impl ElementExt for Result<Element, eyre::Report> {
    fn text_opt(self) -> Result<Option<String>, eyre::Report> {
        self.map(|element| element.text_opt())
    }

    fn attr_opt(self, name: &str) -> Result<Option<String>, eyre::Report> {
        self.map(|element| element.attr_opt(name))
    }

    fn html_opt(self) -> Result<Option<String>, eyre::Report> {
        self.map(|element| element.html_opt())
    }
}
