# TSK-020-02 — Write the authorization regression suite

| | |
|---|---|
| ID | TSK-020-02 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Partial — focused authorization regressions exist; exhaustive route matrix remains |
| Layer | BE |
| Requirement | [REQ-020](../../requirement/01-REQ-020-automated-testing-ci.md) |
| Design | [DES-020 sections 2, 8](../../design/01-DES-020-automated-testing-ci.md) |
| Covers | AC-8 |
| Depends on | TSK-020-01 |
| Estimate | 1d |
| Branch | `feat/REQ-020-authorization-suite` |

## 1. Goal

Every endpoint in `routes/mod.rs` is asserted against `admin`, `contribute` and `user`. This is the
security-relevant deliverable of REQ-020 and the evidence base for REQ-004 and REQ-006.

The current source has focused authorization tests for high-risk routes, token authentication, resource
ownership and analytics. This task remains open because it still lacks an automatically checked matrix
covering every mounted route and all three roles.

## 2. Files in scope

| File | Action |
|---|---|
| `crates/conductor-server/tests/authorization.rs` | create |
| `crates/conductor-server/tests/support/mod.rs` | modify, add a POST helper and a token-authenticated helper |

## 3. Implementation steps

1. Enumerate every route from
   [`routes/mod.rs`](../../../crates/conductor-server/src/http/routes/mod.rs). Endpoints authenticated by a
   browser session and endpoints authenticated by an `evc_` token are two separate tables; do not mix them.
2. For each session endpoint write one test asserting the status for each of the three roles. Assert the
   exact expected status, never "not 500".
3. Where current behaviour is wrong, write the **correct** expectation. The remaining dashboard case may
   be ignored with a named REQ-004 reason until the guard lands; do not weaken it to match the bug.
4. Current reconciliation findings from [requirements.md section 3](../../requirements.md):
   - `GET /api/dashboard` with role `user` should be `403`; today it is `200`
     ([dashboard.rs:8-13](../../../crates/conductor-server/src/http/routes/dashboard.rs)).
   - `GET /api/resources` now applies actor-aware visibility and no longer returns the whole catalog.
   - `POST /api/secrets` now rejects an omitted or empty scope set instead of granting defaults.
5. Preserve and extend token-path cases for malformed, expired, revoked, wrong-scope and disabled-owner
   tokens. These paths now have focused coverage, but must also appear in the exhaustive route matrix.
6. Run the full suite and record its current output. If the dashboard case is added before its guard,
   additionally run ignored tests to retain explicit evidence of that one remaining failure.

## 4. Required tests

### Layer BE (Rust)

| Type | Tool | Must cover |
|---|---|---|
| HTTP route | `axum::Router` with `tower::ServiceExt::oneshot` | Status code for every session endpoint against all three primary roles |
| Authorization regression | as above | Explicit `200` or `403` per role; no "not 500" assertions |
| Token path | as above | Malformed, expired, revoked, and disabled-owner tokens |

Mandatory: every endpoint is tested against all three roles, including where all three are expected to
succeed. An endpoint added later without a row in this table is a review failure.

## 5. Commands and reports

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace -- --ignored    # only while a correct dashboard case is intentionally ignored
```

## 6. Definition of done

- [ ] Every route in `routes/mod.rs` appears in the suite
- [ ] Each session endpoint asserts an explicit status for all three roles
- [ ] Any expected failure carries `#[ignore]` with a reason naming the requirement that fixes it
- [x] Existing focused authorization tests are green
- [ ] The remaining dashboard case and exhaustive route inventory are committed
- [ ] Every command in section 5 runs clean
- [ ] No new clippy warning

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-8 | Regular members cannot inspect Agent/Skill/Plugin archives | `resource_archive_import.rs`, `plugin_archive_import.rs` | Pass |
| AC-8 | Contributors/other members cannot manage another member's tokens | `member_secrets.rs` | Pass |
| AC-8 | Registration wrong-scope/revoked/owner-scoped heartbeat | `client_registration.rs` | Pass |
| AC-8 | Storage/data-policy changes require Admin | `storage_settings.rs` | Pass |
| AC-8 | Saved-view visibility/ownership | `analytics_views.rs` | Pass |
| AC-8 | Dashboard role matrix | not committed | Missing |

### Command output

```
cargo test --workspace  PASS (94 tests; verified 2026-08-14)
```

### Notes

Focused security cases live beside each feature rather than in one `authorization.rs`. That is useful
coverage but does not satisfy the exhaustive route-inventory acceptance criterion.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-09 | Todo | Created |
| 2026-08-14 | Partial | Focused route/token authorization coverage passes; dashboard guard and exhaustive three-role route matrix remain |
