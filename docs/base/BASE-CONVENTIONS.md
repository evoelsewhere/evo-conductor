# BASE — Document conventions

Foundation document. Every `REQ`, `DES` and `TSK` references this file and does not restate its contents.
Change a convention here, not in individual documents.

## 1. Identifiers

| Type | Format | Example | Note |
|---|---|---|---|
| Requirement | `REQ-NNN` | `REQ-007` | Sequential, never reused even if rejected |
| Design | `DES-NNN` | `DES-007` | Same number as the requirement it designs |
| Task | `TSK-NNN-MM` | `TSK-007-02` | `NNN` is the requirement, `MM` is the task sequence |

File names carry the **build step** as a prefix so the directory listing reads as the implementation plan:

```
requirement/<step>-<REQ-ID>-<slug>.md            01-REQ-020-automated-testing-ci.md
design/<step>-<DES-ID>-<slug>.md                 01-DES-020-automated-testing-ci.md
task/<step>-<REQ-ID>-<slug>/<TSK-ID>-<slug>.md   01-REQ-020-automated-testing-ci/
                                                   TSK-020-01-backend-test-harness.md
```

The step is a **position**, the ID is an **identity**. They are deliberately separate: the step changes
when the plan is resequenced, the ID never does. Cross-references always use the ID
(`REQ-007`), never the step, so reordering the plan never invalidates a reference.

A requirement has one design, so `design/` stays flat. A requirement has several tasks, so each gets its
own directory under `task/` named after the requirement it implements. The step prefix lives on the
directory, not on the files inside it.

When the plan is resequenced, rename the files and directories and update the `Build order` row in each
document. Do not renumber IDs.

## 2. Lifecycle

```
REQ: Draft --> Accepted --> (design exists)
       |--> Rejected      record the reason, keep the file
       |--> Deferred      record the condition that would reopen it

DES: Draft --> Approved --> (tasks exist)
       |--> Superseded    link to the replacement design

TSK: Todo --> In Progress --> In Review --> Done
                    |--> Blocked   state what is blocking
```

Gates:

- Do not create a `DES` while its `REQ` is not `Accepted`.
- Do not create a `TSK` while its `DES` is not `Approved`.
- Do not close a `TSK` while its results section is empty.

## 3. Mandatory metadata

Every document opens with a metadata table containing at least ID, created date, updated date, status and
references. Dates use `YYYY-MM-DD`.

## 4. Referencing rules

- New documents always link back to their parent: `REQ` to a section of
  [requirements.md](../requirements.md), `DES` to its `REQ`, `TSK` to its `DES`.
- Source references must name file and line and must be a working relative link, for example
  [`resource.rs:16`](../../crates/conductor-storage/src/repos/resource.rs#L16).
- Do not describe the current state of the code from memory. If the file has not been opened, the
  statement does not belong in the document.
- Content that already exists in a foundation document is linked, never copied.

## 5. Acceptance criteria

- Numbered `AC-1`, `AC-2` and so on within each `REQ`.
- Each criterion must be verifiable: state the input, the expected behaviour and how it is observed.
- Not acceptable: "authorization must be strict". Acceptable: "a User calling `GET /api/dashboard`
  receives `403`".
- Every `DES` carries a traceability table from acceptance criteria to components. Every `TSK` carries a
  traceability table from acceptance criteria to test cases.
- Every acceptance criterion must be covered by at least one automated test.

## 6. Priorities

| Level | Meaning |
|---|---|
| P0 | Required for the V1 acceptance run in [requirements.md section 16](../requirements.md), or a foundation defect that blocks other work |
| P1 | Required for a release usable by a real team |
| P2 | Later extension |
| Deferred | Explicitly out of scope for now; the requirement records the condition that would reopen it |

## 7. Technology stack

| Layer | Technology |
|---|---|
| Backend | Rust, Axum 0.8, sqlx 0.8 with the `Any` driver, tokio |
| Database | PostgreSQL in production, SQLite for development |
| Frontend | React 19, Vite 8, TanStack Router and Query, Tailwind 4, Zustand, Base UI |
| Client | EvoFlux: Python 3.12 FastAPI sidecar plus React 19, inside a Tauri v2 shell |

## 8. Test tooling

Commands and report formats are specified in [TEMPLATE-TASK.md](TEMPLATE-TASK.md). None of the tooling
below is currently installed in this repository;
[REQ-020](../requirement/01-REQ-020-automated-testing-ci.md) covers setting it up.

| Layer | Test type | Tool |
|---|---|---|
| Backend | Unit, pure domain | `cargo test` |
| Backend | Repository | `cargo test` with sqlx against `sqlite::memory:` |
| Backend | HTTP route | `axum` with `tower::ServiceExt::oneshot` |
| Backend | Cross-database | migrations executed against PostgreSQL in CI |
| Backend | Runner and reporting | `cargo nextest` producing JUnit XML, `cargo llvm-cov` for coverage |
| Backend | Static | `cargo fmt --check`, `cargo clippy -- -D warnings` |
| Frontend | Unit and component | `vitest` with `@testing-library/react` |
| Frontend | End to end | `playwright`, HTML report with screenshots |
| Frontend | Static | `tsc -b --noEmit`, `eslint` |
| EvoFlux | Backend | `pytest`, `ruff`, `ty` |
| EvoFlux | Frontend | `vitest`, `playwright` |

## 9. Commit and branch conventions

Conventional commits, matching the existing history: `feat(scope):`, `fix(scope):`, `refactor(scope):`,
`chore:`. Common scopes: `api`, `web`, `domain`, `storage`, `auth`, `docs`.

Branch: `feat/REQ-NNN-<slug>`, so work traces back to a requirement.

## 10. Privacy boundary

The following two principles apply to every design and task. They may only be relaxed by a requirement
that states the exception explicitly and has been accepted.

- **Measure, do not surveil.** Conversation content, file content and source code are never collected.
  See [requirements.md section 10](../requirements.md).
- **Members can see their own record.** Anything an administrator can see about a member, that member can
  see about themselves. This is enforced as an automated test, not as a written promise. See
  [REQ-015](../requirement/11-REQ-015-privacy-controls.md).

## 11. Terminology

| Term | Meaning |
|---|---|
| Project workspace | The single project served by one Conductor deployment in V1 |
| Installation | One EvoFlux desktop install, identified independently of the member using it |
| Resource | A distributable unit: agent, standalone skill, portable Agent Plugin package, workflow, command, prompt template |
| Managed resource identity | The immutable tuple `(project_id, resource_id)` used by Conductor and EvoFlux; kind, slug, display name and local path are not identity |
| Draft workspace | A mutable, server-owned source tree for one resource; Save updates this workspace but never changes a released artifact |
| Resource version | An immutable snapshot built from a validated draft workspace and identified by a server-issued version ID, server-allocated or strictly validated SemVer, and SHA-256 digest |
| Release channel | The audience of an immutable resource version: `beta` for explicitly selected members or `published` for all members allowed by the resource access policy |
| Portable Agent Plugin | An Agent Plugins 1.0 package (`plugin.json`, optional `skills/*` and `mcp.json`) distributed as a `.evoplugin`/ZIP artifact; it is distinct from EvoFlux legacy Python hooks |
| Document | A published project document, distinct from a resource and from a session artifact |
| Connection token | An `evc_` prefixed scoped token used by EvoFlux, not a browser session |
| Collection level | L0, L1 or L2 as defined in [REQ-015](../requirement/11-REQ-015-privacy-controls.md) |
