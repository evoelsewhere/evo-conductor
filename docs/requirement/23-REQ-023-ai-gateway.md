# REQ-023 — AI gateway

| | |
|---|---|
| ID | REQ-023 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft, recorded as Deferred by the baseline |
| Priority | Deferred |
| Build order | Step 23 of 23 |
| Spec section | [requirements.md section 10](../requirements.md) defers this explicitly |
| Source | Raised 2026-08-09; the baseline specification then placed it out of scope |
| Depends on | REQ-002, REQ-004, REQ-014 |
| Blocks | the enforcement clauses of REQ-017 and REQ-022 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; deferred |

## 1. Context

Specification section 10 states that LLM provider credentials should remain local to EvoFlux unless
Conductor later introduces a dedicated encrypted secret-management system. This requirement records the
option, the analysis behind it, and the conditions under which it would be reopened. It is not proposed
for the current scope.

The option arises when a project supplies model access centrally, whether through organization API keys,
an internal model deployment, or a shared AI portal.

## 2. Why this is an architectural decision rather than a feature

There are exactly two ways to measure model usage, and they differ in kind rather than in degree.

| | Client-reported, [REQ-014](15-REQ-014-telemetry-ingestion.md) | Gateway-measured, this requirement |
|---|---|---|
| Source of figures | EvoFlux reports its own activity | The request itself, passing through Conductor |
| Trust level | As trustworthy as the client | Non-repudiable |
| Can be disabled | Yes, by changing configuration | No; disabling it removes model access |
| Latency cost | None | One additional network hop |
| Credential location | On the member's machine | Never leaves the server |
| Single point of failure | No | Yes; if the gateway is down, nobody can work |
| Content exposure | None | The gateway sees every prompt in the project |

The last two rows are why the baseline defers this. Conductor would become critical infrastructure with a
much higher availability obligation, and it would sit in the path of every prompt the team writes.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| EvoFlux allows `base_url` configuration on most providers ([factory.py:169-258](../../../evoflux/app/agent/providers/factory.py)), so routing traffic through a gateway would not require changes to the EvoFlux core | The entire gateway layer |
| `ChatCompletionsOnlyProvider` handles OpenAI-compatible providers generically | Upstream credential storage |
| Ollama support exists, so a self-hosted internal model is reachable | Per-member request attribution and usage capture |

## 4. Acceptance criteria, if reopened

| ID | Criterion |
|---|---|
| AC-1 | Conductor exposes an OpenAI-compatible endpoint that EvoFlux can target by setting `base_url` |
| AC-2 | An Admin configures upstream providers, whether organization keys or an internal model endpoint, in one place |
| AC-3 | Upstream credentials never leave the server and never appear in any API response |
| AC-4 | Every request is attributed to exactly one member through their connection token |
| AC-5 | Usage is recorded from the provider's own response rather than estimated client-side |
| AC-6 | Streaming is supported without breaking usage capture |
| AC-7 | Disabling a member or revoking a token blocks the next request immediately |
| AC-8 | A model outside the policy in [REQ-022](22-REQ-022-model-access-policy.md) is refused with a stated reason |
| AC-9 | Request and response content is not stored, unless a collection level that permits it is explicitly enabled per [REQ-015](11-REQ-015-privacy-controls.md) |
| AC-10 | When the gateway is unreachable, EvoFlux reports a clear error and does not hang |
| AC-11 | Gateway-measured usage is distinguishable at the storage layer from client-reported usage |

## 5. Conditions that would reopen this

- The project decides to supply model access centrally rather than having members configure their own
  credentials.
- Usage or cost figures must be non-repudiable rather than self-reported, for example for chargeback.
- Model access policy must be enforced rather than advisory; see
  [REQ-022](22-REQ-022-model-access-policy.md) AC-5.
- The encrypted secret-management system anticipated by specification section 10 exists; see
  [REQ-002](04-REQ-002-configuration-secret-protection.md).

## 6. Risks if implemented

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | The gateway becomes a single point of failure for the whole team | High | Permit fallback to personal credentials; state an availability commitment |
| 2 | The gateway sees every prompt written in the project | High | AC-9; storage disabled by default and governed by [REQ-015](11-REQ-015-privacy-controls.md) |
| 3 | Added latency on every model call | Medium | Measure and publish; stream without buffering |
| 4 | Conductor becomes critical infrastructure with a much higher operational bar | Medium | Run the gateway as a separate process from the console |
| 5 | Centralized organization keys become a concentrated target | High | Requires [REQ-002](04-REQ-002-configuration-secret-protection.md) to be complete first |

## 7. Open questions, if reopened

- Mandatory or optional for members? Mandatory gives complete measurement but makes the gateway a hard
  dependency.
- Only OpenAI-compatible providers and internal models, or also Anthropic, Bedrock and Vertex? The
  narrower scope covers most usage at a fraction of the cost.
- Same process as the console, or separate?

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created; recorded as deferred per specification section 10 | |
