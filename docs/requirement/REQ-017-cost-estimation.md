# REQ-017 — Cost estimation and budget alerts

| | |
|---|---|
| ID | REQ-017 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P1 |
| Spec section | [requirements.md section 11](../requirements.md), addition |
| Source | Baseline specification section 11 lists estimated cost as a metric; the pricing source is added here |
| Depends on | REQ-014, REQ-016 |
| Blocks | REQ-022 |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

Specification section 9 lists "estimated or reported cost" as a telemetry field and section 11 lists
estimated cost as a dashboard metric, but neither states where the price comes from. Token counts are a
technical figure; the question management asks is about money.

This matters more once a project supplies its own API keys or runs internal models, because the cost then
lands on one budget holder rather than being spread across individual accounts.

## 2. Requirement

Conductor shall convert recorded token usage into an estimated cost using a versioned price table, shall
break cost down by member and group, and shall support threshold alerts.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| Token counts will be available once [REQ-014](REQ-014-telemetry-ingestion.md) lands | `model_pricing` table |
| | Cost computation, display and breakdown |
| | Thresholds and alerting |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A `model_pricing` table stores provider, model, input price per thousand tokens, output price per thousand tokens, currency and effective-from date |
| AC-2 | Prices are versioned by effective date; historical periods are costed at the price in force at that time and are not restated when a price changes |
| AC-3 | Internal or self-hosted models can be assigned a custom unit price or explicitly marked as zero cost |
| AC-4 | Cost breaks down by member, sub-role, tag, model and date |
| AC-5 | A model with no price is displayed as unpriced; it is never silently treated as zero |
| AC-6 | Thresholds can be set per member or per group, and exceeding one raises an alert to Admin |
| AC-7 | Members can see their own cost, per [REQ-015](REQ-015-privacy-controls.md) AC-7 |
| AC-8 | Every figure is labelled as an estimate and shows the date the price table was last updated |
| AC-9 | Changes to the price table are recorded in the audit log ([REQ-018](REQ-018-audit-logging.md)) |

## 5. Out of scope

- Blocking work when a budget is exceeded. Enforcement requires a request path Conductor does not control;
  see [REQ-023](REQ-023-ai-gateway.md). Alerting only at this level.
- Reconciliation against a provider's actual invoice.
- Multiple currencies and exchange-rate conversion.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Estimated figures are mistaken for billed amounts | Medium | AC-8 labels every display |
| 2 | Stale prices produce quietly wrong reporting over a long period | Medium | AC-8 shows the last update date; warn when prices are older than a threshold |
| 3 | Per-member budgets create pressure that discourages tool use | Medium | Prefer group thresholds; keep per-member thresholds optional |

## 7. Open questions

- Are prices entered manually or synchronized from providers? Manual entry is recommended: the set of
  models in real use is small, prices change infrequently, and it avoids a new external dependency.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
