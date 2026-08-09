# REQ-006 — Connection tokens and scopes

| | |
|---|---|
| ID | REQ-006 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Build order | Step 8 of 23 |
| Spec section | [requirements.md section 5](../requirements.md) |
| Source | Baseline specification section 5 |
| Depends on | REQ-004, REQ-005 |
| Blocks | REQ-011, REQ-012, REQ-013, REQ-014 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

The connection token is the only credential linking an EvoFlux installation to the project. It authorizes
resource downloads, telemetry uploads and inventory reporting, so its scope model and its revocation
behaviour define the security boundary of the whole integration.

Most of the token mechanism is implemented correctly. Two defects undermine it, and one required scope
does not yet exist.

## 2. Requirement

Conductor shall issue scoped connection tokens that are displayed once, hashed at rest, expirable,
revocable, constrained by the issuing member's role, and rejected when their owner is disabled. EvoFlux
shall store the token in the host operating system's secure credential store.

Supported scopes shall include `subscribe_resources`, `report_telemetry`, `sync_inventory` and
`read_documents`.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Token generation with `evc_` prefix, one-time display, SHA-256 hash at rest ([secret_token.rs](../../crates/conductor-auth/src/secret_token.rs), [secrets.rs](../../crates/conductor-server/src/http/routes/secrets.rs)) | `read_documents` scope | `POST /api/secrets` performs no role check |
| Scope list, expiry column, revocation endpoint | Default expiry policy | Omitting `scopes` grants all three existing scopes by default ([secrets.rs:31-38](../../crates/conductor-server/src/http/routes/secrets.rs)) |
| Expiry checked during validation ([resources.rs:39-44](../../crates/conductor-server/src/http/routes/resources.rs)) | Expiry warning | Owner status not checked during validation, see [REQ-005](07-REQ-005-member-lifecycle.md) |
| `last_used_at` column | Client-side secure storage | `last_used_at` is never updated |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Token creation requires an explicit non-empty scope list; omitting it returns `400` |
| AC-2 | The `read_documents` scope exists and gates the document endpoints in [REQ-009](18-REQ-009-document-management.md) |
| AC-3 | A token is displayed exactly once at creation and is never retrievable afterwards |
| AC-4 | Tokens carry a default expiry, configurable by an Admin; the default is applied when the caller does not specify one |
| AC-5 | A token whose owner is disabled is rejected with `401` |
| AC-6 | Revocation takes effect on the next request, with no caching window |
| AC-7 | `last_used_at` is updated on each successful token-authenticated request |
| AC-8 | Members and Admins can list live tokens with owner, scopes, last use and expiry |
| AC-9 | EvoFlux stores the token in the operating system credential store, never in a plaintext configuration file, and never writes it to a log |
| AC-10 | Members are warned before a token expires |

## 5. Out of scope

- Automatic token rotation. Reconsider at P2.
- Machine-bound tokens tied to a specific installation. Installation identity is handled separately in
  [REQ-013](14-REQ-013-inventory-synchronization.md).

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | An ordinary User mints a fully privileged token | High | AC-1 plus the role constraint in [REQ-004](02-REQ-004-api-authorization.md) |
| 2 | Tokens expiring together interrupt the whole team | Medium | AC-10 |
| 3 | Token written to a configuration file and then committed to a repository | High | AC-9 |

## 7. Open questions

- What default token lifetime suits the team's working rhythm? Ninety days is proposed as a starting
  point.
- Should token creation require Admin approval, or is role-constrained self-service sufficient?
  Recommendation: role-constrained self-service, since the scopes a User may request are already limited.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
