set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

build-cli +FLAGS="-r":
    cargo build -p quelle_cli {{FLAGS}}

build-extension NAME *FLAGS: build-cli
    ./target/release/quelle_cli -vv build -e extensions/{{NAME}} {{FLAGS}}

run NAME *FLAGS: build-cli
    ./target/release/quelle_cli -vv run extensions/extension_{{NAME}}.wasm {{FLAGS}}