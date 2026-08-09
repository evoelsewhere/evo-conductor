# REQ-022 — Model access policy

| | |
|---|---|
| ID | REQ-022 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P2 |
| Build order | Step 22 of 23 |
| Spec section | Addition, not present in the baseline |
| Source | Proposed 2026-08-09 |
| Depends on | REQ-007, REQ-012, REQ-016 |
| Blocks | none |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

EvoFlux supports roughly nineteen model providers. A project with data-residency constraints or cost
constraints will need to restrict which of them may be used, for example permitting only internal models
or prohibiting external providers entirely.

This is also where monitoring becomes actionable. Observing that expensive models are in use without any
mechanism to constrain them leaves the dashboard as something to look at rather than something to act on.

## 2. Requirement

An Admin shall be able to declare which providers and models may be used in the project. The policy shall
be distributed to EvoFlux installations and, where a controlled request path exists, enforced on the
server.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| EvoFlux separates availability, capability and adapter support through a shared resolver | Everything in this requirement |
| The resource catalog can carry an arbitrary payload, so a policy document can be distributed through the existing path | A place to declare the policy |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | An Admin can declare permitted providers and models for the project |
| AC-2 | The policy is distributed to EvoFlux through the existing synchronization path ([REQ-008](10-REQ-008-resource-access-policy.md), [REQ-012](13-REQ-012-resource-sync-client.md)) |
| AC-3 | EvoFlux hides or marks models outside the policy in its model selector |
| AC-4 | Where a gateway exists ([REQ-023](23-REQ-023-ai-gateway.md)), a request for a model outside the policy is refused server-side with a stated reason |
| AC-5 | Where no gateway exists, the documentation states plainly that the policy is advisory and not enforced, and the console does not claim otherwise |
| AC-6 | Policy changes are recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |
| AC-7 | The dashboard reports usage of models outside the policy |

## 5. Out of scope

- Different policies per sub-role. Reconsider once a single project-wide policy is in use.
- Content-based restrictions on what may be sent to a provider. This is outside the product's scope.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Enforcement is claimed while the mechanism is only a client-side hint | High | AC-5 states the limitation explicitly |
| 2 | An overly restrictive policy blocks the team from working | Medium | AC-7 allows observation before restriction |

## 7. Open questions

- Allowlist or denylist? Allowlist is recommended: it is safe by default, and the set of models actually
  in use is small.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
