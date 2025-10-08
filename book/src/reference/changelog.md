# Changelog

This changelog tracks major changes and developments in Quelle.

## Current Status (MVP Ready)

**Quelle has reached MVP (Minimum Viable Product) status** with a fully functional CLI, working extension system, and reliable core features.

### What's Working Now

- **Complete CLI Interface**: All major commands implemented and stable
- **Extension System**: Build, install, and manage extensions with full tooling
- **Store Management**: Local and Git-based extension repositories
- **Novel Discovery**: Search and browse novels across multiple sources
- **Library Management**: Add, update, remove, and organize your collection
- **Chapter Reading**: Read chapters directly in your terminal
- **Export Functionality**: Export to EPUB and PDF formats
- **Development Tools**: Extension generator, dev server, testing tools
- **Three Working Extensions**: ScribbleHub, DragonTea, and RoyalRoad

### Recent Development (2024)

**MVP Achievement**
- All core functionality implemented and stable
- Complete CLI interface with comprehensive command set
- Full extension development workflow with tooling
- Reliable novel downloading and management
- Multiple export formats working

**Extension Development Tools**
- Interactive extension generator (`quelle dev generate`)
- Development server with hot reload (`quelle dev server --watch`)
- Extension testing and validation tools
- Publishing system for local and remote stores

**Available Extensions**
- **ScribbleHub**: Original novels and translations
- **DragonTea**: Light novels and web novels  
- **RoyalRoad**: Original fiction and stories

**CLI Commands**
- Complete library management (`quelle library`)
- Extension management (`quelle extensions`)
- Store management (`quelle store`)
- Publishing system (`quelle publish`)
- Development tools (`quelle dev`)
- Export functionality (`quelle export`)

## Development Milestones

### Phase 1: Foundation (Completed)
- [x] Project architecture design
- [x] WASM runtime implementation
- [x] Basic CLI structure
- [x] Extension interface design
- [x] Local store implementation
- [x] Sample extensions

### Phase 2: Core Features (Completed)
- [x] Extension auto-installation
- [x] URL-to-extension matching
- [x] Search functionality
- [x] Multiple output formats (EPUB, PDF)
- [x] Library management
- [x] Enhanced error handling
- [x] Development tooling

### Phase 3: User Experience 🔄 (In Progress)
- [x] Complete CLI interface
- [x] Extension development tools
- [x] Local and Git stores
- [ ] Pre-built binaries
- [ ] Simplified installation packages
- [ ] Extension versioning improvements
- [ ] Configuration profiles

### Phase 4: Advanced Features 📋 (Planned)
- [ ] Web interface
- [ ] Enhanced extension marketplace
- [ ] Automatic binary updates
- [ ] Advanced search filters
- [ ] Cross-platform GUI

## Technical Achievements

**Architecture**
- Rust-based core with WebAssembly extensions
- Wasmtime runtime for secure WASM execution
- Component model for extension interfaces
- Local and Git-based store system

**Development Tools**
- Interactive extension generator
- Development server with hot reload
- Extension validation and testing
- Automated publishing workflows

**Build System**
- Cargo workspace for multiple crates
- `just` for build automation convenience
- `cargo-component` for WASM components
- Cross-platform build support

**Dependencies**
- Wasmtime 37.0 for WASM runtime
- Clap 4.x for CLI interface  
- Tokio for async operations
- Scraper for HTML parsing in extensions

## Current Capabilities

**For Users**
- Stable, feature-complete CLI interface
- Reliable novel downloading and management
- Multiple export formats (EPUB, PDF)
- Extension installation from stores
- Library organization and tracking

**For Developers**
- Complete extension development toolkit
- Extension generator with templates
- Development server with hot reload
- Testing and validation tools
- Local and remote extension stores
- Publishing workflows

## Looking Forward

**Next Priorities**
1. **Distribution**: Pre-built binaries for easier installation
2. **Extensions**: Support for additional novel sites
3. **Performance**: Optimization and caching improvements
4. **Documentation**: Enhanced user guides and tutorials

**Future Features**
- Cross-platform GUI application
- Enhanced extension marketplace
- Advanced search and filtering
- Cloud synchronization options

## Contributing

Quelle welcomes contributions:

- **Extension Development**: Add support for new novel sources
- **Core Features**: Enhance existing functionality
- **Documentation**: Improve user and developer guides
- **Testing**: Help ensure quality and reliability

## Versioning

Quelle currently uses development builds. Formal semantic versioning will begin with the first stable release following MVP completion.

For the latest updates:
- [GitHub Repository](https://github.com/nacht-org/quelle)
- [Extension Registry](https://github.com/nacht-org/extensions)
- [Project Discussions](https://github.com/nacht-org/quelle/discussions)

---

**Status**: MVP Ready - All core features implemented and stable
**Focus**: Expanding extension support and improving distribution