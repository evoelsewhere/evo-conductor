# REQ-008 — Resource access policy

| | |
|---|---|
| ID | REQ-008 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Accepted — allow-only policy and effective delivery implemented; preview/validation gaps remain |
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
targeting by primary role, sub-role, tag, explicit member ID, or all project members. The implemented V1
policy is intentionally allow-only; deny/exclusion expressions are deferred.

The current catalog now stores inclusion rules in `resource_access_rules` and `ResourceRepo` evaluates
all-members, role, sub-role, tag and explicit-member subjects for token-authenticated subscription
([resource.rs](../../crates/conductor-storage/src/repos/resource.rs)). The remaining gaps are expanded
effective-audience preview and stronger Beta-target validation. Beta adds a second question:
which immutable released version an already-authorized member receives.

The specification is explicit that this must be enforced server-side, because frontend route hiding is
not security.

## 2. Requirement

Conductor shall attach an access policy to each resource and shall return to each EvoFlux installation
only the released version permitted for the token owner. Enforcement shall occur in the Rust API.

Beta audience is a release-channel selector, not a second access policy. An explicit beta-member set may
only narrow the members who already pass the resource access policy; it may never grant access. For an
eligible selected member, the active Beta is the effective version. For every other eligible member, the
active Published version is effective. Private/no-policy ownership, disabled status and project
separation take precedence over beta selection.

## 3. Implementation status

| Implemented | Missing or incomplete |
|---|---|
| Browser lists are actor-filtered; client snapshot/change/version/artifact/smart-fetch paths all derive the member from the connection secret | Expanded administrator previews by resource and by member |
| `resource_access_rules` stores all-members, primary-role, sub-role, tag and member allow subjects; shared/no-policy defaults to all and private/no-policy defaults to owner | Beta target updates verify active users but do not yet prove that every target passes the normal policy before committing |
| Effective-version resolution returns selected eligible Beta, otherwise Published, and direct version/artifact reads rerun the resolver | Target-specific diagnostics and a dedicated effective-audience endpoint |
| Beta target changes, access changes and archive events feed cursor delivery; smart fetch computes the complete current member tree and safe tombstones | Full direct-endpoint negative matrix across Beta selection, policy changes and disabled members |

### Acceptance progress

| AC | State |
|---|---|
| AC-1–AC-7, AC-12, AC-14 | Implemented for the allow-only V1 contract, except the full Beta eligibility check in AC-11 |
| AC-8–AC-11, AC-13 | Partial |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A resource may be targeted by primary role, sub-role, tag, explicit member ID, or all project members |
| AC-2 | V1 policies are allow-only. Deny/exclusion rules are not accepted or implied; adding them requires a new precedence design and migration |
| AC-3 | A shared resource with no policy is visible to all active project members; a Beta remains limited to its explicit eligible member set |
| AC-4 | A `private` resource with no explicit allow rule is returned only to its owner; explicit allow rules may share it with the named audience, while owner and Admin retain management access |
| AC-5 | The API resolves exactly one effective version per member: active Beta for an eligible selected beta member, otherwise active Published; Draft, Deprecated and Archived versions are excluded per [REQ-007](09-REQ-007-resource-lifecycle.md) |
| AC-6 | Resources belonging to another project are never returned, once [REQ-003](06-REQ-003-server-project-separation.md) lands |
| AC-7 | Enforcement occurs in the API; a test calls the endpoint directly, bypassing the console, and confirms filtering |
| AC-8 | An administrator can preview, for a given resource, exactly which members currently receive it |
| AC-9 | An administrator can preview, for a given member, exactly which resources they currently receive |
| AC-10 | Tests cover tagged and untagged, shared and private, matching and non-matching allow subjects, ownership and no-policy defaults |
| AC-11 | Creating or updating a Beta accepts explicit member IDs only when every target is active, belongs to the project and already passes the resource policy; otherwise the API rejects the entire change with target-specific diagnostics |
| AC-12 | Negative endpoint tests prove a non-selected member cannot retrieve a Beta by listing changes, requesting its metadata or calling its artifact endpoint directly |
| AC-13 | The administrator preview distinguishes normal policy audience, selected Beta audience, ineligible selections and each member's effective version/channel |
| AC-14 | When a beta target is removed, disabled or loses policy eligibility, the next desired-state response returns the Published fallback or an authorized tombstone if no Published version exists |

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
| 2026-08-14 | Aligned V1 with the implemented allow-only resolver and recorded remaining preview/Beta-validation work | Codex |
