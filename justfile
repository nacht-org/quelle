build-extension NAME:
    cargo component build -r -p extension_{{NAME}} --target wasm32-unknown-unknown

publish-extension NAME:
    just build-extension {{NAME}}
    cargo run -p quelle_cli -- extension publish ./target/wasm32-unknown-unknown/release/extension_{{NAME}}.wasm --store local --overwrite

reset-store:
    just build-extension scribblehub
    rm -rf ./data
    cargo run -p quelle_cli -- store add local ./data/stores/local
