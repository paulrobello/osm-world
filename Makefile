run-sacramento:
	cargo run --manifest-path /Users/probello/Repos/osm-world/Cargo.toml -- --input /Users/probello/.cache/par-osm-rust/prepared/7da37d40d6fc8fda7fd51113f7c06c6d5f3c1bfe32b74633bb5e808fe32c0890.osm --spawn-lat 38.5816 --spawn-lon -121.4944 --srtm-dir /Users/probello/.cache/par-osm-rust/srtm

run-woodland:
	cargo run --manifest-path /Users/probello/Repos/osm-world/Cargo.toml -- --input /Users/probello/.cache/par-osm-rust/prepared/6bc9f48dfcde8a8b07942340e536fb11f4401f2dded11d162c51f790205e43fe.osm --spawn-lat 38.67727858898496 --spawn-lon -121.75359597904097 --srtm-dir /Users/probello/.cache/par-osm-rust/srtm

run-woodland-last:
	cargo run --manifest-path /Users/probello/Repos/osm-world/Cargo.toml -- --input /Users/probello/.cache/par-osm-rust/prepared/6bc9f48dfcde8a8b07942340e536fb11f4401f2dded11d162c51f790205e43fe.osm --srtm-dir /Users/probello/.cache/par-osm-rust/srtm

build:
	cargo build

release:
	cargo build --release

run:
	cargo run

run-release:
	cargo run --release

# Launch via a throwaway macOS .app bundle so keyboard/mouse focus works,
# including from inside tmux (raw binaries never become the key window there).
run-app:
	./scripts/run-app.sh

web-install:
	cd web && bun install

web-dev:
	cd web && bun run dev

web-build:
	cd web && bun run build

serve:
	cargo run -- --serve --host 127.0.0.1 --port 3030

dev:
	@bash -c 'set -euo pipefail; \
	TMP_DIR=$$(mktemp -d); API_STATUS="$$TMP_DIR/api.status"; WEB_STATUS="$$TMP_DIR/web.status"; \
	API_PID=""; WEB_PID=""; \
	cleanup() { \
		if [ -n "$$WEB_PID" ]; then kill "$$WEB_PID" 2>/dev/null || true; wait "$$WEB_PID" 2>/dev/null || true; fi; \
		if [ -n "$$API_PID" ]; then kill "$$API_PID" 2>/dev/null || true; wait "$$API_PID" 2>/dev/null || true; fi; \
		rm -rf "$$TMP_DIR"; \
	}; \
	trap cleanup EXIT INT TERM; \
	(set +e; cargo run -- --serve --host 127.0.0.1 --port 3030; STATUS=$$?; echo "$$STATUS" > "$$API_STATUS"; exit "$$STATUS") & \
	API_PID=$$!; \
	for _ in $$(seq 1 60); do \
		if curl -fsS http://127.0.0.1:3030/health >/dev/null 2>&1; then READY=1; break; fi; \
		if [ -f "$$API_STATUS" ]; then echo "osm-world API failed to start"; exit $$(cat "$$API_STATUS"); fi; \
		sleep 0.5; \
	done; \
	if [ "$${READY:-0}" != "1" ]; then echo "osm-world API did not become ready"; exit 1; fi; \
	(cd web && set +e; bun run dev; STATUS=$$?; echo "$$STATUS" > "$$WEB_STATUS"; exit "$$STATUS") & \
	WEB_PID=$$!; \
	while true; do \
		if [ -f "$$API_STATUS" ]; then echo "osm-world API exited"; exit $$(cat "$$API_STATUS"); fi; \
		if [ -f "$$WEB_STATUS" ]; then exit $$(cat "$$WEB_STATUS"); fi; \
		sleep 1; \
	done'

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
