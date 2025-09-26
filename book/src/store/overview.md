# Store Overview

The store system is the heart of Quelle's extension management, providing a comprehensive package management solution for discovering, installing, and maintaining extensions. It offers a flexible, trait-based architecture that supports multiple backend types while maintaining a consistent interface.

## What is a Store?

A store is a repository of extensions that Quelle can access to discover, download, and install new functionality. Think of stores like package repositories in other systems (npm registry, apt repositories, etc.), but specifically designed for Quelle's WebAssembly-based extensions.

## Store Types

Quelle supports multiple types of stores, each suited for different use cases:

### Local Stores
File system-based stores perfect for:
- Development and testing
- Private or proprietary extensions
- Offline usage
- Custom organizational needs

### Git Stores *(Coming Soon)*
Git repository-based stores ideal for:
- Collaborative development
- Version control integration
- Distributed extension sharing
- Community-driven repositories

### HTTP Stores *(Coming Soon)*
Web-based registries suitable for:
- Centralized extension distribution
- Large-scale public repositories
- API-driven extension management
- Integration with existing systems

## Key Capabilities

- **Multi-Store Management**: Work with multiple stores simultaneously, with priority-based resolution
- **Version Management**: Full semantic versioning support with intelligent conflict resolution
- **Search and Discovery**: Advanced search across all configured stores with filtering and sorting
- **Package Management**: Complete lifecycle management - install, update, remove, and rollback
- **Security**: Checksum verification, trusted store marking, and integrity validation
- **Performance**: Intelligent caching, parallel operations, and efficient data structures
- **Reliability**: Comprehensive error handling, health monitoring, and recovery mechanisms

## Store Manager

The `StoreManager` is the central component that coordinates multiple stores and manages the local extension registry. It provides:

- **Unified Interface**: Single API to work with multiple store types
- **Smart Resolution**: Automatically resolves conflicts using store priorities and trust levels
- **Local Registry**: Tracks installed extensions with metadata and dependencies
- **Update Management**: Checks for and applies updates across all configured stores
- **Health Monitoring**: Monitors store availability and performance

## Extension Lifecycle

Extensions in Quelle follow a well-defined lifecycle:

1. **Discovery**: Find extensions through search or browsing
2. **Installation**: Download and install with dependency resolution
3. **Usage**: Load and execute extensions in the Quelle engine
4. **Updates**: Check for and apply updates when available
5. **Removal**: Clean uninstall with dependency cleanup

## Getting Started

To get started with the store system:

1. **Store Management**: Learn how to [add and configure stores](./management.md)
2. **Extension Management**: Understand [extension installation and updates](./extensions.md)
3. **Search**: Explore [search and discovery features](./search.md)
4. **Configuration**: Set up [advanced configuration options](./configuration.md)
5. **CLI Reference**: Master the [command-line interface](./cli-reference.md)

For developers interested in the technical details, see the [API Reference](../development/api-reference.md) and [Store Development](../development/store-development.md) guides.

## Architecture Highlights

The store system is built around several key architectural principles:

### Trait-Based Design
All stores implement a common `Store` trait, ensuring consistent behavior regardless of the underlying storage mechanism. This allows the system to work with local files, remote repositories, or cloud storage transparently.

### Async-First Architecture
Every operation is designed with async/await in mind, ensuring the system remains responsive even when working with slow network connections or large extension repositories.

### Flexible Package Layouts
Each store can define its own file organization structure, allowing adaptation to existing repositories without requiring reorganization.

### Security by Default
All packages are verified with SHA256 checksums, and stores can be marked as trusted to establish a security hierarchy.

For detailed technical information about the architecture, see the [Development section](../development/architecture.md) of this book.