# syntax=docker/dockerfile:1
#
# Build:  docker build -t evo-conductor .
# Run:    docker run -p 4700:4700 --env-file .env --ulimit nofile=131072:131072 evo-conductor
#
# The process cannot raise its own file-descriptor limit, so --ulimit (or
# docker-compose.yml's `ulimits:`) is required at realtime connection counts.

FROM oven/bun:1 AS web-build
WORKDIR /src/apps/web
COPY apps/web/package.json apps/web/bun.lock ./
RUN bun install --frozen-lockfile
COPY apps/web/ ./
RUN bun run build

FROM rust:1-slim-bookworm AS rust-build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p conductor-server --locked

FROM debian:bookworm-slim AS runtime
# No OpenSSL: sqlx and reqwest build against rustls (Cargo.lock has no
# openssl/native-tls entries), so only certificate roots are needed.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /app --uid 10001 conductor

WORKDIR /app
COPY --from=rust-build /src/target/release/evo-conductor ./evo-conductor
COPY --from=web-build /src/apps/web/dist ./apps/web/dist

ENV CONDUCTOR_HOST=0.0.0.0 \
    CONDUCTOR_PORT=4700 \
    CONDUCTOR_WEB_DIST=/app/apps/web/dist \
    CONDUCTOR_DATA_DIR=/data

RUN mkdir -p /data && chown -R conductor:conductor /app /data
USER conductor

EXPOSE 4700
ENTRYPOINT ["./evo-conductor"]
