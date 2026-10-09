all: release

check:
	cargo check

test:
	cargo test

release:
	cargo build --release

.PHONY: clean
clean:
	cargo clean
