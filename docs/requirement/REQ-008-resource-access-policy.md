# REQ-008 — Resource access policy

| | |
|---|---|
| ID | REQ-008 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Spec section | [requirements.md section 7](../requirements.md) |
| Source | Baseline specification section 7 |
| Depends on | REQ-004, REQ-007 |
| Blocks | REQ-012, V1 acceptance criterion 5 |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

Developers, business analysts and testers need different resource sets. The specification requires
targeting by primary role, sub-role, tag, explicit member ID, or all project members, with exclusions.

The raw material already exists. Tags can be attached to resources today because `entity_type` is a
free-form validated string ([access.rs:27-39](../../crates/conductor-server/src/http/routes/access.rs)),
`user_sub_roles` exists, and each resource carries a `visibility` column. What is missing is the policy
table and the query that joins them.

The specification is explicit that this must be enforced server-side, because frontend route hiding is
not security.

## 2. Requirement

Conductor shall attach an access policy to each resource and shall return to each EvoFlux installation
only the published resources permitted for the token owner. Enforcement shall occur in the Rust API.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Token authentication, scope check and expiry check ([resources.rs:17-51](../../crates/conductor-server/src/http/routes/resources.rs)) | `resource_access_policies` table | `subscribe` returns the entire catalog to any valid token ([resources.rs:53](../../crates/conductor-server/src/http/routes/resources.rs)) |
| Generic tag assignment usable for resources | Any targeting logic | `visibility` is stored but never used in a filter |
| `visibility`, `user_sub_roles`, `tag_assignments`, indexes on tag assignment | Policy preview for administrators | Sub-roles and tags remain display-only, which section 3 of the specification warns against |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A resource may be targeted by primary role, sub-role, tag, explicit member ID, or all project members |
| AC-2 | A policy may exclude specific members, and exclusion takes precedence over every inclusion rule |
| AC-3 | A resource with no policy is visible to all project members |
| AC-4 | A `private` resource is returned only to its owner regardless of other rules |
| AC-5 | Only `published` versions are returned; drafts, deprecated-then-archived and archived resources are excluded per [REQ-007](REQ-007-resource-lifecycle.md) |
| AC-6 | Resources belonging to another project are never returned, once [REQ-003](REQ-003-server-project-separation.md) lands |
| AC-7 | Enforcement occurs in the API; a test calls the endpoint directly, bypassing the console, and confirms filtering |
| AC-8 | An administrator can preview, for a given resource, exactly which members currently receive it |
| AC-9 | An administrator can preview, for a given member, exactly which resources they currently receive |
| AC-10 | Tests cover the matrix of: tagged and untagged, shared and private, matching and non-matching, included and excluded |

## 5. Out of scope

- Incremental or cursor-based synchronization; that is part of
  [REQ-012](REQ-012-resource-sync-client.md).
- Version pinning per member. Reconsider at P2.
- Time-bounded access grants.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | A filter written in the wrong direction exposes restricted resources | High | AC-10 covers the full matrix; AC-7 tests the API directly |
| 2 | Over-filtering leaves members with nothing and is hard to diagnose | Medium | AC-8 and AC-9 make the effective result visible before it is relied on |
| 3 | The joining query becomes slow as the catalog grows | Low | Tag assignment indexes already exist; measure before optimizing |

## 7. Open questions

- When a resource carries several required tags, is the rule intersection or union? The specification's
  example uses "required tags", implying intersection. Confirm, because union is the more intuitive
  default and the two behave very differently once several tags are in use.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
