.PHONY: build run test lint fmt typecheck checkall clean release bench doc clippy-fix fmt-fix

build:
	cargo build

release:
	cargo build --release

run:
	cargo run

run-release:
	cargo run --release

test:
	cargo test

bench:
	cargo bench

doc:
	cargo doc --no-deps --open

lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt -- --check

typecheck:
	cargo check --all-targets

clippy-fix:
	cargo clippy --all-targets --all-features --fix --allow-dirty

fmt-fix:
	cargo fmt

checkall: fmt typecheck lint test

clean:
	cargo clean
