# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- `wit/scraper.wit` interface exposing HTML parsing and tree traversal to extensions. Resources: `document`, `node`, `text-node`, and `child-node` variant.
- `TextNode`, `ChildNode`, and `Element::children()` in `quelle_extension` for tree traversal from extension code.
- `Element::attr()` as a direct method alongside `attr_opt`.
- `ElementList::len()` method.
- Added `ghostwire` as the cloud-scraper backend for bypassing Cloudflare protections.

### Changed

- `Html`, `Element`, and `ElementList` now wrap WIT resources — lifetime parameters removed from all three types.
- `Element::detach(self)` consumes the owned handle directly; collecting `NodeId`s via `Html::detach` is no longer needed.
- `scraper` and `ego-tree` dependencies moved from `quelle_extension` to `quelle_engine`.
- `dragontea`: anti-scraping remapping now uses `Element::children()` and `TextNode::set_text`.
- `novelfull`, `royalroad`: simplified detach calls.
- Upgraded wasmtime to 42.0.
