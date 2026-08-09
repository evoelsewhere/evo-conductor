---
name: conductor-dev-workflow
description: Use for EVERY development task on Evo Conductor (crates/, apps/web/) — new features, refactors, and edit requests alike. Enforces the gated pipeline Requirement → Design → Task → Code → Test → Report, and routes to the right specialist skill. Trigger on "implement REQ-xxx", "add an endpoint to Conductor", "build the members screen", "sửa lại X trong conductor", "làm tính năng X cho conductor".
---

# Evo Conductor development workflow

Conductor is the control plane for a project whose members use EvoFlux. It is a Rust workspace plus a
React console. It is **not** the agent runtime — agents and tools run on the member's machine.

## 0. The gate — check this before writing any code

This repo drives work from documents in `docs/`, not from ad-hoc requests:

```
REQ (Accepted) --> DES (Approved) --> TSK --> code + test --> results
```

Before implementing anything, confirm:

1. A requirement exists in `docs/requirement/REQ-NNN-*.md` and its status is `Accepted`.
2. A design exists in `docs/design/DES-NNN-*.md` and its status is `Approved`.
3. A task exists in `docs/task/TSK-NNN-MM-*.md`.

If any of these is missing, **say so and stop**. Write the missing document instead of guessing at the
implementation. The conventions are in `docs/base/BASE-CONVENTIONS.md`; templates are next to it.

The one exception is a trivial mechanical fix — a typo, a formatting change — which needs no requirement.

## 1. Route to the right skill

| Files under | Layer | Skill |
|---|---|---|
| `crates/conductor-domain/`, `conductor-storage/`, `conductor-auth/`, `conductor-server/` | Backend | `conductor-backend` |
| `apps/web/src/` | Frontend | `conductor-frontend` |
| Anything that EvoFlux consumes, produces, or authenticates against | Cross-repo contract | `conductor-evoflux-integration` |

A task that touches both backend and frontend is **two tasks**, not one. See
`docs/base/TEMPLATE-TASK.md`.

Whenever the change is about what EvoFlux downloads, uploads, or reads on disk, load
`conductor-evoflux-integration` as well. Getting the landing paths wrong on the EvoFlux side silently
destroys user files; the exact rules are in that skill.

## 2. Know the current state before you build on it

The codebase has known defects that documents already record. Do not replicate or build on top of them:

| Defect | Where | Requirement that fixes it |
|---|---|---|
| Migrations discard `ALTER` errors, no `schema_version` | `crates/conductor-storage/src/migrate.rs:166` | REQ-001 |
| OIDC client secret stored plaintext in a column named `client_secret_enc` | `crates/conductor-server/src/http/routes/setup.rs:71-85` | REQ-002 |
| `GET /api/dashboard` has no role check; `can_view_telemetry()` never called | `crates/conductor-server/src/http/routes/dashboard.rs:8-13` | REQ-004 |
| `POST /api/secrets` has no role check and grants all scopes when `scopes` is omitted | `crates/conductor-server/src/http/routes/secrets.rs:31-38` | REQ-004, REQ-006 |
| Token validation never checks whether the owner is disabled | `crates/conductor-server/src/http/routes/resources.rs:31-51` | REQ-005 |
| `ResourceRepo` has only `list()`; the catalog cannot be written | `crates/conductor-storage/src/repos/resource.rs` | REQ-007 |
| `subscribe` returns the whole catalog to any valid token | `crates/conductor-server/src/http/routes/resources.rs:53` | REQ-008 |
| `member_inventory` is never written, so `members_online` is always zero | `crates/conductor-storage/src/repos/dashboard.rs` | REQ-013 |
| `telemetry_events` has no index | `crates/conductor-storage/src/migrate.rs:133-143` | REQ-014 |
| No audit table exists anywhere | `crates/conductor-storage/src/migrate.rs` | REQ-018 |

If a task requires a new table, REQ-001 must land first. Adding tables on the current migration
mechanism produces a schema nobody can verify.

## 3. Code

Follow the skill for the layer you are in. Two rules apply everywhere:

- **Server-side enforcement.** Authorization is checked in the Rust API. Hiding a route in the console is
  a usability measure, never a security measure. This is stated in `docs/requirements.md` section 7.
- **No content collection.** Nothing in this system may carry conversation content, file content, source
  code or credentials. See `docs/base/BASE-CONVENTIONS.md` section 10.

## 4. Test

There is currently **no test tooling in this repository**: the Rust workspace has zero tests, and
`apps/web/package.json` declares neither a test script nor a lint script. REQ-020 builds it.

Until REQ-020 lands, a task cannot honestly claim its tests pass. Say that plainly rather than reporting
success. If you are implementing REQ-020, that is the point.

Once it exists:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run --profile ci
```

```bash
cd apps/web
bun run typecheck
bun run lint
bun run test:unit
```

Every new endpoint is tested against all three primary roles (`admin`, `contribute`, `user`), including
cases where all three are expected to succeed. This is the area where real defects already exist.

## 5. Build

```bash
cargo build --release
cd apps/web && bun run build
```

`make dev` runs the API on `:4700` and Vite on `:5174` together. `make reset-db` recreates a development
database. `make help` lists the rest.

## 6. Report

Fill in the results section of the task document with **real command output**, not a description of it. A
task whose results section is empty cannot be closed. Frontend tasks with a visible change require
screenshots.

## 7. Commit

Conventional commits, matching the existing history: `feat(api):`, `feat(web):`, `fix(...)`,
`refactor(...)`, `chore:`. Common scopes: `api`, `web`, `domain`, `storage`, `auth`, `docs`.

Branch: `feat/REQ-NNN-<slug>`.

Push and PR creation require explicit user confirmation. Note that as of the last check the repository
`evoelsewhere/evo-conductor` granted only `READ` to the configured account, so a push may fail with 403 —
report that plainly rather than working around it.
