run-sacramento:
	cargo run -- --input ../osm-to-bedrock/map_exports/planet_-121.7526,38.63863_-121.72179,38.65671.osm.pbf --srtm-dir ~/.cache/par-osm-rust/srtm

build:
	cargo build

release:
	cargo build --release

run:
	cargo run

run-release:
	cargo run --release

web-install:
	cd web && bun install

web-dev:
	cd web && bun run dev

web-build:
	cd web && bun run build

serve:
	cargo run -- --serve --host 127.0.0.1 --port 3030

dev:
	@set -e; \
	cargo run -- --serve --host 127.0.0.1 --port 3030 & \
	API_PID=$$!; \
	trap 'kill $$API_PID 2>/dev/null || true' INT TERM EXIT; \
	sleep 1; \
	if ! kill -0 $$API_PID 2>/dev/null; then echo "osm-world API failed to start"; exit 1; fi; \
	cd web && bun run dev

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
