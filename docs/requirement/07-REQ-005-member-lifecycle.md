# REQ-005 — Member lifecycle and disablement

| | |
|---|---|
| ID | REQ-005 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Draft — partial implementation |
| Priority | P0 |
| Build order | Step 7 of 23 |
| Spec section | [requirements.md section 4](../requirements.md) |
| Source | Baseline specification section 4 |
| Depends on | REQ-004, REQ-018 |
| Blocks | V1 acceptance criterion 13 |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

Both member creation flows are already implemented and match the specification. The remaining work is
concentrated in one place: disabling a member currently closes the browser door and leaves the machine
door open.

This is the classic offboarding gap. It becomes materially more serious once resources and documents are
actually being distributed, because a departed member's installation keeps receiving project content.

## 2. Requirement

Member creation, approval and disablement shall behave as described in specification section 4.
Disabling a member shall be a single decisive action that terminates every access path, and an
administrator shall be able to see what was terminated.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Admin-created invited members, temporary-password change, pending SSO approval, enable/disable and password reset | Disabling does not mark all connection-secret rows revoked in the same transaction |
| Browser tokens carry `session_version`; every status/password change invalidates existing sessions | Re-enabling can make an old, unrevoked connection secret usable again, contrary to AC-4 |
| Connection-secret authentication now loads the owner and immediately rejects any non-active owner | Project-wide audit events and the access-path summary required by AC-7/AC-8 |
| Disable actively closes the member's realtime connections; resource/history rows are retained | Token-expiry warnings |

### Acceptance progress

| AC | State |
|---|---|
| AC-2, AC-3, AC-5, AC-6 | Implemented |
| AC-1, AC-4, AC-7, AC-8 | Not complete |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Disabling a member revokes all of their connection tokens within the same database transaction |
| AC-2 | Token validation checks that the owner is `active`; a token whose owner is disabled returns `401` immediately, even if the token itself was not marked revoked |
| AC-3 | Existing browser sessions belonging to a disabled member are rejected on their next request |
| AC-4 | Re-enabling a member does not restore previously revoked tokens; new tokens must be issued |
| AC-5 | Historical telemetry and audit records for a disabled member remain queryable, subject to [REQ-019](21-REQ-019-data-retention.md) |
| AC-6 | Resources owned or published by a disabled member are not deleted or unpublished automatically |
| AC-7 | Every lifecycle transition is written to the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |
| AC-8 | An administrator can see, for any member, the state of each access path: sessions, tokens, and connected installations |

## 5. Out of scope

- Automatically removing already-synchronized content from a departed member's machine. This cannot be
  guaranteed and must not be promised.
- SCIM or directory-driven lifecycle synchronization. Reconsider at P2.
- Deleting a member. Disablement is the supported terminal state.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | A departed member continues to receive project resources | High | AC-1 and AC-2 |
| 2 | Checking owner status on every token call adds a query | Low | Resolve within the same query that validates the token |
| 3 | Re-enabling silently restores stale credentials | Medium | AC-4 |

## 7. Open questions

None. The specification is unambiguous on this section.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-14 | Recorded session invalidation, owner-status token checks and realtime disconnect behavior | Codex |
