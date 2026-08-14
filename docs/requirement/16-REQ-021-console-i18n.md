# REQ-021 — Console internationalization

| | |
|---|---|
| ID | REQ-021 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Draft |
| Priority | P2 |
| Build order | Step 16 of 23 |
| Spec section | Addition, not present in the baseline |
| Source | Proposed 2026-08-09 |
| Depends on | none |
| Blocks | none |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

The console is English only, with display strings written directly into JSX. EvoFlux already ships full
internationalization, added in commit `2dba74e feat(web): add full app internationalization`, so the team
is accustomed to using the product in Vietnamese.

The cost of extracting strings grows with every screen. The current console has expanded beyond the
original eight screens to include Resource Studio, inventory, analytics and saved-view surfaces, while
display strings are still written directly in English. Retrofitting now is correspondingly larger.

## 2. Requirement

The console shall support Vietnamese and English, shall allow the language to be changed, and shall
remember the choice.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| Current management, Resource Studio, inventory and analytics screens use inline English strings | Internationalization infrastructure |
| | Translations |
| | Locale-aware formatting |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | An internationalization layer exists and no display string remains inline in a component |
| AC-2 | Complete Vietnamese and English translations exist for every current screen |
| AC-3 | Language can be changed from the account menu and the choice persists |
| AC-4 | The initial language follows the browser preference and falls back to English |
| AC-5 | API error messages are translatable, which requires the API to return error codes rather than prose |
| AC-6 | Dates and numbers use locale-appropriate formatting |
| AC-7 | An automated check detects missing translation keys |

## 5. Out of scope

- A third language.
- Right-to-left layout support.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Cost grows with every screen added before this lands | Medium | Consider raising to P1 and doing it before the monitoring screens |
| 2 | Missing keys render raw identifiers in the UI | Low | AC-7 |
| 3 | The API returns hardcoded English prose that cannot be translated | Medium | AC-5 requires error codes; retrofitting this later is disruptive |

## 7. Open questions

- Will the team use the console primarily in Vietnamese? If so this should move to P1 and be completed
  before the usage, audit and document screens are built, rather than after.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-14 | Reconciled the expanded English-only console surface with current source | Codex |
