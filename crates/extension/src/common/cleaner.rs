//! Smart HTML content cleaner for chapter content.
//!
//! [`ContentCleaner`] strips noise from raw chapter HTML — ads, hidden
//! anti-scraping spans, script/style tags, social widgets, empty wrappers,
//! and unwanted attributes — leaving clean, semantic markup.
//!
//! [`ContentCleaner::new()`] is pre-loaded with sensible defaults. Builder
//! methods let each extension layer site-specific rules on top:
//!
//! ```rust,ignore
//! let content = ContentCleaner::new()
//!     .remove(".hidden-anti-scrape-class")
//!     .strip_attr("data-chapter-id")
//!     .clean(&chapter_element)?;
//! ```

use crate::common::scraping::{ChildNode, Element};
use eyre::eyre;

/// Tags that are always detached — they carry no readable content.
const DEFAULT_REMOVE_TAGS: &[&str] = &[
    "script", "style", "noscript", "iframe", "object", "embed", "canvas", "svg", "form", "button",
    "input", "select", "textarea",
];

/// CSS selectors for common ad / tracker / social-widget containers.
const DEFAULT_REMOVE_SELECTORS: &[&str] = &[
    // Generic ad containers
    "[class*='advert']",
    "[class*='advertisement']",
    "[id*='advert']",
    "[id*='advertisement']",
    "[class*=' ad-']",
    "[class*=' ad_']",
    "[id*=' ad-']",
    "[id*=' ad_']",
    // Google AdSense / DFP
    "ins.adsbygoogle",
    "[data-ad-client]",
    "[data-ad-slot]",
    // Social share / follow widgets
    "[class*='share']",
    "[class*='social']",
    "[class*='follow']",
    // Cookie banners & GDPR notices
    "[class*='cookie']",
    "[id*='cookie']",
    // Newsletter / subscribe prompts
    "[class*='newsletter']",
    "[class*='subscribe']",
    "[id*='newsletter']",
    "[id*='subscribe']",
    // Generic "sponsored" markers
    "[class*='sponsor']",
    "[id*='sponsor']",
    // Donation / Patreon nags often embedded in chapters
    "[class*='patreon']",
    "[class*='donation']",
    "[class*='support-author']",
    // Inline-hidden elements (extensions can add their own rules via `.remove(selector)`)
    "[style*='display:none']",
    "[style*='display: none']",
    "[style*='visibility:hidden']",
    "[style*='visibility: hidden']",
];

/// Block tags for which a whitespace-only element is detached.
const DEFAULT_EMPTY_BLOCK_TAGS: &[&str] = &["p", "div", "section", "blockquote", "li", "dd"];

/// Attributes stripped from every element by default.
const DEFAULT_STRIP_ATTRS: &[&str] = &[
    "style",
    "onclick",
    "onmouseover",
    "onmouseout",
    "onmouseenter",
    "onmouseleave",
    "onfocus",
    "onblur",
    "onchange",
    "onsubmit",
    "onload",
    "onerror",
    "onkeydown",
    "onkeyup",
    "onkeypress",
    "data-src-original",
    "data-lazy-src",
    "data-track",
    "data-analytics",
    "data-ad",
    "jscontroller",
    "jsaction",
    "jsmodel",
    "jsname",
];

/// Controls how attributes are cleaned across the tree.
#[derive(Clone, Debug)]
enum AttrStrategy {
    /// Strip these specific attribute names from every element.
    Denylist(Vec<String>),
    /// Strip every attribute whose name is NOT in this set.
    Allowlist(Vec<String>),
}

impl Default for AttrStrategy {
    fn default() -> Self {
        AttrStrategy::Denylist(DEFAULT_STRIP_ATTRS.iter().map(|s| s.to_string()).collect())
    }
}

/// A composable, reusable chapter-content cleaner.
///
/// Start with [`ContentCleaner::new()`] for smart defaults, or
/// [`ContentCleaner::empty()`] for full manual control.
#[derive(Clone, Debug)]
pub struct ContentCleaner {
    remove_selectors: Vec<String>,
    remove_tags: Vec<String>,
    remove_empty_tags: Vec<String>,
    attr_strategy: AttrStrategy,
}

impl Default for ContentCleaner {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCleaner {
    /// Creates a cleaner pre-loaded with smart defaults.
    pub fn new() -> Self {
        Self {
            remove_tags: DEFAULT_REMOVE_TAGS.iter().map(|s| s.to_string()).collect(),
            remove_selectors: DEFAULT_REMOVE_SELECTORS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            remove_empty_tags: DEFAULT_EMPTY_BLOCK_TAGS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            attr_strategy: AttrStrategy::default(),
        }
    }

    /// Creates an empty cleaner with no rules applied.
    pub fn empty() -> Self {
        Self {
            remove_tags: Vec::new(),
            remove_selectors: Vec::new(),
            remove_empty_tags: Vec::new(),
            attr_strategy: AttrStrategy::Denylist(Vec::new()),
        }
    }

    /// Replace the default tag removal list entirely.
    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.remove_tags = tags.iter().map(|s| s.to_ascii_lowercase()).collect();
        self
    }

    /// Replace the default selector removal list entirely.
    pub fn with_selectors(mut self, selectors: &[&str]) -> Self {
        self.remove_selectors = selectors.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Replace the default empty-tag list entirely.
    pub fn with_empty_tags(mut self, tags: &[&str]) -> Self {
        self.remove_empty_tags = tags.iter().map(|s| s.to_ascii_lowercase()).collect();
        self
    }

    /// Replace the default strip-attribute list entirely.
    /// Switches back to denylist mode if [`keep_attrs`](Self::keep_attrs) was called before.
    pub fn with_strip_attrs(mut self, attrs: &[&str]) -> Self {
        self.attr_strategy = AttrStrategy::Denylist(attrs.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Detach all elements matching `selector`.
    pub fn remove(mut self, selector: impl Into<String>) -> Self {
        self.remove_selectors.push(selector.into());
        self
    }

    /// Detach all elements with the given tag name, regardless of content.
    pub fn remove_tag(mut self, tag: impl Into<String>) -> Self {
        self.remove_tags.push(tag.into().to_ascii_lowercase());
        self
    }

    /// Detach elements with the given tag name when they contain only whitespace.
    pub fn remove_empty_tag(mut self, tag: impl Into<String>) -> Self {
        self.remove_empty_tags.push(tag.into().to_ascii_lowercase());
        self
    }

    /// Strip a specific attribute from every element. No-op if [`keep_attrs`](Self::keep_attrs)
    /// has already been called.
    pub fn strip_attr(mut self, attr: impl Into<String>) -> Self {
        if let AttrStrategy::Denylist(ref mut list) = self.attr_strategy {
            list.push(attr.into());
        }
        self
    }

    /// Switch to an allowlist strategy: keep only `attrs` and strip everything else.
    /// Replaces any previously configured denylist.
    pub fn keep_attrs(mut self, attrs: &[&str]) -> Self {
        self.attr_strategy = AttrStrategy::Allowlist(attrs.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Clean `element` in-place and return its inner HTML.
    ///
    /// Passes are applied in order:
    /// 1. Detach by tag name.
    /// 2. Detach by CSS selector.
    /// 3. Detach whitespace-only block elements.
    /// 4. Strip / allowlist attributes on all remaining elements.
    pub fn clean(&self, element: &Element) -> eyre::Result<String> {
        // Pass 1: detach by tag name.
        // All tags are combined into one selector to minimise WIT boundary crossings.
        if !self.remove_tags.is_empty() {
            let selector = self.remove_tags.join(",");
            for el in element
                .select(&selector)
                .map_err(|e| eyre!("invalid remove_tags selector `{selector}`: {e}"))?
            {
                el.detach();
            }
        }

        // Pass 2: detach by CSS selector.
        for selector in &self.remove_selectors {
            match element.select(selector) {
                Ok(list) => list.into_iter().for_each(|el| el.detach()),
                Err(e) => return Err(eyre!("invalid remove selector `{selector}`: {e}")),
            }
        }

        // Pass 3: detach whitespace-only block elements.
        if !self.remove_empty_tags.is_empty() {
            let selector = self.remove_empty_tags.join(",");
            let candidates = element
                .select(&selector)
                .map_err(|e| eyre!("invalid remove_empty_tags selector `{selector}`: {e}"))?;

            for el in candidates {
                if is_whitespace_only(&el) {
                    el.detach();
                }
            }
        }

        // Pass 4: attribute cleaning — walk the subtree on the guest side.
        self.clean_attrs_recursive(element);

        element
            .inner_html_opt()
            .ok_or_else(|| eyre!("element was empty after cleaning"))
    }

    fn clean_attrs_recursive(&self, element: &Element) {
        self.apply_attr_strategy(element);

        for child in element.children() {
            if let ChildNode::Element(child_el) = child {
                self.clean_attrs_recursive(&child_el);
            }
        }
    }

    fn apply_attr_strategy(&self, element: &Element) {
        match &self.attr_strategy {
            AttrStrategy::Denylist(deny) => {
                for attr in deny {
                    // Check before removing to avoid a redundant WIT call on the miss path.
                    if element.has_attr(attr) {
                        element.remove_attr(attr);
                    }
                }
            }
            AttrStrategy::Allowlist(keep) => {
                for attr in element.attr_names() {
                    if !keep.iter().any(|k| k == &attr) {
                        element.remove_attr(&attr);
                    }
                }
            }
        }
    }
}

/// Returns `true` if `element` has no child elements and its text is entirely whitespace.
fn is_whitespace_only(element: &Element) -> bool {
    let has_element_child = element
        .children()
        .iter()
        .any(|c| matches!(c, ChildNode::Element(_)));

    !has_element_child && element.text_or_empty().trim().is_empty()
}
