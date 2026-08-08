.PHONY: dev dev-api dev-web build build-web check setup-data

setup-data:
	mkdir -p data

dev-api: setup-data
	CONDUCTOR_DATABASE_URL=sqlite:data/conductor.db?mode=rwc \
	CONDUCTOR_HOST=127.0.0.1 \
	CONDUCTOR_PORT=4700 \
	cargo run -p conductor-server

dev-web:
	cd apps/web && bun install && bun run dev

dev:
	@echo "Start API:  make dev-api"
	@echo "Start Web:  make dev-web"
	@echo "Then open http://127.0.0.1:5174 (proxies /api → :4700)"

build-web:
	cd apps/web && bun install && bun run build

build: build-web
	cargo build -p conductor-server --release

check:
	cargo check --workspace
	cd apps/web && bun run typecheck
