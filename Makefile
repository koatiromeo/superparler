.PHONY: dev build test test-rust test-front lint fmt fmt-check clean models setup \
        front-dev front-build ci release run-release doc

# Default target
.DEFAULT_GOAL := dev

# ─── Development ───────────────────────────────────────────────
dev:
	cargo tauri dev

front-dev:
	cd src && npm run dev

front-build:
	cd src && npm run build

# ─── Build & Release ───────────────────────────────────────────
build: front-build
	cargo tauri build

release: build

run-release:
	cargo tauri build --debug && ./src-tauri/target/debug/superparler

# ─── Tests ─────────────────────────────────────────────────────
test: test-rust test-front

test-rust:
	cargo test --manifest-path src-tauri/Cargo.toml --lib

test-front:
	cd src && npm run typecheck && npm run lint

# ─── Code Quality ──────────────────────────────────────────────
lint:
	cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

fmt:
	cargo fmt --manifest-path src-tauri/Cargo.toml

fmt-check:
	cargo fmt --manifest-path src-tauri/Cargo.toml --check

# ─── CI (local simulation) ────────────────────────────────────
ci: fmt-check lint test

# ─── Documentation ─────────────────────────────────────────────
doc:
	cargo doc --manifest-path src-tauri/Cargo.toml --open --no-deps

# ─── Setup & Models ────────────────────────────────────────────
setup:
	@bash scripts/setup.sh

models:
	@bash scripts/download-models.sh

# ─── Clean ─────────────────────────────────────────────────────
clean:
	cargo clean --manifest-path src-tauri/Cargo.toml
	rm -rf src/dist src/.vite
	@echo "Cleaned build artifacts (models preserved)"
