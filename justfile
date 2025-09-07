build-extension NAME:
    cargo component build -r -p extension_{{NAME}} --target wasm32-unknown-unknown
