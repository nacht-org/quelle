build-extension NAME:
    cargo component build -r -p extension_{{NAME}} --target wasm32-unknown-unknown

publish NAME:
    just build-extension {{NAME}}
    cargo run -p quelle_cli -- publish extension ./target/wasm32-unknown-unknown/release/extension_{{NAME}}.wasm --store local --overwrite

reset-store:
    cargo run -p quelle_cli -- store remove local --force
    rm -rf ./data
    mkdir -p ./data/stores/local
    cargo run -p quelle_cli -- store add local ./data/stores/local

setup:
    just reset-store
    just publish scribblehub

run *ARGS:
    cargo run -p quelle_cli -- {{ARGS}}
