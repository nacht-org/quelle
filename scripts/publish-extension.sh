#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Default values
STORE="official"
OVERWRITE=""
DRY_RUN=""
NOTES=""
TAGS=""

usage() {
    cat << EOF
Usage: $0 [OPTIONS] <extension_name>

Publish an extension to the specified store.

Options:
    -s, --store STORE       Store to publish to (default: official)
    -o, --overwrite         Overwrite existing version
    -d, --dry-run          Build but don't publish
    -n, --notes NOTES      Publication notes
    -t, --tags TAGS        Comma-separated tags
    -h, --help             Show this help message

Examples:
    $0 scribblehub
    $0 -s local -o dragontea
    $0 --dry-run --notes "Testing new feature" scribblehub
    $0 --store official --overwrite --tags "manga,novels" scribblehub

Available extensions:
EOF
    if [ -d "$PROJECT_ROOT/extensions" ]; then
        find "$PROJECT_ROOT/extensions" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort | sed 's/^/    /'
    fi
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_requirements() {
    log_info "Checking requirements..."

    # Check if we're in the right directory
    if [ ! -f "$PROJECT_ROOT/Cargo.toml" ] || [ ! -d "$PROJECT_ROOT/extensions" ]; then
        log_error "This script must be run from the Quelle project root or scripts directory"
        exit 1
    fi

    # Check if cargo-component is installed
    if ! command -v cargo-component &> /dev/null; then
        log_error "cargo-component is not installed. Please install it with:"
        echo "  curl -L https://github.com/bytecodealliance/cargo-component/releases/latest/download/cargo-component-x86_64-unknown-linux-gnu.tar.gz | tar xzf -"
        echo "  sudo mv cargo-component /usr/local/bin/"
        exit 1
    fi

    # Check if wasm32-unknown-unknown target is installed
    if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
        log_warning "wasm32-unknown-unknown target not found, installing..."
        rustup target add wasm32-unknown-unknown
    fi

    log_success "Requirements check passed"
}

build_extension() {
    local extension_name="$1"

    log_info "Building extension: $extension_name"

    cd "$PROJECT_ROOT"

    # Verify extension exists
    if [ ! -d "extensions/$extension_name" ]; then
        log_error "Extension '$extension_name' not found in extensions directory"
        return 1
    fi

    if [ ! -f "extensions/$extension_name/Cargo.toml" ]; then
        log_error "Extension '$extension_name' missing Cargo.toml"
        return 1
    fi

    # Build the extension
    cargo component build -r -p "extension_$extension_name" --target wasm32-unknown-unknown

    # Verify build output
    local wasm_file="./target/wasm32-unknown-unknown/release/extension_$extension_name.wasm"
    if [ ! -f "$wasm_file" ]; then
        log_error "Built WASM file not found at $wasm_file"
        return 1
    fi

    local file_size=$(wc -c < "$wasm_file")
    log_success "Built extension successfully (size: $file_size bytes)"
    echo "$wasm_file"
}

publish_extension() {
    local extension_name="$1"
    local wasm_file="$2"

    if [ -n "$DRY_RUN" ]; then
        log_info "DRY RUN: Would publish $extension_name to store '$STORE'"
        return 0
    fi

    log_info "Publishing extension '$extension_name' to store '$STORE'..."

    cd "$PROJECT_ROOT"

    local cmd=(cargo run -p quelle_cli -- publish extension "$wasm_file" --store "$STORE")

    if [ -n "$OVERWRITE" ]; then
        cmd+=(--overwrite)
    fi

    if [ -n "$NOTES" ]; then
        cmd+=(--notes "$NOTES")
    fi

    if [ -n "$TAGS" ]; then
        cmd+=(--tags "$TAGS")
    fi

    # Add timeout for network operations
    cmd+=(--timeout 300)

    log_info "Running: ${cmd[*]}"

    if "${cmd[@]}"; then
        log_success "Extension '$extension_name' published successfully to '$STORE'"
        return 0
    else
        log_error "Failed to publish extension '$extension_name'"
        return 1
    fi
}

main() {
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -s|--store)
                STORE="$2"
                shift 2
                ;;
            -o|--overwrite)
                OVERWRITE="1"
                shift
                ;;
            -d|--dry-run)
                DRY_RUN="1"
                shift
                ;;
            -n|--notes)
                NOTES="$2"
                shift 2
                ;;
            -t|--tags)
                TAGS="$2"
                shift 2
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            -*)
                log_error "Unknown option: $1"
                usage
                exit 1
                ;;
            *)
                if [ -z "${EXTENSION_NAME:-}" ]; then
                    EXTENSION_NAME="$1"
                else
                    log_error "Multiple extension names provided: '$EXTENSION_NAME' and '$1'"
                    usage
                    exit 1
                fi
                shift
                ;;
        esac
    done

    # Validate arguments
    if [ -z "${EXTENSION_NAME:-}" ]; then
        log_error "Extension name is required"
        usage
        exit 1
    fi

    log_info "Publishing extension '$EXTENSION_NAME' to store '$STORE'"
    if [ -n "$DRY_RUN" ]; then
        log_warning "DRY RUN mode enabled - will not actually publish"
    fi

    # Check requirements
    check_requirements

    # Build extension
    local wasm_file
    if ! wasm_file=$(build_extension "$EXTENSION_NAME"); then
        log_error "Failed to build extension '$EXTENSION_NAME'"
        exit 1
    fi

    # Publish extension
    if ! publish_extension "$EXTENSION_NAME" "$wasm_file"; then
        exit 1
    fi

    log_success "Operation completed successfully!"
}

main "$@"
