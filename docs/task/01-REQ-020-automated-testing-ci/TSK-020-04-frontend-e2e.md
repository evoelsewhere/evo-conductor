# TSK-020-04 — Set up Playwright and one end-to-end flow

| | |
|---|---|
| ID | TSK-020-04 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Todo |
| Layer | FE |
| Requirement | [REQ-020](../../requirement/01-REQ-020-automated-testing-ci.md) |
| Design | [DES-020 section 6](../../design/01-DES-020-automated-testing-ci.md) |
| Covers | AC-4 |
| Depends on | TSK-020-03 |
| Estimate | 1d |
| Branch | `feat/REQ-020-frontend-e2e` |

## 1. Goal

One end-to-end test proves the console and the API work together: setup wizard, sign-in, and the
authenticated shell. This is the only test in the suite that exercises the real Rust server.

## 2. Files in scope

| File | Action |
|---|---|
| `apps/web/package.json` | modify, add `@playwright/test` and the `test:e2e` script |
| `apps/web/playwright.config.ts` | create |
| `apps/web/e2e/first-run.spec.ts` | create |

## 3. Implementation steps

1. Add `@playwright/test` and a `test:e2e` script. Configure the HTML reporter, and screenshot plus trace
   on failure only.
2. Configure `webServer` so Playwright starts the stack itself rather than assuming it is running. The API
   must use a **throwaway database**, not the developer's `data/conductor.db`; set
   `CONDUCTOR_DATABASE_URL` to a temporary file path for the run.
3. Write `first-run.spec.ts` covering the real first-run path, which is the only flow that works today
   end to end:
   - a fresh instance redirects to `/setup`,
   - completing the wizard creates the project and the admin,
   - signing in as that admin reaches `/app`,
   - the sidebar shows the project name entered during setup.
4. Assert on user-visible text and roles, not on CSS classes.
5. Confirm the run is repeatable: two consecutive runs both pass, which fails if the database is not
   actually reset.

## 4. Required tests

### Layer FE (React)

| Type | Tool | Must cover |
|---|---|---|
| End to end | `playwright` | Setup wizard, sign-in, authenticated shell, project branding visible |
| Unit | `vitest` | TSK-020-03 |
| Component | `vitest` with `@testing-library/react` | TSK-020-03 |

## 5. Commands and reports

```bash
cd apps/web
bunx playwright install --with-deps chromium
bun run test:e2e -- --reporter=html
```

Report at `apps/web/playwright-report/`.

## 6. Definition of done

- [ ] `test:e2e` starts the stack on its own
- [ ] The run uses a throwaway database and never touches `data/conductor.db`
- [ ] The first-run spec passes twice in a row from a clean state
- [ ] HTML report produced, screenshot captured on failure
- [ ] Section 7 contains real output and a screenshot of the authenticated shell
- [ ] `bun run build` still succeeds

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-4 | `first run completes setup and signs in` | `e2e/first-run.spec.ts` | |

### Command output

```
<paste the unmodified output>
```

### Screenshots

<!-- Required: the authenticated shell showing the project name from the wizard. -->

### Notes

<!-- Flakiness observed and how it was addressed; anything that had to be waited on explicitly. -->

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-09 | Todo | Created |
