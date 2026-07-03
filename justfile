# justfile — Common development tasks for rmux
#
# Usage:
#   just fmt    — Format all code
#   just lint   — Run clippy with strict warnings
#   just test   — Run all tests
#   just check  — Run fmt + lint + test
#   just doc    — Build and open documentation

# Format all code with rustfmt
fmt:
    cargo fmt --all

# Run clippy with zero-warning tolerance
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run all tests across the workspace
test:
    cargo test --workspace

# Full verification: format, lint, and test
check: fmt lint test
    @echo "All checks passed"

# Build documentation (without dependencies) and open in browser
doc:
    cargo doc --no-deps --workspace --open

# Build only (no docs, no tests)
build:
    cargo build --workspace

# Release build
release:
    cargo build --workspace --release

# Clean build artifacts
clean:
    cargo clean
