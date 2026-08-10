# TSK-011-02 — Expose the client registration API

| | |
|---|---|
| ID | TSK-011-02 |
| Created | 2026-08-10 |
| Updated | 2026-08-10 |
| Status | Draft planning — do not start until DES-011 is approved |
| Layer | Conductor BE |
| Requirement | [REQ-011](../../requirement/12-REQ-011-client-registration.md) |
| Design | [DES-011 sections 5, 6 and 9](../../design/12-DES-011-client-registration.md#5-api-changes) |
| Covers | AC-2, AC-3, AC-4, AC-5, AC-8, AC-9 |
| Depends on | TSK-011-01 |
| Estimate | 2d |
| Branch | `feat/REQ-011-client-registration` |

## 1. Goal

Expose typed, token-authenticated register and heartbeat endpoints. Each derives identity and policy from
server-owned data and returns safe, actionable errors to EvoFlux.

## 2. Files in scope

| File | Action |
|---|---|
| `crates/conductor-server/src/http/extractors/client_token.rs` | Create reusable `evc_` credential extractor. |
| `crates/conductor-server/src/http/extractors/mod.rs` | Modify exports. |
| `crates/conductor-server/src/http/routes/client.rs` | Create register/heartbeat handlers and DTO mapping. |
| `crates/conductor-server/src/http/routes/resources.rs` | Reuse shared extractor without changing GET semantics. |
| `crates/conductor-server/src/http/routes/mod.rs` | Mount routes. |
| `crates/conductor-server/tests/client_registration_api.rs` | Create HTTP contract/authorisation tests. |

## 3. Implementation steps

1. Extract current bearer validation from resource subscription to a shared extractor; validate prefix,
   hash lookup, expiry, revocation, active owner and `subscribe_resources` scope.
2. Parse registration request, invoke TSK-011-01 storage transaction, and build project/member/policy
   fields from instance/user/role/tag/accepted privacy configuration only.
3. Implement heartbeat as a minimal scoped update; return `404` for missing/not-owned IDs without
   leaking information and return the server-selected interval.
4. Define stable error bodies for validation, authentication, scope, stale state and conflict. Update
   token `last_used_at` only after successful authentication.
5. Mount `/v1/client/*` and prove subscription GET remains read-only with equivalent auth behaviour.

## 4. Required tests

| Type | Tool | Must cover |
|---|---|---|
| HTTP route | `axum` with `tower::ServiceExt::oneshot` | Exact response schema, 400, 401, 403, 404, 409 and repeat-safe 200 heartbeat. |
| Authorisation regression | as above | Active, expired, revoked, disabled-owner, missing-scope and cross-member cases. |
| Bootstrap assembly | `cargo test` | Primary role, sub-roles, tags, branding and collection level present. |

## 5. Commands and reports

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -p conductor-server -p conductor-storage -p conductor-domain
```

## 6. Definition of done

- [ ] Client cannot choose user, role, tags or project workspace.
- [ ] Register is idempotent; heartbeat cannot update another member's installation.
- [ ] Error codes match the EvoFlux actions in DES-011.
- [ ] Existing resource subscription tests remain green.

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-2 | Not run — planning task | — | Pending |
| AC-3 | Not run — planning task | — | Pending |
| AC-4 | Not run — planning task | — | Pending |
| AC-5 | Not run — planning task | — | Pending |
| AC-8 | Not run — planning task | — | Pending |
| AC-9 | Not run — planning task | — | Pending |

### Command output

```text
Not run — planning task.
```

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-10 | Draft planning | Created before design approval at user request |
