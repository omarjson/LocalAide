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

release: build
	npx tauri build

clean:
	rm -rf dist src-tauri/target
