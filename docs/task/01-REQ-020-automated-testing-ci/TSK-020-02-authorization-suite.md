# TSK-020-02 — Write the authorization regression suite

| | |
|---|---|
| ID | TSK-020-02 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Todo |
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

Two cases are **expected to fail on current code**. That is the point of the task, not a defect in it.

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
3. Where current behaviour is wrong, still write the **correct** expectation and mark the test
   `#[ignore = "expected failure until REQ-004; remove the ignore in that task"]`. Do not weaken the
   assertion to match the bug.
4. Known expected failures, from [requirements.md section 3](../../requirements.md):
   - `GET /api/dashboard` with role `user` should be `403`; today it is `200`
     ([dashboard.rs:8-13](../../../crates/conductor-server/src/http/routes/dashboard.rs)).
   - `GET /api/resources` with role `user` should be filtered; today it returns the whole catalog
     ([resources.rs:53](../../../crates/conductor-server/src/http/routes/resources.rs)).
   - `POST /api/secrets` with an omitted `scopes` array should be `400`; today it grants all three scopes
     ([secrets.rs:31-38](../../../crates/conductor-server/src/http/routes/secrets.rs)).
5. Add token-path cases: a malformed token, an expired token, a revoked token, and a token whose owner is
   disabled. The last is expected to fail until REQ-005 and carries the same ignore marker.
6. **Run the suite with `--run-ignored all` and capture the failures before any fix.** That output is the
   before-evidence recorded in section 7 and referenced by REQ-004's and REQ-006's tasks.

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
cargo nextest run
cargo nextest run --run-ignored all    # captures the expected failures
```

## 6. Definition of done

- [ ] Every route in `routes/mod.rs` appears in the suite
- [ ] Each session endpoint asserts an explicit status for all three roles
- [ ] Expected failures carry `#[ignore]` with a reason naming the requirement that fixes them
- [ ] `cargo nextest run` is green; `--run-ignored all` shows exactly the documented failures and no others
- [ ] The before-fix failure output is pasted in section 7
- [ ] Every command in section 5 runs clean
- [ ] No new clippy warning

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-8 | `dashboard_role_matrix` | `tests/authorization.rs` | |
| AC-8 | `resources_role_matrix` | `tests/authorization.rs` | |
| AC-8 | `secrets_requires_explicit_scopes` | `tests/authorization.rs` | |
| AC-8 | `members_role_matrix` | `tests/authorization.rs` | |
| AC-8 | `settings_role_matrix` | `tests/authorization.rs` | |
| AC-8 | `token_rejected_when_owner_disabled` | `tests/authorization.rs` | |

### Expected failures before any fix

<!-- Paste the `--run-ignored all` output. This is the evidence REQ-004 and REQ-006 are built on.
     It must show the failures, not a summary of them. -->

```
```

### Command output

```
<paste the unmodified output>
```

### Notes

<!-- Any endpoint whose correct expectation is genuinely unclear belongs here as an open question,
     not as a guessed assertion. -->

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-09 | Todo | Created |
