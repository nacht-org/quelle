# === Core Commands ===

# Build extension
build NAME:
    cargo component build -r -p extension_{{NAME}} --target wasm32-unknown-unknown

# Publish extension to local store
publish NAME STORE="official":
    just build {{NAME}}
    cargo run -p quelle_cli -- publish extension ./target/wasm32-unknown-unknown/release/extension_{{NAME}}.wasm --store {{STORE}} --overwrite

# Set up local store and publish scribblehub
setup:
    cargo run -p quelle_cli -- store remove local --force
    rm -rf ./data
    mkdir -p ./data/stores/local
    cargo run -p quelle_cli -- store add local local
    just publish scribblehub local

# Run CLI with arguments
cli *ARGS:
    cargo run -p quelle_cli -- {{ARGS}}

# === Development Commands ===

# Generate new extension interactively
generate:
    cargo run -p quelle_cli -- dev generate

# Start development server
dev NAME:
    cargo run -p quelle_cli -- dev server {{NAME}} --watch

# Test extension
test NAME *ARGS:
    cargo run -p quelle_cli -- dev test {{NAME}} {{ARGS}}

# Validate extension
validate NAME:
    cargo run -p quelle_cli -- dev validate {{NAME}} --extended

# === Utility Commands ===

# List available extensions
list:
    find extensions -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort

# Show CLI help
help:
    cargo run -p quelle_cli -- --help
