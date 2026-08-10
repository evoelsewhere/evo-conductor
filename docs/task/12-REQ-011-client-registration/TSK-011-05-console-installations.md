# TSK-011-05 — Show installations in the Conductor console

| | |
|---|---|
| ID | TSK-011-05 |
| Created | 2026-08-10 |
| Updated | 2026-08-10 |
| Status | Draft planning — do not start until DES-011 is approved |
| Layer | Conductor FE |
| Requirement | [REQ-011](../../requirement/12-REQ-011-client-registration.md) |
| Design | [DES-011 section 7](../../design/12-DES-011-client-registration.md#7-frontend-changes--conductor-console) |
| Covers | AC-11 |
| Depends on | TSK-011-02 |
| Estimate | 1.5d |
| Branch | `feat/REQ-011-client-registration` |

## 1. Goal

Let authorised administrators understand which EvoFlux installations belong to each member, while members
see their own records and private machine/workspace content stays out of the console.

## 2. Files in scope

| File | Action |
|---|---|
| `apps/web/src/shared/api/client.ts` | Modify typed installation DTO and list query. |
| `apps/web/src/features/members/components/member-installations-panel.tsx` | Create member installation list/status formatter. |
| `apps/web/src/features/members/pages/members-page.tsx` | Modify edit-member dialog to mount panel. |
| `apps/web/src/features/dashboard/pages/overview-page.tsx` | Modify only if compact authorised metric is approved. |
| `apps/web/src/features/members/components/member-installations-panel.test.tsx` | Create component/role tests. |
| `apps/web/e2e/member-installations.spec.ts` | Create browser flow if Playwright exists. |

## 3. Implementation steps

1. Agree server read endpoint with TSK-011-02: each row is label, platform, EvoFlux version, connected
   and last-seen timestamps plus derived active/offline state.
2. Add an accessible installation panel to the existing edit-member dialog with loading, empty, error and
   ordered populated states.
3. Enforce UI/backend policy: administrators inspect project members; a member inspects only self;
   insufficient roles receive no navigation or data.
4. Never render workspace association, hostname, IP, token prefix/value, prompts, source content, paths,
   telemetry or inventory details.
5. If approved, the dashboard has only a count/link; no polling loop beyond normal page query lifecycle.

## 4. Required tests

| Type | Tool | Must cover |
|---|---|---|
| Unit/component | `vitest` with `@testing-library/react` | Loading, empty, error, one member/two installations, last-seen formatting and safe field exclusion. |
| Role-based rendering | `vitest` | Administrator, self, insufficient-role hidden/blocked states. |
| End to end | `playwright` | Authorised member edit dialog shows two rows; unauthorised user cannot navigate/retrieve data. |

## 5. Commands and reports

```bash
cd apps/web
bun run typecheck
bun run lint
bun run test:unit -- member-installations
bun run test:e2e -- member-installations
```

## 6. Definition of done

- [ ] Two installations owned by one member are separate and correctly attributed.
- [ ] Privacy field exclusion and role access are tested, not just documented.
- [ ] Loading, empty and error states are accessible and tested.
- [ ] Covered AC has passing tests and screenshots.

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-11 | Not run — planning task | — | Pending |

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
