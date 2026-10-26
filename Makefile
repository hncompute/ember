# Define the project name (matches your Cargo.toml name)
BINARY_NAME = ember

# -A warnings: Allows (suppresses) all standard compiler warnings
# -A clippy::all: Suppresses all standard Clippy linter suggestions
export RUSTFLAGS = -A warnings
export CLIPPYFLAGS = -A clippy::all

.PHONY: all build run test clean check

all: build

# Standard build without the noise
build:
	cargo build --release

# Run the project
run:
	cargo run --release

# Run tests silently (only shows test failures)
test:
	cargo test -- --quiet

# Check if the code compiles without producing warnings
check:
	cargo check

# Clean build artifacts
clean:
	cargo clean

