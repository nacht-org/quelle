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

# Publish extension using the helper script
publish-script NAME *ARGS:
    ./scripts/publish-extension.sh {{ARGS}} {{NAME}}

# Publish extension with overwrite (common for development)
publish-dev NAME:
    ./scripts/publish-extension.sh --store local --overwrite {{NAME}}

# Test build extension without publishing (dry run)
test-extension NAME:
    ./scripts/publish-extension.sh --dry-run {{NAME}}

# Show available extensions
list-extensions:
    find extensions -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort

# Show publishing help
publish-help:
    ./scripts/publish-extension.sh --help

run *ARGS:
    cargo run -p quelle_cli -- {{ARGS}}
