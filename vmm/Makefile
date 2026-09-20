all: release

check:
	cargo check

test:
	cargo test

debug:
	RUST_LOG=debug cargo run -- --kernel ./assets/vmlinux.bin --initramfs ./assets/initramfs.cpio

release:
	cargo build --release

.PHONY: clean
clean:
	cargo clean
