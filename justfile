set shell := ["bash", "-uc"]

# Port used by `just dev`.
port := "8080"

# List available recipes.
default:
    @just --list

# Check out the cassis sibling repository if it is missing.
setup:
    if [ ! -d ../cassis ]; then git clone https://github.com/andunieee/cassis.git ../cassis; fi

# Run the app locally with hot reload (default http://localhost:8080).
dev port=port: setup
    trunk serve --port {{port}}

# Build an optimized bundle into ./dist.
build: setup
    trunk build --release

# Type-check the wasm target without building the bundle.
check: setup
    cargo check --target wasm32-unknown-unknown

# Format the Rust sources.
fmt:
    cargo fmt

# Remove build artifacts.
clean:
    rm -rf dist target
