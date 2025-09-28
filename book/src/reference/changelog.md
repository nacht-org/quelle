# Changelog

This changelog tracks major changes and developments in Quelle. Since Quelle is in early development, this mainly covers implementation milestones.

## Current Status (Pre-MVP)

Quelle is currently in early development. The core architecture is working but many features are still being built.

### What's Working Now

- ✅ **Core WASM Engine**: Extensions load and run successfully
- ✅ **CLI Framework**: Basic command structure implemented
- ✅ **Store System**: Local directory stores work
- ✅ **Extension Management**: Install, list, and manage extensions
- ✅ **Sample Extensions**: DragonTea and ScribbleHub extensions functional
- ✅ **Novel Fetching**: Can get novel info from supported URLs
- ✅ **Chapter Fetching**: Can download individual chapters
- ✅ **Basic Search**: Search functionality when extensions support it

### Recent Development (2024)

**Core Infrastructure**
- Implemented WebAssembly extension system using Wasmtime
- Created WIT (WebAssembly Interface Types) definitions for extensions
- Built CLI using Clap with store and extension management commands
- Implemented local store system for extension management
- Created extension build system using `just` and `cargo-component`

**Extensions**
- DragonTea extension: Supports novel info and chapter fetching
- ScribbleHub extension: Basic novel and chapter scraping
- Extension auto-detection based on URL patterns

**CLI Commands**
- `quelle status` - System status overview
- `quelle list` - Show available extensions
- `quelle fetch novel/chapter` - Content fetching
- `quelle search` - Novel search across extensions
- `quelle store` commands - Store management
- `quelle extension` commands - Extension management

## Development Milestones

### Phase 1: Foundation (Completed)
- [x] Project architecture design
- [x] WASM runtime implementation
- [x] Basic CLI structure
- [x] Extension interface design
- [x] Local store implementation
- [x] Sample extensions

### Phase 2: Core Features (In Progress)
- [x] Extension auto-installation
- [x] URL-to-extension matching
- [x] Basic search functionality
- [ ] Multiple output formats (EPUB, PDF)
- [ ] Batch downloading
- [ ] Enhanced error handling

### Phase 3: User Experience (Planned)
- [ ] Pre-built binaries
- [ ] Simplified installation
- [ ] Git/HTTP stores
- [ ] Extension versioning
- [ ] Dependency management
- [ ] Configuration profiles

### Phase 4: Advanced Features (Future)
- [ ] Web interface
- [ ] Extension marketplace
- [ ] Automatic updates
- [ ] Download resume
- [ ] Advanced search filters

## Known Issues

- Extension installation from stores has limitations
- Manual WASM file copying often required
- Limited extension metadata
- No version management yet
- Basic error messages

## Technical Details

**Architecture**
- Rust-based core with WebAssembly extensions
- Wasmtime runtime for WASM execution
- Component model for extension interfaces
- Local file-based stores and registry

**Build System**
- Cargo workspace for multiple crates
- `just` for build automation
- `cargo-component` for WASM components
- Cross-platform build support

**Dependencies**
- Wasmtime 37.0 for WASM runtime
- Clap 4.x for CLI interface  
- Tokio for async operations
- Scraper for HTML parsing in extensions

## Looking Forward

The immediate focus is on:

1. **Stability**: Making current features more reliable
2. **Documentation**: Improving user and developer guides
3. **Extensions**: Adding support for more novel sites
4. **User Experience**: Simplifying installation and usage

## Contributing

Since Quelle is in active development:

- Core architecture is stable but APIs may change
- Extension interfaces are maturing
- Documentation reflects current capabilities
- Community contributions welcome

## Versioning

Quelle currently uses development builds without formal versioning. Once the MVP is complete, semantic versioning will be adopted.

For the latest updates, check:
- GitHub repository commits
- Project discussions
- This documentation (updated regularly)

---

**Note**: This changelog will be more detailed once Quelle reaches MVP and begins regular releases. Currently, it serves as a development status tracker.