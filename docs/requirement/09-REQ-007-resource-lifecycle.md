# REQ-007 — Resource model, versioning and lifecycle

| | |
|---|---|
| ID | REQ-007 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Build order | Step 9 of 23 |
| Spec section | [requirements.md section 6](../requirements.md) |
| Source | Baseline specification section 6 |
| Depends on | REQ-001, REQ-004, REQ-018 |
| Blocks | REQ-008, REQ-010, REQ-012 |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

This is the core value of Conductor: standardizing agents, prompts, skills and configuration across a
project. The catalog is currently read-only, so the `resources` table can never contain anything, while
the console already tells users that publishing is available.

Resource content is small structured text. In EvoFlux an agent is a Markdown file whose YAML frontmatter
carries configuration and whose body is the system prompt
([loader.py](../../../evoflux/app/agent/loader.py)), which the existing free-form JSON `payload` column
can already carry without a schema change.

## 2. Requirement

Conductor shall store resources with a full metadata set and an explicit lifecycle, shall create an
immutable version on every publish, shall allow rollback, and shall validate payloads before publication.
Only published versions shall be distributed.

Resource types shall be agent, skill, MCP configuration, workflow, command, document or project policy,
and reusable prompt template.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Domain types `ManagedResource`, `ResourceKind`, `ResourceVisibility` ([resource.rs](../../crates/conductor-domain/src/resource.rs)) | Every write path; `ResourceRepo` has only `list()` ([resource.rs](../../crates/conductor-storage/src/repos/resource.rs)) | The console empty state promises publishing that the backend does not provide ([resources-page.tsx:38](../../apps/web/src/features/resources/pages/resources-page.tsx)) |
| `payload` as free-form JSON, sufficient to carry Markdown unchanged | `resource_versions` table | `UNIQUE(kind, slug)` with a single `version` column means an update overwrites, losing history |
| `visibility` column, `version` column | Lifecycle status, checksum, change notes, publisher | `ResourceKind::Command` is absent from `ResourceCounts` |
| Read-only console listing | Payload validation and size limits | |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | `POST /api/resources` creates a draft; `PATCH /api/resources/{id}` updates it; `POST /api/resources/{id}/publish` publishes a version; `POST /api/resources/{id}/deprecate` deprecates it |
| AC-2 | Each resource carries stable ID and slug, name, description, type, semantic version, payload reference, content checksum, owner, publisher, visibility, lifecycle status, timestamps, tags and change notes |
| AC-3 | Lifecycle status is one of `draft`, `published`, `deprecated`, `archived`, and transitions are validated |
| AC-4 | Every publish writes an immutable row to `resource_versions`; prior versions are never modified or deleted |
| AC-5 | `GET /api/resources/{id}/versions` returns the history, and rollback republishes a prior version as a new version rather than mutating history |
| AC-6 | Only Admin and Contributor may write; a User receives `403` |
| AC-7 | A payload exceeding the size limit is rejected at publish time with a message stating both the limit and the actual size |
| AC-8 | Size limits match the EvoFlux consumer: 128 KB for instruction content ([workspace_instructions.py:25](../../../evoflux/app/agent/hooks/workspace_instructions.py)) and 64 KB for per-repository `AGENTS.md` ([multi_repo_context.py:16](../../../evoflux/app/agent/hooks/multi_repo_context.py)) |
| AC-9 | For `kind = agent`, the payload is validated as parseable YAML frontmatter plus a body; a malformed payload is rejected |
| AC-10 | A content checksum is computed and stored, and is exposed to clients for change detection |
| AC-11 | `ResourceCounts` counts every resource type, including `command` and any type added later |
| AC-12 | Only `published` versions are returned by the distribution endpoints |
| AC-13 | Publish, update, deprecate, archive and rollback are all recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |

## 5. Out of scope

- MCP publication constraints, which are stricter; see [REQ-010](19-REQ-010-mcp-distribution-safety.md).
- Documents, which have their own model; see [REQ-009](18-REQ-009-document-management.md).
- Access targeting; see [REQ-008](10-REQ-008-resource-access-policy.md).
- Staged or canary rollout. Reconsider at P2.
- Authoring resources inside the console. Upload and preview are sufficient for V1.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | A bad prompt edit changes agent behaviour across the whole team with no traceability | High | AC-4, AC-5 and AC-13 |
| 2 | Published content is silently truncated on member machines | Medium | AC-7 and AC-8 reject at the source |
| 3 | A new resource type is added while lower layers are not updated | Medium | AC-11, which addresses a defect already present with `command` |
| 4 | Version history grows without bound | Low | Resource payloads are small text; revisit only if measurements say otherwise |

## 7. Open questions

- Are versions assigned automatically or entered by the publisher as semantic versions? The specification
  says semantic version, which implies publisher-supplied; confirm whether the server should validate the
  ordering.
- Does deprecation stop distribution immediately, or does it warn while continuing to serve? Recommendation:
  `deprecated` continues to serve with a warning, `archived` stops serving.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
