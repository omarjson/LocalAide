.PHONY: all check fmt test lint build smoke release clean

# The same gates CI runs (.github/workflows/ci.yml), in the same order.
all: check fmt lint test smoke

check:
	npm run check
	cd src-tauri && cargo check --all-targets

fmt:
	cd src-tauri && cargo fmt --check

test:
	cd src-tauri && cargo test --all-targets
	npm run test:unit

lint:
	cd src-tauri && cargo clippy --all-targets -- -D warnings

build:
	npm run build

smoke: build
	node scripts/smoke-test.mjs

# The app downloads its model engine (llama-server) on first use, so the
# package needs only THIRD_PARTY_LICENSES.html:
#   cd src-tauri && cargo about generate about.hbs > ../THIRD_PARTY_LICENSES.html
release: build
	npx tauri build --config src-tauri/tauri.release.conf.json

clean:
	rm -rf dist src-tauri/target
