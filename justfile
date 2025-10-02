# === Core Development Commands ===

build-extension NAME:
    cargo component build -r -p extension_{{NAME}} --target wasm32-unknown-unknown

publish NAME:
    just build-extension {{NAME}}
    cargo run -p quelle_cli -- publish extension ./target/wasm32-unknown-unknown/release/extension_{{NAME}}.wasm --store local --overwrite

publish-remote NAME:
    just build-extension {{NAME}}
    cargo run -p quelle_cli -- publish extension ./target/wasm32-unknown-unknown/release/extension_{{NAME}}.wasm --store remote --overwrite

reset-store:
    cargo run -p quelle_cli -- store remove local --force
    rm -rf ./data
    mkdir -p ./data/stores/local
    cargo run -p quelle_cli -- store add local local

setup:
    just reset-store
    just publish scribblehub

# === Extension Development Commands ===

# Start development server with hot reload for an extension
dev-server NAME:
    cargo run -p quelle_cli -- dev server {{NAME}} --watch --verbose

# Quick interactive test for extension functionality
dev-test NAME *ARGS:
    cargo run -p quelle_cli -- dev test {{NAME}} {{ARGS}} --verbose

# Validate extension structure and build
dev-validate NAME:
    cargo run -p quelle_cli -- dev validate {{NAME}} --extended

# Test novel info fetching with a specific URL
dev-test-novel NAME URL:
    cargo run -p quelle_cli -- dev test {{NAME}} --url {{URL}}

# Test search functionality with a query
dev-test-search NAME QUERY:
    cargo run -p quelle_cli -- dev test {{NAME}} --query "{{QUERY}}"

# Build and hot-reload development cycle
dev-quick NAME:
    just build-extension {{NAME}} && just dev-test {{NAME}}

# === Legacy Publishing Commands ===

# Publish extension using the helper script
publish-script NAME *ARGS:
    ./scripts/publish-extension.sh {{ARGS}} {{NAME}}

# Publish extension with overwrite (common for development)
publish-dev NAME:
    ./scripts/publish-extension.sh --store local --overwrite {{NAME}}

# Test build extension without publishing (dry run)
test-extension NAME:
    ./scripts/publish-extension.sh --dry-run {{NAME}}

# === Utility Commands ===

# Show available extensions
list-extensions:
    find extensions -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort

# Show publishing help
publish-help:
    ./scripts/publish-extension.sh --help

# Run CLI with arguments
run *ARGS:
    cargo run -p quelle_cli -- {{ARGS}}

# === Development Workflow Examples ===

# Example: Full development workflow for scribblehub extension
example-dev-scribblehub:
    @echo "🚀 Starting development server for scribblehub extension..."
    @echo "💡 This will:"
    @echo "   1. Build the extension"
    @echo "   2. Start file watching for auto-rebuild"
    @echo "   3. Provide interactive testing commands"
    @echo ""
    @echo "Available commands in dev server:"
    @echo "  test <url>     - Test novel info fetching"
    @echo "  search <query> - Test search functionality"
    @echo "  chapter <url>  - Test chapter content fetching"
    @echo "  meta          - Show extension metadata"
    @echo "  rebuild       - Force rebuild extension"
    @echo "  quit          - Exit development server"
    @echo ""
    just dev-server scribblehub

# Example: Quick test of a novel URL
example-test-novel:
    @echo "🧪 Testing novel info fetch..."
    just dev-test-novel scribblehub "https://www.scribblehub.com/series/123456/example-novel/"

# Example: Quick search test
example-test-search:
    @echo "🔍 Testing search functionality..."
    just dev-test-search scribblehub "fantasy adventure"
