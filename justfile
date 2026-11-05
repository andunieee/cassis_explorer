set shell := ["bash", "-uc"]

# Port used by `just dev`.
port := "8080"

# List available recipes.
default:
    @just --list

# Run the app locally with hot reload (default http://localhost:8080).
dev port=port:
    trunk serve --port {{port}}

# Build an optimized bundle into ./dist.
build:
    trunk build --release

# Type-check the wasm target without building the bundle.
check:
    cargo check --target wasm32-unknown-unknown

# Format the Rust sources.
fmt:
    cargo fmt

# Remove build artifacts.
clean:
    rm -rf dist target
