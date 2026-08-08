.PHONY: all setup-data reset-db kill-dev-ports dev-api dev-web dev build-web build check help

API_PORT ?= 4700
WEB_PORT ?= 5174

all: check

setup-data:
	mkdir -p data

# Wipe local state so the next start shows the setup wizard again.
# Stop the API first: SQLite keeps serving a deleted file from an open handle.
reset-db:
	rm -f data/conductor.db data/conductor.db-wal data/conductor.db-shm
	@echo "local database cleared — restart make dev to get the setup wizard"

kill-dev-ports: ## Stop processes listening on :4700 / :5174
	@command -v lsof >/dev/null 2>&1 || { echo "error: 'lsof' not found"; exit 1; }
	@for port in $(API_PORT) $(WEB_PORT); do \
		pids=$$(lsof -tiTCP:$$port -sTCP:LISTEN); \
		if [ -n "$$pids" ]; then \
			echo "stopping processes on port $$port: $$pids"; \
			kill $$pids; \
			for i in 1 2 3 4 5; do \
				sleep 0.2; \
				pids=$$(lsof -tiTCP:$$port -sTCP:LISTEN); \
				[ -z "$$pids" ] && break; \
			done; \
			pids=$$(lsof -tiTCP:$$port -sTCP:LISTEN); \
			if [ -n "$$pids" ]; then \
				echo "force stopping processes on port $$port: $$pids"; \
				kill -9 $$pids; \
			fi; \
		fi; \
	done

dev-api: setup-data ## API only (:4700)
	CONDUCTOR_DATABASE_URL=sqlite:data/conductor.db?mode=rwc \
	CONDUCTOR_HOST=127.0.0.1 \
	CONDUCTOR_PORT=$(API_PORT) \
	cargo run -p conductor-server

dev-web: ## Vite only (:5174, proxies /api → :4700)
	@command -v bun >/dev/null 2>&1 || { echo "error: 'bun' not found — install from https://bun.sh"; exit 1; }
	cd apps/web && bun install && bun run dev -- --port $(WEB_PORT)

dev: kill-dev-ports setup-data ## API + web together (open http://127.0.0.1:5174)
	@command -v bun >/dev/null 2>&1 || { echo "error: 'bun' not found — install from https://bun.sh"; exit 1; }
	@echo "API  http://127.0.0.1:$(API_PORT)"
	@echo "Web  http://127.0.0.1:$(WEB_PORT)  (proxies /api → :$(API_PORT))"
	@trap 'kill 0' INT TERM EXIT; \
	( CONDUCTOR_DATABASE_URL=sqlite:data/conductor.db?mode=rwc \
	  CONDUCTOR_HOST=127.0.0.1 \
	  CONDUCTOR_PORT=$(API_PORT) \
	  cargo run -p conductor-server 2>&1 | sed 's/^/[api] /' ) & \
	( cd apps/web && bun install >/dev/null && bun run dev -- --port $(WEB_PORT) --strictPort 2>&1 | sed 's/^/[web] /' ) & \
	wait

build-web:
	cd apps/web && bun install && bun run build

build: build-web
	cargo build -p conductor-server --release

check:
	cargo check --workspace
	cd apps/web && bun run typecheck

help: ## Show targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-16s\033[0m %s\n", $$1, $$2}'
