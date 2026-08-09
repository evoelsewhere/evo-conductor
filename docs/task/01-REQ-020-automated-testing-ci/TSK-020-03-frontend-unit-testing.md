# TSK-020-03 — Set up frontend unit testing and linting

| | |
|---|---|
| ID | TSK-020-03 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Todo |
| Layer | FE |
| Requirement | [REQ-020](../../requirement/01-REQ-020-automated-testing-ci.md) |
| Design | [DES-020 section 6](../../design/01-DES-020-automated-testing-ci.md) |
| Covers | AC-3, AC-5 |
| Depends on | none, runs in parallel with TSK-020-01 |
| Estimate | 1d |
| Branch | `feat/REQ-020-frontend-testing` |

## 1. Goal

`apps/web` gains a unit test runner and a linter, both wired to scripts, plus enough real tests to prove
the setup works on this codebase rather than on a hello-world.

`apps/web/package.json` currently declares **no test script and no lint script** — only `dev`, `build`,
`typecheck` and `preview`.

## 2. Files in scope

| File | Action |
|---|---|
| `apps/web/package.json` | modify, add devDependencies and the `test:unit` and `lint` scripts |
| `apps/web/vitest.config.ts` | create |
| `apps/web/eslint.config.js` | create |
| `apps/web/src/shared/api/client.test.ts` | create |
| `apps/web/src/features/resources/pages/resources-page.test.tsx` | create |

## 3. Implementation steps

1. Add devDependencies: `vitest`, `@vitest/coverage-v8`, `@testing-library/react`,
   `@testing-library/user-event`, `jsdom`, `eslint` with the TypeScript and React hooks plugins.
2. `vitest.config.ts`: `jsdom` environment, and reuse the `@` alias already declared in
   `vite.config.ts` so imports resolve identically in test and build.
3. `eslint.config.js`: flat config, TypeScript plus React hooks rules. Do not enable rules the existing
   code cannot satisfy; the goal is a clean baseline, not a refactor.
4. Test `shared/api/client.ts`: with `fetch` mocked, assert that `request` prefixes `/api`, sets
   `Content-Type`, attaches `Authorization: Bearer` when `conductor.token` is present in `localStorage`,
   omits it when absent, and throws on a non-2xx response.
5. Test `resources-page.tsx` in all four render states: loading, empty, error, populated. Wrap in a
   `QueryClientProvider` with retries disabled.
6. Run `bun run lint` and fix what it reports, or narrow the rule set. Do not commit with warnings
   suppressed inline.

## 4. Required tests

### Layer FE (React)

| Type | Tool | Must cover |
|---|---|---|
| Unit | `vitest` | `request<T>` with `fetch` mocked: path prefix, headers, token presence and absence, error throwing |
| Component | `vitest` with `@testing-library/react` | Resources page in loading, empty, error and populated states |
| Role-based rendering | `vitest` with `@testing-library/react` | Deferred to the first task that adds a role-gated screen; no such screen changes here |
| End to end | `playwright` | TSK-020-04 |

Mandatory: the empty state and the error state have tests. On the resources page the empty state also
asserts the text currently promises publishing that the backend does not provide, so the claim is
documented rather than forgotten.

## 5. Commands and reports

```bash
cd apps/web
bun run typecheck
bun run lint
bun run test:unit -- --reporter=verbose
```

## 6. Definition of done

- [ ] `test:unit` and `lint` scripts exist and run
- [ ] API client tests pass
- [ ] Resources page tests cover all four states
- [ ] `bun run lint` is clean with no inline suppressions
- [ ] `bun run typecheck` is clean
- [ ] `bun run build` succeeds
- [ ] Section 7 contains real output

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-3 | `request attaches bearer token` | `client.test.ts` | |
| AC-3 | `request throws on non-2xx` | `client.test.ts` | |
| AC-3 | `resources page renders four states` | `resources-page.test.tsx` | |
| AC-5 | `bun run lint` exits zero | — | |

### Command output

```
<paste the unmodified output>
```

### Screenshots

<!-- Not required: this task changes no visible behaviour. State that explicitly rather than leaving blank. -->

### Notes

<!-- Rules deliberately left disabled, and why. -->

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-09 | Todo | Created |
