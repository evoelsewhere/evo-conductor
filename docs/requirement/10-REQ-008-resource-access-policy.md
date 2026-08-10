# REQ-008 — Resource access policy

| | |
|---|---|
| ID | REQ-008 |
| Created | 2026-08-09 |
| Updated | 2026-08-11 |
| Status | Accepted (2026-08-11; owner requested design and task planning) |
| Priority | P0 |
| Build order | Step 10 of 23 |
| Spec section | [requirements.md section 7](../requirements.md) |
| Source | Baseline specification section 7 |
| Depends on | REQ-004, REQ-007 |
| Blocks | REQ-012, V1 acceptance criterion 6 |
| Repositories | `evo-conductor` |
| Design | [DES-007](../design/09-DES-007-governed-resource-delivery.md) sections 5.2 and 6 — Approved 2026-08-11 |

## 1. Context

Developers, business analysts and testers need different resource sets. The specification requires
targeting by primary role, sub-role, tag, explicit member ID, or all project members, with exclusions.

The current catalog now stores inclusion rules in `resource_access_rules` and `ResourceRepo` evaluates
all-members, role, sub-role, tag and explicit-member subjects for token-authenticated subscription
([resource.rs](../../crates/conductor-storage/src/repos/resource.rs)). The remaining baseline gaps are
exclusions, strict private-resource semantics and effective-audience preview. Beta adds a second question:
which immutable released version an already-authorized member receives.

The specification is explicit that this must be enforced server-side, because frontend route hiding is
not security.

## 2. Requirement

Conductor shall attach an access policy to each resource and shall return to each EvoFlux installation
only the released version permitted for the token owner. Enforcement shall occur in the Rust API.

Beta audience is a release-channel selector, not a second access policy. An explicit beta-member set may
only narrow the members who already pass the resource access policy; it may never grant access. For an
eligible selected member, the active Beta is the effective version. For every other eligible member, the
active Published version is effective. Exclusion, private ownership, disabled status and project
separation take precedence over beta selection.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Token authentication and `subscribe` call `list_visible_to` for the connection-secret owner ([resources.rs](../../crates/conductor-server/src/http/routes/resources.rs)) | Explicit exclusion rules and their precedence | A private resource with an explicit allow rule can currently be returned to a non-owner; AC-4 requires owner-only semantics |
| `resource_access_rules` stores all, primary-role, sub-role, tag and member inclusions; the repository joins user roles/tags server-side ([resource.rs](../../crates/conductor-storage/src/repos/resource.rs)) | Effective audience/member preview and target-specific explanations | The existing policy model has inclusions only, although the baseline requires member exclusions |
| Shared/no-policy and owner/default handling exist in the visibility query | Beta version/audience tables and effective-version resolution | Current filtering is resource-level and has no Beta-versus-Published channel selection |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A resource may be targeted by primary role, sub-role, tag, explicit member ID, or all project members |
| AC-2 | A policy may exclude specific members, and exclusion takes precedence over every inclusion rule |
| AC-3 | A shared resource with no policy is visible to all active project members; a Beta remains limited to its explicit eligible member set |
| AC-4 | A `private` resource is returned only to its owner regardless of other rules |
| AC-5 | The API resolves exactly one effective version per member: active Beta for an eligible selected beta member, otherwise active Published; Draft, Deprecated and Archived versions are excluded per [REQ-007](09-REQ-007-resource-lifecycle.md) |
| AC-6 | Resources belonging to another project are never returned, once [REQ-003](06-REQ-003-server-project-separation.md) lands |
| AC-7 | Enforcement occurs in the API; a test calls the endpoint directly, bypassing the console, and confirms filtering |
| AC-8 | An administrator can preview, for a given resource, exactly which members currently receive it |
| AC-9 | An administrator can preview, for a given member, exactly which resources they currently receive |
| AC-10 | Tests cover the matrix of: tagged and untagged, shared and private, matching and non-matching, included and excluded |
| AC-11 | Creating or updating a Beta accepts explicit member IDs only when every target is active, belongs to the project and already passes the resource policy; otherwise the API rejects the entire change with target-specific diagnostics |
| AC-12 | Negative endpoint tests prove a non-selected member cannot retrieve a Beta by listing changes, requesting its metadata or calling its artifact endpoint directly |
| AC-13 | The administrator preview distinguishes normal policy audience, selected Beta audience, ineligible selections and each member's effective version/channel |
| AC-14 | When a beta target is removed, disabled, excluded or loses policy eligibility, the next desired-state response returns the Published fallback or an authorized tombstone if no Published version exists |

## 5. Out of scope

- Incremental or cursor-based synchronization; that is part of
  [REQ-012](13-REQ-012-resource-sync-client.md).
- Arbitrary per-member version pinning. V1 supports only one explicit-member Beta channel plus the
  Published fallback.
- Time-bounded access grants.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | A filter written in the wrong direction exposes restricted resources | High | AC-10 covers the full matrix; AC-7 tests the API directly |
| 2 | Over-filtering leaves members with nothing and is hard to diagnose | Medium | AC-8 and AC-9 make the effective result visible before it is relied on |
| 3 | The joining query becomes slow as the catalog grows | Low | Tag assignment indexes already exist; measure before optimizing |
| 4 | Beta targeting is treated as an allow list and bypasses the normal resource policy | High | AC-11 defines Beta as an intersection and AC-12 tests direct artifact access |

## 7. Open questions

- When a resource carries several required tags, is the rule intersection or union? The specification's
  example uses "required tags", implying intersection. Confirm, because union is the more intuitive
  default and the two behave very differently once several tags are in use.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-11 | Added explicit-member Beta targeting as a narrowing release-channel policy with fallback and leak-prevention criteria | Codex |
| 2026-08-11 | Accepted into the coordinated governed-resource design by project-owner request | Codex |
