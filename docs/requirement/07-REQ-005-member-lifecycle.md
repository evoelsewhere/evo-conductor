# REQ-005 — Member lifecycle and disablement

| | |
|---|---|
| ID | REQ-005 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
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
| Admin-created member with one-time temporary password and forced password change ([users.rs](../../crates/conductor-server/src/http/routes/users.rs), [auth.rs](../../crates/conductor-server/src/http/routes/auth.rs)) | Link between disabling a member and revoking their tokens | Connection-token validation never checks the owner's status ([resources.rs:31-51](../../crates/conductor-server/src/http/routes/resources.rs)) |
| SSO-created member entering `pending`, with an approval page and an approve action | Notice before token expiry | `UserStatus::can_authenticate()` governs browser sign-in only ([user.rs:52](../../crates/conductor-domain/src/user.rs)) |
| Enable, disable and password reset actions | Audit records for these actions | |
| `invited_by`, `approved_at`, `approved_by` recorded on the user row | | |

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
