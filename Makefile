.PHONY: build run test lint fmt typecheck checkall clean

build:
	cargo build

run:
	cargo run

test:
	cargo test

lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt -- --check

typecheck:
	cargo check

checkall: fmt typecheck lint test

clean:
	cargo clean
