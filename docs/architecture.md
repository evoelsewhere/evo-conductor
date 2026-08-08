# Architecture — Evo Conductor

## Layering

```
┌─────────────────────────────────────────────┐
│ apps/web          React feature modules     │
└──────────────────────┬──────────────────────┘
                       │ HTTP /api
┌──────────────────────▼──────────────────────┐
│ conductor-server    http routes + extractors│
└──────────────────────┬──────────────────────┘
         ┌─────────────┼─────────────┐
         ▼             ▼             ▼
 conductor-auth   conductor-storage  (future app services)
         │             │
         └──────┬──────┘
                ▼
        conductor-domain
```

- **domain** — no sqlx/axum; only serde + domain rules
- **storage** — one repo per aggregate (`InstanceRepo`, `UserRepo`, …)
- **auth** — crypto/session primitives reusable by HTTP and future workers
- **server** — transport only; bind config in `config.rs`, app factory in `http::build_router`

## First-run

1. Empty DB → `setup_completed = false`
2. Web gate → `/setup`
3. Persist instance + SSO + default sub-roles
4. Subsequent boots skip setup

## Auth

- Browser: JWT session
- EvoFlux: connection secret `evc_<prefix>_<secret>` (SHA-256 at rest)

## Extension points

- `conductor-application` crate for use-cases if HTTP + workers share logic
- OIDC callback under `http/routes/auth`
- Telemetry ingest via `SecretScope::ReportTelemetry`
