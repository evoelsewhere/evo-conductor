# REQ-009 — Project document management

| | |
|---|---|
| ID | REQ-009 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P1 |
| Build order | Step 18 of 23 |
| Spec section | [requirements.md section 12](../requirements.md) |
| Source | Baseline specification section 12 |
| Depends on | REQ-001, REQ-006, REQ-007, REQ-008 |
| Blocks | REQ-012 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

Projects need to distribute coding standards, architecture guidelines, security policies, migration
rulebooks, onboarding instructions and MCP usage policies. These are the instructions that make an agent
behave according to project conventions rather than generic defaults.

EvoFlux already consumes this class of file, so the work is distribution and governance, not consumption.

## 2. Requirement

Conductor shall manage published project documents with versioning, tags, role-based access, publication
status, checksums and synchronization state, and shall distribute only explicitly published documents.
Document content shall remain distinct from user-generated session artifacts.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Nothing on the Conductor side | `documents` table, endpoints, console screens, `read_documents` scope | |
| On the EvoFlux side, the consumer already exists, see below | Attachment storage | |

### How documents reach the agent

`WorkspaceInstructionsHook` appends the `AGENTS.md` of every workspace root to the system prompt of every
model call, loads nested directories on demand, and blocks a mutating tool call once so the model is
forced to read newly applicable rules before editing
([workspace_instructions.py:30-70](../../../evoflux/app/agent/hooks/workspace_instructions.py)).

### The landing location, and the trap

There are three candidate destinations and only one is correct.

- Writing `AGENTS.override.md` is incorrect. The loader returns either the override file or the standard
  file, never both ([workspace_instructions.py:194-199](../../../evoflux/app/agent/hooks/workspace_instructions.py)),
  so this silently discards the project's own instructions.
- Overwriting `AGENTS.md` is incorrect. It normally lives inside the repository and is tracked by git.
- Writing into a Conductor-owned directory outside the repository and registering it as an extra
  workspace root is correct. The hook merges roots as `[workspace, *extra]`
  ([workspace_instructions.py:44-48](../../../evoflux/app/agent/hooks/workspace_instructions.py)), so
  Conductor content is injected alongside project content.

Note that `extra_workspace_paths` is also a sandbox root
([sandbox.py:120-123](../../../evoflux/app/agent/sandbox.py)), so agents can read that directory.

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A `documents` table stores title, slug, body, format, version, status, checksum, owner, publisher, tags, access policy and timestamps |
| AC-2 | Documents support Markdown bodies and file attachments |
| AC-3 | Documents follow the same lifecycle as resources: `draft`, `published`, `deprecated`, `archived` |
| AC-4 | Every publish creates an immutable version; history is viewable and rollback is supported |
| AC-5 | Access is governed by the same policy model as [REQ-008](10-REQ-008-resource-access-policy.md) |
| AC-6 | Document endpoints require the `read_documents` scope for token-authenticated callers |
| AC-7 | Only `published` documents are distributed |
| AC-8 | A checksum is exposed so clients can detect change without downloading the body |
| AC-9 | Documents intended for agent consumption are size-validated against the EvoFlux limits stated in [REQ-007](09-REQ-007-resource-lifecycle.md) AC-8 |
| AC-10 | The console can preview a document as rendered Markdown before publication |
| AC-11 | Publication and retirement are recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |

## 5. Out of scope

- A collaborative wiki with in-console editing, search and cross-linking. EvoFlux already provides a
  knowledge wiki for session-derived material, and duplicating it here would build a second product.
- Session artifacts and agent outputs, which the specification explicitly separates from documents.
- Binary rendering such as PDF preview.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Writing `AGENTS.override.md` because the name appears correct, discarding project instructions | High | Stated in section 3; [REQ-012](13-REQ-012-resource-sync-client.md) requires a test asserting the file is never created |
| 2 | Scope grows into a full wiki | Medium | Section 5 is binding |
| 3 | Attachment storage introduces a new class of operational concern such as size limits and backups | Medium | Consider deferring attachments to a second phase and shipping Markdown first |

## 7. Open questions

- Are file attachments required for V1, or is Markdown alone sufficient at first? Deferring attachments
  removes a storage subsystem from the critical path.
- Where are attachments stored: in the database, on the filesystem, or in object storage?

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
