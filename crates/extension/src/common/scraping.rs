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

/// A raw text node within the document tree. Carries no tag or attributes.
pub struct TextNode {
    pub(crate) node: wit_scraper::TextNode,
}

impl TextNode {
    /// Returns the text content of this node.
    pub fn text(&self) -> String {
        self.node.text()
    }

    /// Overwrites the text content of this node in-place.
    pub fn set_text(&self, content: impl Into<String>) {
        self.node.set_text(&content.into());
    }
}

// ---------------------------------------------------------------------------
// Html
// ---------------------------------------------------------------------------

/// A parsed HTML document — the root you start all queries from.
pub struct Html {
    pub(crate) doc: wit_scraper::Document,
}

impl Html {
    /// Parses an HTML string into a queryable document.
    pub fn new(html: &str) -> Self {
        Html {
            doc: wit_scraper::Document::new(html),
        }
    }

    /// Returns all elements matching `pattern`, or an error if the selector is malformed.
    pub fn select(&self, pattern: &str) -> Result<ElementList, eyre::Report> {
        let nodes = self
            .doc
            .select(pattern)
            .map_err(|e| eyre::Report::msg(e.message))?;
        Ok(ElementList {
            elements: nodes.into_iter().map(|node| Element { node }).collect(),
        })
    }

    /// Returns the first element matching `pattern`, or an error if none is found
    /// or the selector is malformed.
    pub fn select_first(&self, pattern: &str) -> Result<Element, eyre::Report> {
        self.select_first_opt(pattern)?
            .ok_or_else(|| eyre!("no element matched selector `{pattern}`"))
    }

    /// Returns the first element matching `pattern`, or `None` if absent.
    /// Returns an error if the selector is malformed.
    pub fn select_first_opt(&self, pattern: &str) -> Result<Option<Element>, eyre::Report> {
        let node = self
            .doc
            .select_first(pattern)
            .map_err(|e| eyre::Report::msg(e.message))?;
        Ok(node.map(|node| Element { node }))
    }
}

// ---------------------------------------------------------------------------
// ElementList
// ---------------------------------------------------------------------------

/// A collection of matched HTML elements.
pub struct ElementList {
    pub(crate) elements: Vec<Element>,
}

impl ElementList {
    /// Returns an iterator over the elements.
    pub fn iter(&self) -> impl Iterator<Item = &Element> + '_ {
        self.elements.iter()
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Returns `true` if the list contains no elements.
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

/// A single HTML element within the parsed document.
pub struct Element {
    pub(crate) node: wit_scraper::Node,
}

impl Element {
    /// Returns all descendant elements matching `pattern`, or an error if the
    /// selector is malformed.
    pub fn select(&self, pattern: &str) -> Result<ElementList, eyre::Report> {
        let nodes = self
            .node
            .select(pattern)
            .map_err(|e| eyre::Report::msg(e.message))?;
        Ok(ElementList {
            elements: nodes.into_iter().map(|node| Element { node }).collect(),
        })
    }

    /// Returns the first descendant element matching `pattern`, or an error if
    /// none is found or the selector is malformed.
    pub fn select_first(&self, pattern: &str) -> Result<Element, eyre::Report> {
        self.select_first_opt(pattern)?.ok_or_else(|| {
            eyre!(
                "no element matched selector `{pattern}` within <{}>",
                self.name()
            )
        })
    }

    /// Returns the first descendant element matching `pattern`, or `None` if absent.
    /// Returns an error if the selector is malformed.
    pub fn select_first_opt(&self, pattern: &str) -> Result<Option<Element>, eyre::Report> {
        let node = self
            .node
            .select_first(pattern)
            .map_err(|e| eyre::Report::msg(e.message))?;
        Ok(node.map(|node| Element { node }))
    }

    /// Returns the tag name of this element (e.g. `"div"`, `"a"`, `"p"`).
    pub fn name(&self) -> String {
        self.node.name()
    }

    /// Returns the value of `name`, or an error if the attribute is absent.
    pub fn attr(&self, name: &str) -> Result<String, eyre::Report> {
        self.attr_opt(name)
            .ok_or_else(|| eyre!("attribute `{name}` not found on <{}>", self.name()))
    }

    /// Returns the value of `name`, or `None` if the attribute is absent.
    pub fn attr_opt(&self, name: &str) -> Option<String> {
        self.node.attr(name)
    }

    /// Returns `true` if this element has the given attribute.
    pub fn has_attr(&self, name: &str) -> bool {
        self.node.has_attr(name)
    }

    /// Removes the given attribute from this element. No-op if absent.
    pub fn remove_attr(&self, name: &str) {
        self.node.remove_attr(name);
    }

    /// Returns the names of all attributes present on this element.
    pub fn attr_names(&self) -> Vec<String> {
        self.node.attr_names()
    }

    /// Returns the trimmed text content, or `None` if empty.
    pub fn text_opt(&self) -> Option<String> {
        let value = self.node.text();
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    /// Returns the trimmed text content, or an empty string if absent.
    pub fn text_or_empty(&self) -> String {
        self.node.text().trim().to_string()
    }

    /// Returns the trimmed outer HTML (tag + children), or `None` if empty.
    pub fn html_opt(&self) -> Option<String> {
        let value = self.node.outer_html();
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    /// Returns the trimmed inner HTML (children only, no wrapping tag), or `None` if empty.
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
    pub fn detach(self) {
        self.node.detach();
    }

    /// Returns the direct children of this element in document order.
    ///
    /// Each child is either an [`Element`] or a [`TextNode`], giving you full
    /// control over tree traversal in extension code.
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
// ElementListExt
// ---------------------------------------------------------------------------

/// Extends `Result<ElementList, _>` with convenience text-extraction methods.
pub trait ElementListExt: Sized {
    /// Returns the trimmed text content of every element in the list.
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
// ElementExt
// ---------------------------------------------------------------------------

/// Extends `Result<Element, _>` with convenience extraction methods.
pub trait ElementExt: Sized {
    /// Returns the trimmed text, or an error if the element has no text content.
    fn text(self) -> Result<String, eyre::Report>;

    /// Returns the trimmed text, or an empty string if absent.
    fn text_or_empty(self) -> Result<String, eyre::Report>;

    /// Returns the trimmed text as `Some`, or `None` if empty.
    fn text_opt(self) -> Result<Option<String>, eyre::Report>;

    /// Returns the value of `name`, or an error if the attribute is absent.
    fn attr(self, name: &str) -> Result<String, eyre::Report>;

    /// Returns the value of `name` as `Some`, or `None` if absent.
    fn attr_opt(self, name: &str) -> Result<Option<String>, eyre::Report>;

    /// Returns the trimmed outer HTML, or an error if the element is empty.
    fn html(self) -> Result<String, eyre::Report>;

    /// Returns the trimmed outer HTML as `Some`, or `None` if empty.
    fn html_opt(self) -> Result<Option<String>, eyre::Report>;
}

impl ElementExt for Result<Element, eyre::Report> {
    fn text(self) -> Result<String, eyre::Report> {
        self.and_then(|element| {
            let tag = element.name();
            element
                .text_opt()
                .ok_or_else(|| eyre!("no text content in <{tag}>"))
        })
    }

    fn text_or_empty(self) -> Result<String, eyre::Report> {
        self.map(|element| element.text_or_empty())
    }

    fn text_opt(self) -> Result<Option<String>, eyre::Report> {
        self.map(|element| element.text_opt())
    }

    fn attr(self, name: &str) -> Result<String, eyre::Report> {
        self.and_then(|element| {
            let tag = element.name();
            element
                .attr_opt(name)
                .ok_or_else(|| eyre!("attribute `{name}` not found on <{tag}>"))
        })
    }

    fn attr_opt(self, name: &str) -> Result<Option<String>, eyre::Report> {
        self.map(|element| element.attr_opt(name))
    }

    fn html(self) -> Result<String, eyre::Report> {
        self.and_then(|element| {
            let tag = element.name();
            element
                .html_opt()
                .ok_or_else(|| eyre!("no HTML content in <{tag}>"))
        })
    }

    fn html_opt(self) -> Result<Option<String>, eyre::Report> {
        self.map(|element| element.html_opt())
    }
}
