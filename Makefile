.PHONY: all setup-data reset-db kill-dev-ports dev-api dev-web dev dev-tools build-web build check help

API_PORT ?= 4700
WEB_PORT ?= 5174

# Environment the dev API runs with.
API_ENV = CONDUCTOR_DATABASE_URL='sqlite:data/conductor.db?mode=rwc' \
	  CONDUCTOR_HOST=127.0.0.1 \
	  CONDUCTOR_PORT=$(API_PORT)

# Rebuild and restart the API when a Rust source changes, when cargo-watch is
# available; fall back to a plain run so `make dev` still works without it.
#
# Only `crates/` and the workspace manifest are watched. Watching everything
# would include `data/`, where SQLite writes -wal and -shm files while serving,
# and each write would trigger another restart — an endless loop.
#
# `-d 1` coalesces filesystem events: saving one file emits both a write and a
# rename on macOS, and without the delay cargo-watch fires twice — the first run
# binds the port and the second fails with "Address already in use".
#
# `build && exec` rather than `cargo run`: with `cargo run` the server is a
# grandchild of cargo-watch, the restart signal reaches cargo instead of the
# server, and the old process can outlive the new one's attempt to bind. The
# result is an intermittent "Address already in use" and, worse, the previous
# build continuing to serve as though the reload had worked. `exec` replaces the
# shell with the server itself, so the signal has a single, correct target.
RUN_API = if command -v cargo-watch >/dev/null 2>&1; then \
	    cargo watch -q -d 1 -w crates -w Cargo.toml \
	      -s 'cargo build -q -p conductor-server && exec target/debug/evo-conductor'; \
	  else \
	    echo "cargo-watch not installed — the API will not reload. Run: make dev-tools"; \
	    cargo run -p conductor-server; \
	  fi

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

dev-api: setup-data ## API only (:4700), reloads on Rust changes
	@export $(API_ENV); $(RUN_API)

dev-web: ## Vite only (:5174, proxies /api → :4700)
	@command -v bun >/dev/null 2>&1 || { echo "error: 'bun' not found — install from https://bun.sh"; exit 1; }
	cd apps/web && bun install && bun run dev -- --port $(WEB_PORT)

dev: kill-dev-ports setup-data ## API + web together, both reloading (open http://127.0.0.1:5174)
	@command -v bun >/dev/null 2>&1 || { echo "error: 'bun' not found — install from https://bun.sh"; exit 1; }
	@echo "API  http://127.0.0.1:$(API_PORT)"
	@echo "Web  http://127.0.0.1:$(WEB_PORT)  (proxies /api → :$(API_PORT))"
	@trap 'kill 0' INT TERM EXIT; \
	( export $(API_ENV); $(RUN_API) 2>&1 | sed 's/^/[api] /' ) & \
	( cd apps/web && bun install >/dev/null && bun run dev -- --port $(WEB_PORT) --strictPort 2>&1 | sed 's/^/[web] /' ) & \
	wait

dev-tools: ## Install cargo-watch, which gives the API hot reload
	cargo install cargo-watch --locked

build-web:
	cd apps/web && bun install && bun run build

build: build-web
	cargo build -p conductor-server --release

check:
	cargo check --workspace
	cd apps/web && bun run typecheck

help: ## Show targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-16s\033[0m %s\n", $$1, $$2}'
