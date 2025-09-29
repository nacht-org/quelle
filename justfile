build-extension NAME:
    cargo component build -r -p extension_{{NAME}} --target wasm32-unknown-unknown

publish NAME:
    just build-extension {{NAME}}
    cargo run -p quelle_cli -- publish extension ./target/wasm32-unknown-unknown/release/extension_{{NAME}}.wasm --store local --overwrite

reset-store:
    rm -rf ./data
    cargo run -p quelle_cli -- store add local ./data/stores/local

run *ARGS:
    cargo run -p quelle_cli -- {{ARGS}}
