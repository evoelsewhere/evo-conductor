---
name: conductor-backend
description: Use when implementing, fixing, or refactoring Rust code in Evo Conductor under crates/ — domain types, sqlx repositories, Argon2/JWT/OIDC auth, or Axum HTTP routes and extractors. Covers the layering rules, the portable-SQL constraint, error mapping, and the authorization pattern. Trigger on "add an endpoint", "add a table", "change the API", "sửa backend conductor", "thêm route cho conductor".
---

# Conductor backend — Rust, Axum, sqlx

Four crates, strictly layered. A change that reaches across a layer boundary in the wrong direction is a
design error, not a shortcut.

```
conductor-domain      entities, roles, DTOs, errors    no sqlx, no axum
      ^
conductor-storage     Db + one repo per aggregate      sqlx only
      ^
conductor-auth        Argon2, JWT, OIDC, evc_ tokens   reusable by HTTP and future workers
      ^
conductor-server      config + http::{routes, extractors, state} + thin main
```

`conductor-domain` must not gain a `sqlx` or `axum` dependency. If a domain type needs to be built from a
database row, the conversion belongs in `conductor-storage/src/mapping.rs`.

## Adding an endpoint, end to end

Work bottom-up. Each step compiles on its own.

**1. Domain** — add the request and response types in `conductor-domain/src/<aggregate>.rs`, re-export
from `lib.rs`. Serde only. Field names are `snake_case` on the wire.

**2. Storage** — add a method to the matching repo in `conductor-storage/src/repos/`. One repo per
aggregate: `instance`, `user`, `role`, `secret`, `resource`, `dashboard`. Row-to-domain conversion goes
through `mapping.rs` helpers such as `parse_dt`.

**3. Route** — add a handler in `conductor-server/src/http/routes/<group>.rs` and register it in the
single `router()` in `routes/mod.rs`. Handlers are thin: extract, authorize, delegate, map.

**4. Frontend types** — mirror the wire types in `apps/web/src/shared/api/client.ts`. See
`conductor-frontend`.

## Authorization — the pattern, and the mistake to avoid

`AuthUser` is the session extractor
(`conductor-server/src/http/extractors/auth_user.rs`). It verifies a `Bearer` JWT, loads the user, and
admits only `UserStatus::Active`; `Disabled`, `Pending` and `Invited` are rejected with `Forbidden`.

**`AuthUser` proves identity, not permission.** Every handler that exposes project-wide data or performs
a privileged action must additionally check the role:

```rust
pub async fn handler(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<T>> {
    if !user.primary_role.can_view_telemetry() {
        return Err(ConductorError::Forbidden.into());
    }
    // ...
}
```

The predicates live in `conductor-domain/src/role.rs`: `can_manage_members`, `can_list_members`,
`can_manage_resources`, `can_manage_tags`, `can_view_telemetry`, `can_manage_settings`.

`users.rs`, `access.rs` and `settings.rs` follow this pattern correctly. `dashboard.rs` and
`resources.rs` do **not** — they accept any authenticated session. `can_view_telemetry()` is defined and
never called anywhere in the codebase. Do not copy those two files as a template; REQ-004 fixes them.

## Token-authenticated routes are a different family

EvoFlux authenticates with an `evc_` connection token, not a JWT, and those handlers read the
`Authorization` header directly rather than using `AuthUser` — see `routes/resources.rs::subscribe`. Such
a handler must check, in this order:

1. the header starts with `evc_`,
2. `hash_token` matches a stored `token_hash`,
3. the token is not expired and not revoked,
4. the token carries the required `SecretScope`,
5. **the owner is still `active`** — this check is currently missing and is what REQ-005 adds.

When adding the `/api/v1/client/*` family, keep it structurally separate from session routes. The two
have different authentication, different error semantics and very different call frequency.

## Errors

`ConductorError` (`conductor-domain/src/error.rs`) maps to HTTP status in `status_code()`:

| Variant | Status |
|---|---|
| `NotFound(_)` | 404 |
| `Unauthorized`, `InvalidCredentials` | 401 |
| `Forbidden` | 403 |
| `Conflict(_)`, `SetupAlreadyCompleted` | 409 |
| `SetupRequired` | 428 |
| `Message(_)`, `Other(_)` | 400 |

Handlers return `ApiResult<T>`. `ApiError` converts from both `ConductorError` and `sqlx::Error`, so `?`
works throughout. The response body is `{"error": "...", "code": <status>}`.

`From<sqlx::Error>` collapses database errors into a 400 with the raw message. Be careful not to leak
constraint details to a client through that path; map expected failures explicitly to `Conflict` or
`NotFound` first.

## SQL must stay portable

Storage uses `sqlx` with the `Any` driver so one binary serves SQLite, PostgreSQL and MySQL, selected by
`CONDUCTOR_DATABASE_URL` and detected by `DatabaseKind::detect`
(`conductor-storage/src/dialect.rs`). Production targets PostgreSQL; SQLite is for development.

Constraints that follow:

- Identifiers are `TEXT` holding a UUID string, not a native UUID type.
- Booleans are `INTEGER` 0 or 1, compared explicitly (`== 1`), never as a native bool.
- Timestamps are RFC 3339 `TEXT`, parsed via `mapping::parse_dt`.
- No dialect-specific syntax: no `RETURNING`, no `ON CONFLICT`, no `SERIAL`, no `JSONB` operators.
- JSON payloads are stored as `TEXT` and parsed with `serde_json`.

**Verify before relying on it:** existing queries use `?` placeholders throughout. Whether the `Any`
driver rewrites those for PostgreSQL has not yet been exercised — nothing in this repo has ever run
against PostgreSQL. REQ-001 AC-5 is the first time it will. Treat a PostgreSQL run as a real test, not a
formality.

## Migrations

`conductor-storage/src/migrate.rs` currently runs an array of `CREATE TABLE IF NOT EXISTS` statements
followed by `ALTER TABLE` statements whose errors are **discarded with `let _ = ...`**. There is no
`schema_version` table, so the system cannot report what has been applied and a failed migration is
indistinguishable from a successful one.

Do not add tables to this mechanism. REQ-001 replaces it with versioned migrations, and seven new tables
are queued behind it: `client_installations`, `client_heartbeats`, `resource_versions`,
`resource_access_policies`, `resource_sync_state`, `documents`, `usage_aggregates`, `audit_events`, plus
`model_pricing` and `server_config`.

## Secrets

- Passwords: Argon2 via `conductor-auth/src/password.rs`.
- Connection tokens: generated with `generate_connection_token`, stored as a SHA-256 hash
  (`conductor-auth/src/secret_token.rs`), returned in cleartext exactly once at creation.
- OIDC client secret: **currently plaintext** in a column named `client_secret_enc`. It must be
  recoverable for token exchange (`InstanceRepo::sso_runtime`), so it needs symmetric encryption rather
  than hashing. REQ-002. Never widen this path; never log the value; API responses expose only the
  `client_secret_set` flag.

## State

`AppState` (`http/state.rs`) holds the `Db`, an `Arc<RwLock<Option<JwtService>>>` and a `DashMap` of
pending OIDC exchanges with a 600-second TTL. The JWT secret is read once at startup from the `instance`
row; REQ-003 moves it to a `server_config` table. Handlers get state via `State(state): State<AppState>`.

## Tests

None exist yet; REQ-020 introduces the harness. When it does, a backend change carries:

- unit tests in `conductor-domain` for rules, parsing and boundaries,
- repository tests against `sqlite::memory:`, including that migrations apply cleanly to an empty database,
- HTTP tests with `axum::Router` and `tower::ServiceExt::oneshot`, asserting status, body shape, and
  **every authorization branch for all three roles**.

## Commands

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --release
make dev          # API on :4700, Vite on :5174
make reset-db     # fresh setup wizard
```
