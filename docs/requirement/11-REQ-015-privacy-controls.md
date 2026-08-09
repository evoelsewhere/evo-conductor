# REQ-015 — Privacy controls and collection levels

| | |
|---|---|
| ID | REQ-015 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Build order | Step 11 of 23 |
| Spec section | [requirements.md section 10](../requirements.md) |
| Source | Baseline specification section 10, extended 2026-08-09 |
| Depends on | REQ-004 |
| Blocks | REQ-012, REQ-013, REQ-014, REQ-016 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

EvoFlux is positioned as a local-first product. The moment usage data flows to a server, what preserves
user trust is that members know exactly what was sent.

This is a product risk rather than a technical one. A monitoring system that members do not trust will be
worked around rather than used honestly, and the resulting data becomes worthless for the decisions it
was collected to support.

The existing `TelemetrySnapshot` type is already content-free, which is the correct starting point and
should be preserved as the schema grows.

## 2. Requirement

Conductor shall define, publish and enforce a project collection level. By default EvoFlux shall not
upload prompt or response content, source code, terminal output, tool arguments containing project data,
document content, environment variables, credentials, or local file paths beyond an approved normalized
identifier.

## 3. The collection level decision

Specification section 10 permits detailed audit content under conditions but does not define the
intermediate ground. Answering what a member used the system for cannot be done from counters alone, so
one of three levels must be chosen deliberately rather than drifted into.

| Level | Collected | Answers "used for what" | Cost |
|---|---|---|---|
| L0 | Mode, agent or prompt used, tool mix, counts, durations | At the level of work category | Safe, but too vague to act on |
| L1 | L0 plus agent-generated session title, task name, and repository identifier if enabled | At the level of a concrete task | Titles may carry incidental context |
| L2 | Full prompt and response content | Completely | Becomes a surveillance system and changes the relationship with the team |

L1 is recommended as the default. It answers the real operational question, which is what the team is
using AI for, without turning Conductor into a system that reads people's work.

## 4. Implementation status

| Implemented | Missing |
|---|---|
| `TelemetrySnapshot` carries counters only, with no content ([telemetry.rs](../../crates/conductor-domain/src/telemetry.rs)) | Collection-level configuration and enforcement |
| | Any client-side redaction |
| | Member-facing disclosure |
| | The personal transparency view |

## 5. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | The project collection level is stored as configuration and is readable by every member without special permission |
| AC-2 | The active level is returned to EvoFlux at registration, per [REQ-011](12-REQ-011-client-registration.md) AC-5 |
| AC-3 | EvoFlux enforces the level before transmission; content that the level excludes is never placed on the wire |
| AC-4 | Local file paths are normalized to an approved identifier; absolute paths are never transmitted |
| AC-5 | Enabling L2 requires all four conditions from specification section 10: explicit Admin action, visible disclosure to members, a restricted viewer list, and a retention period |
| AC-6 | Changing the collection level is recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |
| AC-7 | A member can view a personal page showing exactly the fields an administrator can see about them, no more and no less |
| AC-8 | An automated test asserts that the field set visible to an administrator about a member is a subset of the field set visible to that member about themselves |
| AC-9 | A plain-language explanation of what is and is not collected is available in the console to all members |
| AC-10 | A schema test asserts that no telemetry or inventory field can carry conversation content, file content or credentials at L0 or L1 |

## 6. Out of scope

- Allowing individual members to opt out. Collection level is a project policy, not a personal setting.
- Personal data deletion on request. Reconsider at P2 if a legal obligation applies.
- Holding LLM provider credentials, which specification section 10 places outside scope; see
  [REQ-023](23-REQ-023-ai-gateway.md).

## 7. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Usage data is used to evaluate individuals, so the team learns to avoid it and the data loses value | High | AC-7, AC-8 and AC-9 make collection symmetric and visible; ranking members is explicitly out of scope in [REQ-016](17-REQ-016-usage-aggregation-dashboards.md) |
| 2 | Gradual drift from L1 to L2 without an explicit decision | High | AC-5 makes L2 a governed action rather than a configuration flag |
| 3 | Session titles unintentionally contain sensitive context | Medium | Allow members to edit or suppress a title before it is transmitted |
| 4 | The personal view falls behind the administrator view and becomes an empty promise | Medium | AC-8 turns the principle into a test |

## 8. Open questions

- **Narrowed.** Acceptance criterion 15 in [requirements.md section 16](../requirements.md) states that
  prompts are not uploaded by default, which rules out L2 as the V1 default. The remaining choice is
  between L0 and L1. L1 is recommended, and it stays consistent with criterion 15 because it carries an
  agent-generated session title rather than prompt content.
- Should repository names be transmitted at L1 by default, or only when the project enables it?
  Recommendation: disabled by default.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
