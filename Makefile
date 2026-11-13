.PHONY: all check test lint build smoke release clean

all: check test smoke

check:
	npm run check
	cd src-tauri && cargo check --all-targets

test:
	cd src-tauri && cargo test --all-targets

lint:
	cd src-tauri && cargo clippy --all-targets

build:
	npm run build

smoke: build
	node scripts/smoke-test.mjs

# The packaged app needs llama-server next to the binary to chat. Stage it
# first (see PACKAGING.md: build llama.cpp from the pinned tag, then run
# scripts/prepare-sidecar.sh), or rely on CI which does both.
release: build
	npx tauri build --config src-tauri/tauri.sidecar.conf.json

clean:
	rm -rf dist src-tauri/target
