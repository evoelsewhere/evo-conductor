# TSK-011-04 — Build the EvoFlux connection experience

| | |
|---|---|
| ID | TSK-011-04 |
| Created | 2026-08-10 |
| Updated | 2026-08-10 |
| Status | Draft planning — do not start until DES-011 is approved |
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

- [ ] Invalid credential and temporary offline service are distinct to members.
- [ ] Project identity comes only from successful server bootstrap.
- [ ] No rendered/request-debug/error data contains raw token.
- [ ] Disconnect leaves visibly disconnected, non-communicating state.
- [ ] Covered ACs have passing tests and screenshots.

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-1 | Not run — planning task | — | Pending |
| AC-6 | Not run — planning task | — | Pending |
| AC-9 | Not run — planning task | — | Pending |
| AC-10 | Not run — planning task | — | Pending |

### Command output

```text
Not run — planning task.
```

### Screenshots

Not captured — planning task.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-10 | Draft planning | Created before design approval at user request |
