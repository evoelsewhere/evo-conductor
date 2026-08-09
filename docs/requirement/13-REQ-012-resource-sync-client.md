# REQ-012 — Resource synchronization client

| | |
|---|---|
| ID | REQ-012 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Build order | Step 13 of 23 |
| Spec section | [requirements.md sections 6 and 12](../requirements.md) |
| Source | Baseline specification sections 6, 12 and 14 |
| Depends on | REQ-007, REQ-008, REQ-011 |
| Blocks | V1 acceptance criteria 6 and 12 |
| Repositories | `evoflux` primarily, `evo-conductor` for the change endpoint |
| Design | Not created; requires acceptance |

## 1. Context

This requirement delivers the visible benefit of the whole system: a member opens EvoFlux and the
project's approved agents, prompts and documents are simply there.

The consumption side already exists in EvoFlux, so the work is retrieval, placement and conflict
handling. The single largest risk is damaging the user's own files, which would destroy trust in the
integration immediately and permanently.

## 2. Requirement

EvoFlux shall retrieve published resources and documents permitted for the token owner, shall place them
where the agent runtime already reads them, and shall never modify repository files or overwrite content
the user has edited.

## 3. Implementation status

| Implemented, EvoFlux side | Missing |
|---|---|
| Agents load from `AGENTS_DIR`, defaulting to `{CONFIG_DIR}/agents` ([config.py:245](../../../evoflux/app/core/config.py)) | The entire synchronization client |
| An agent is a Markdown file with YAML frontmatter and a prompt body ([loader.py](../../../evoflux/app/agent/loader.py)) | Conflict detection |
| Instruction files load per workspace root into every model call ([workspace_instructions.py:30-70](../../../evoflux/app/agent/hooks/workspace_instructions.py)) | A synchronization status screen |
| Extra roots merge as `[workspace, *extra]` ([workspace_instructions.py:44-48](../../../evoflux/app/agent/hooks/workspace_instructions.py)) | `GET /api/v1/resources/changes?cursor=` on the server |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Synchronization runs on connection, on a configurable interval, and on manual request |
| AC-2 | Resources of type `agent` are written to `AGENTS_DIR`; documents and context resources are written to a Conductor-owned directory |
| AC-3 | The Conductor-owned directory is registered as an extra workspace root, so its `AGENTS.md` reaches the system prompt |
| AC-4 | EvoFlux never writes `AGENTS.override.md`, verified by an explicit test asserting the file is not created |
| AC-5 | EvoFlux never writes inside the user's repository working tree |
| AC-6 | A file modified locally is not overwritten; the conflict is reported naming the file and the nature of the difference |
| AC-7 | Only published versions permitted by the access policy are written, satisfying V1 acceptance criterion 6 |
| AC-8 | Checksums are used to skip unchanged content instead of rewriting files on every cycle |
| AC-9 | `GET /api/v1/resources/changes?cursor=` returns only changes since the cursor, and the client persists the cursor |
| AC-10 | When Conductor is unreachable, EvoFlux continues to operate using the previously synchronized content |
| AC-11 | A revoked token stops synchronization on the next cycle and informs the user, satisfying V1 acceptance criterion 12 |
| AC-12 | A synchronization status view shows last run time, resources received, changes applied and any errors |
| AC-13 | Resources removed from the member's permitted set are removed locally, and the removal is reported |
| AC-14 | When a new version of an already-synchronized resource is published, the installation converges on that version within one synchronization cycle and reports the version it holds |

## 5. Out of scope

- MCP activation, which requires confirmation; see [REQ-010](19-REQ-010-mcp-distribution-safety.md).
- Uploading local inventory, covered by [REQ-013](14-REQ-013-inventory-synchronization.md).
- Per-member version pinning. Reconsider at P2.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Overwriting a user's own work | High | AC-4, AC-5, AC-6 |
| 2 | `AGENTS.override.md` is used because the name appears correct, silently discarding project instructions | High | AC-4 makes this a test, not a convention |
| 3 | EvoFlux becomes dependent on network availability | Medium | AC-10 |
| 4 | Agents can read the Conductor-owned directory because it is also a sandbox root ([sandbox.py:120-123](../../../evoflux/app/agent/sandbox.py)) | Low | Document the behaviour so it is not a surprise |
| 5 | Deletion logic removes files the user created in the managed directory | Medium | AC-13 limits removal to previously synchronized paths recorded in local state |

## 7. Open questions

- Where should the Conductor-owned directory live: `{CONFIG_DIR}/conductor/<project-slug>/` shared across
  the machine, or one directory per workspace? The former is recommended, since it keeps one project's
  content in one place and avoids duplication across workspaces.
- What is the default synchronization interval?

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
