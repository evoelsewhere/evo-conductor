# TSK-011-04 — Build the EvoFlux connection experience

| | |
|---|---|
| ID | TSK-011-04 |
| Created | 2026-08-10 |
| Updated | 2026-08-14 |
| Status | Implemented — merged; dedicated Playwright evidence remains |
| Layer | EvoFlux FE |
| Requirement | [REQ-011](../../requirement/12-REQ-011-client-registration.md) |
| Design | [DES-011 sections 5, 7 and 8](../../design/12-DES-011-client-registration.md#8-evoflux-changes) |
| Covers | AC-1, AC-6, AC-9, AC-10 |
| Depends on | TSK-011-03 |
| Estimate | 1.5d |
| Branch | `feat/REQ-011-client-registration` |

## 1. Goal

Give a member a safe way to connect/disconnect Conductor, see joined-project branding, and understand
whether action is needed without exposing the connection token.

## 2. Files in scope

| File | Action |
|---|---|
| `../../../../evoflux/web/src/api/client/settings.ts` | Modify typed connect/disconnect/status client. |
| `../../../../evoflux/web/src/components/settings/ConductorConnectionSettings.tsx` | Modify form, status panel and branding. |
| `../../../../evoflux/web/src/routes/settings.connection.tsx` | Modify screen composition if needed. |
| `../../../../evoflux/web/src/__tests__/components/settings/ConductorConnectionSettings.test.tsx` | Create/modify component tests. |
| `../../../../evoflux/web/e2e/conductor-connection.spec.ts` | Create browser flow if Playwright exists. |

## 3. Implementation steps

1. Show URL/token only while disconnected or reconnecting; pre-validate URL and `evc_` prefix, while
   leaving credential truth to the server.
2. Render separate connecting, connected, offline, authorization-required, forbidden, error and
   disconnected states. Each terminal state states the next action.
3. Show project name/logo only after successful bootstrap, with accessible alternative text and safe logo
   fallback. Do not render token metadata or raw role/tag JSON.
4. Add explicit Disconnect. For REQ-011 it removes the connection immediately and returns to disconnected.
5. Ensure browser-visible errors/debug state never echo submitted token text.

## 4. Required tests

| Type | Tool | Must cover |
|---|---|---|
| Unit/component | `vitest` with `@testing-library/react` | Validation, loading, branding, logo fallback, terminal errors and disconnect. |
| End to end | `playwright` | Connect, connected identity, revoked-token action and disconnect. |
| Accessibility | component test | Labelled inputs, keyboard action, status announcement and logo alt text. |

## 5. Commands and reports

```bash
cd ../evoflux/web
bun run lint
bun run typecheck
bun run test:unit -- ConductorConnectionSettings
bun run test:e2e -- conductor-connection
```

## 6. Definition of done

- [x] Invalid credential and temporary offline service are distinct to members.
- [x] Project identity comes only from successful server bootstrap.
- [x] No rendered/request-debug/error data contains raw token.
- [x] Disconnect leaves visibly disconnected, non-communicating state.
- [ ] Covered ACs have passing tests and screenshots. Component tests pass; dedicated Playwright screenshots are missing.

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-1 | URL and token validation before Connect | `validates the URL and evc_ token before connecting` | Pass |
| AC-6 | Successful bootstrap clears token and renders server-owned branding | `connects, clears the submitted token, and renders server-owned branding` | Pass |
| AC-9 | Terminal/offline copy is implemented | Component/state implementation | Playwright terminal-state proof pending |
| AC-10 | Explicit Disconnect returns to connection inputs | `disconnects explicitly and returns to connection inputs` | Pass |

### Command output

```text
bun run lint       PASS (one unrelated scheduler warning)
bun run typecheck  PASS (verified 2026-08-14)
bun run test:unit -- ConductorConnectionSettings PASS (5 tests; verified 2026-08-14)
bun run test       PASS (historical full-suite evidence: 203 tests across 57 files)
bun run build      PASS
```

### Screenshots

Not captured for the EvoFlux connection flow. Dedicated Playwright evidence remains required before Done.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-10 | Draft planning | Created before design approval at user request |
| 2026-08-10 | In Review | Connection UI implemented by `57b3f6b8`; EvoFlux PR #4 was open |
| 2026-08-14 | Implemented | Current connection/settings UI and component tests remain; dedicated browser screenshots are still missing |
