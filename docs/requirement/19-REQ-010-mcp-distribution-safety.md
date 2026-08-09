# REQ-010 — MCP distribution safety

| | |
|---|---|
| ID | REQ-010 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P1 |
| Build order | Step 19 of 23 |
| Spec section | [requirements.md section 6](../requirements.md), addition |
| Source | Risk analysis 2026-08-09 |
| Depends on | REQ-007, REQ-012 |
| Blocks | none |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

The specification lists MCP configuration as one resource type among several. It behaves differently from
the others in one decisive respect: an MCP server definition contains an executable command with
arguments and environment variables.

Distributing MCP configuration to member machines is therefore remote code execution by configuration. A
bad prompt degrades answers. A bad MCP definition starts an unknown process on every machine in the
project. These two cases share a table but must not share a trust level.

The exposure is real today rather than theoretical: any authenticated user can currently mint a
fully scoped token, and there is no audit log.

## 2. Requirement

MCP configuration shall follow a stricter publication and activation path than other resource types.
Publication shall be limited to Admin, and a member's EvoFlux installation shall not activate a newly
received MCP server without explicit user confirmation.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| `ResourceKind::Mcp` exists in the catalog ([resource.rs](../../crates/conductor-domain/src/resource.rs)) | Every item in this requirement |
| EvoFlux has an `MCPManager` owning MCP server lifecycles | Role differentiation between MCP and other resource types |
| EvoFlux applies the same `allow`, `deny`, `ask` permission rules to MCP tools as to native tools | A confirmation step when a new MCP definition arrives |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Only Admin may publish a resource of type `mcp`; Contributor receives `403` |
| AC-2 | The console displays an explicit warning when publishing MCP configuration, stating that it will start a process on member machines |
| AC-3 | EvoFlux does not activate a newly received MCP server automatically; it waits for user confirmation |
| AC-4 | The confirmation screen displays the exact command, arguments and environment variables that will run |
| AC-5 | Declining is remembered; the user is not prompted again on every synchronization |
| AC-6 | A change to an already-accepted MCP definition triggers a new prompt, detected by checksum |
| AC-7 | When an Admin archives an MCP resource, EvoFlux stops that server on the next synchronization |
| AC-8 | Publication, modification and retirement of MCP resources are recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |

## 5. Out of scope

- Signing or provenance verification of MCP packages. Reconsider at P2.
- Sandboxed execution of MCP servers, which belongs to EvoFlux rather than to Conductor.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | A compromised Admin account can run code on every machine in the project | High | AC-3 and AC-4 keep the user as the final gate |
| 2 | Users approve prompts reflexively without reading them | Medium | AC-4 shows the real command; AC-6 re-prompts only on genuine change |
| 3 | The confirmation step is seen as friction and removal is requested | Medium | AC-5 limits prompting to actual changes |

## 7. Open questions

- Should a "trust this Conductor completely" mode exist that bypasses AC-3? Recommendation: no. This is
  the final boundary between centralized configuration management and remote control of a member's
  machine, and it should not be configurable away.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
