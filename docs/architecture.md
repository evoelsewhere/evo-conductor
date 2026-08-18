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

- Browser: 24-hour JWT session with issuer/audience validation and a database-backed session version
- OIDC: authorization code + PKCE, state, nonce, JWKS signature, issuer, and audience validation
- SSO identity binding: immutable provider `issuer + subject`; email is used only for initial linking/provisioning
- Authorization: every request reloads current user status and role from storage; UI route guards are convenience only
- EvoFlux: connection secret `evc_<prefix>_<secret>` (SHA-256 at rest)
- Private resources are visible only to their owner; disabled owners cannot use existing connection secrets

## Extension points

- `conductor-application` crate for use-cases if HTTP + workers share logic
- OIDC callback under `http/routes/auth`
- Telemetry ingest via `SecretScope::ReportTelemetry`

## EvoFlux realtime control plane

- Transport: authenticated SSE at `/api/v1/realtime/events` is an invalidation channel; authenticated smart HTTP at `POST /api/v1/resources/fetch` is the authoritative data plane
- Fan-out: one bounded Tokio broadcast ring per Conductor process, not one polling task per client
- Backpressure: a lagging receiver is instructed to smart-fetch the current member-specific commit; no file payload is buffered in SSE
- Lifecycle: heartbeat, token-expiry close, secret-revocation close, member-disable close, and server-drain signal
- Admission control: configurable global and per-secret semaphores; overload returns `503` or `429` with `Retry-After`
- Presence: the dashboard derives recently seen clients and members from project-scoped installation heartbeats; the current threshold is three missed 60-second heartbeats (180 seconds)
- Live runtime: active SSE owners and streams are reported separately and are explicitly scoped to the current Conductor process
- Host metrics: CPU and RAM describe the Conductor host only; unsupported CPU warm-up, GPU, and VRAM values remain null instead of being fabricated as zero
- Feedback: administrators receive a project aggregate while contributors receive only the aggregate for resources they own; dashboard feedback never includes member identity or comments
- Horizontal scale boundary: the current hub is single-process. Multiple replicas require a shared broker plus transactional outbox; PostgreSQL and NATS JetStream are the recommended production path

The complete client contract and scale-out design are in [evoflux-integration.md](evoflux-integration.md). Git-style object negotiation and atomic checkout are normative in [resource-fetch-protocol.md](resource-fetch-protocol.md).

## Governed resource catalog

- `resources` is the stable catalog identity and caches the currently published payload for fast subscriptions
- `resource_versions` stores draft, published and deprecated immutable payload versions
- `resource_access_rules` stores allow subjects resolved from primary role, sub-role, tag or member
- `resource_usage_events` is an idempotent operational event log keyed by EvoFlux event ID
- `resource_feedback` stores one current response per resource/member
- Publishing commits storage first, then emits a realtime head invalidation; clients converge by negotiating the authoritative desired commit
- Usage identity is derived from the reporting connection secret; monitoring never trusts a client-provided member ID

Product behavior and permission decisions are documented in [resource-catalog-product.md](resource-catalog-product.md).
