# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- New `wit/scraper.wit` interface exposing HTML parsing and tree traversal to
  extensions via the host. Resources:
  - `document` — parse an HTML string; `select` and `select-first` via CSS
    selector.
  - `node` — owned element handle; `select`, `select-first`, `attr`, `text`,
    `outer-html`, `inner-html`, `detach`, and `children`.
  - `text-node` — owned handle to a raw text node; `text` and `set-text` for
    in-place mutation.
  - `child-node` variant — `element(node) | text(text-node)`, the primitive
    for writing any tree traversal (pre-order, post-order, BFS, etc.) entirely
    in extension code.
- `TextNode` type in `quelle_extension` wrapping the `text-node` WIT resource,
  with `.text()` and `.set_text()` methods.
- `ChildNode` enum in `quelle_extension` — `Element(Element) | Text(TextNode)`,
  re-exported from the prelude.
- `Element::children() -> Vec<ChildNode>` for tree traversal from extension
  code.
- `Element::attr() -> Result<String, _>` as a direct method alongside the
  existing `attr_opt`.
- `ElementList::len()` method.

### Changed

- `Html`, `Element`, and `ElementList` in `quelle_extension` now wrap WIT
  resources instead of `scraper` types directly — **lifetime parameters
  removed** from all three types.
- `Element::detach(self)` now consumes the owned handle directly. The previous
  pattern of collecting `NodeId`s and calling `Html::detach` is no longer
  needed.
- `scraper` and `ego-tree` dependencies moved from `quelle_extension` to
  `quelle_engine`, where they back the host implementation of the scraper WIT
  interface.
- `dragontea` extension: removed the internal `jumble` module; anti-scraping
  text remapping now walks children via `Element::children()` and writes back
  through `TextNode::set_text`.
- `novelfull` extension: replaced `element.element.id()` + `doc.detach(node_id)`
  with direct `element.detach()` calls.
- `royalroad` extension: same detach simplification; `mut doc` binding removed.