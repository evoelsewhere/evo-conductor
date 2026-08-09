---
name: conductor-frontend
description: Use when implementing, fixing, or refactoring React/TypeScript code in the Evo Conductor console under apps/web/src — feature pages, the API client, TanStack Router routes and Query hooks, Zustand stores, or the shared UI kit. Covers the feature-module layout, role-gated rendering, and the four render states. Trigger on "add a screen", "build the usage dashboard", "fix the members page", "sửa giao diện conductor", "thêm màn hình cho conductor".
---

# Conductor frontend — React 19, Vite, TanStack, Tailwind 4

The console is an admin surface, not the agent UI. Everything it shows comes from the Rust API on
`:4700`, proxied by Vite from `/api` during development.

## Layout

```
apps/web/src/
  app/                  router, boot gate
  features/<name>/pages/<name>-page.tsx
  shared/api/client.ts  wire types + the single request helper + the api object
  shared/components/    app-shell, page-frame, brand, logo, stat-card, theme-toggle
  shared/ui/            avatar badge badge-list button card dialog empty-state input label
                        menu multi-select select skeleton spinner switch table textarea tooltip
  shared/stores/        auth, theme, ui        (Zustand)
  shared/hooks/         use-media-query
  shared/lib/utils.ts
  styles/               design tokens, aligned with EvoFlux
```

A new screen is a new folder under `features/`, one page component, registered as a route in `app/`. Do
not put feature-specific components in `shared/`; `shared/` is for things at least two features use.

## The API client is the only place that talks to the server

`shared/api/client.ts` holds three things and nothing else: wire types, one `request<T>` helper, and the
exported `api` object.

```ts
async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = localStorage.getItem("conductor.token")
  const headers = new Headers(init?.headers)
  headers.set("Content-Type", "application/json")
  if (token) headers.set("Authorization", `Bearer ${token}`)
  const res = await fetch(`/api${path}`, { ...init, headers })
  // non-2xx throws Error(message)
}
```

Rules:

- Never call `fetch` from a component. Add a method to `api` instead.
- Wire types mirror the Rust `serde` shape exactly, in `snake_case`. Do not camel-case them on the way in;
  the mismatch will bite at the next endpoint.
- The session token lives in `localStorage` under `conductor.token`, the cached user under
  `conductor.user`. This is the browser session, entirely separate from the `evc_` connection token that
  EvoFlux uses — never conflate the two.

## Routing and the boot gate

TanStack Router, defined in `app/`. Route guards run in `beforeLoad` and `throw redirect(...)`:

- `/setup` redirects to `/login` once `api.setupStatus()` reports `configured`.
- `/login` redirects to `/setup` when not configured.
- `/app/*` requires a token, calls `api.me()`, and redirects to `/change-password` when
  `must_change_password` is set. On failure it clears storage and returns to `/login`.

When adding an authenticated screen, nest it under `/app` so it inherits `AppShell` and that guard. Note
the existing catch in the `/app` guard rethrows redirect objects before treating an error as
"unauthenticated" — preserve that shape if you touch it.

## Server data versus client state

- **TanStack Query** owns everything that comes from the server. Use a stable `queryKey`, for example
  `["resources"]`, `["members", filters]`. Invalidate after a mutation rather than refetching by hand.
- **Zustand** (`shared/stores/`) owns auth, theme and UI state only.

Do not mirror server data into a Zustand store. That split is deliberate and matches the convention on
the EvoFlux side.

## Role-gated rendering

The console hides what the current role cannot use. `app-shell.tsx` builds its navigation groups
conditionally from the user's `primary_role`.

**This is a usability measure, never a security measure.** The server must reject the request too. If you
find yourself hiding a button because the endpoint would otherwise leak data, the endpoint is the bug —
see `conductor-backend` and REQ-004.

## Every data screen renders four states

Loading, empty, error, populated. The empty and error states are the ones that break unnoticed, so they
are mandatory in tests.

The empty state must say **why** it is empty. `members_online` currently displays zero forever because
`member_inventory` is never written; a screen that shows a real zero and a screen that has no data yet
must not look identical. Use `EmptyState` from `shared/ui/empty-state.tsx` with a description that
distinguishes them.

Do not let the empty state promise a capability the backend lacks. The resources page currently reads
"Contribute role can also publish shared packages" while no write endpoint exists at all.

## Styling

Tailwind 4 with design tokens defined in `styles/`. The codebase uses the v4 CSS-variable shorthand:

```tsx
<div className="font-mono text-[0.7rem] text-(--color-text-subtle)">{r.slug}</div>
```

Use the token variables rather than raw palette values, so the console stays aligned with EvoFlux's theme
and both light and dark modes keep working. Primitives come from Base UI (`@base-ui/react`) wrapped in
`shared/ui/`; `class-variance-authority` and `tailwind-merge` handle variants. Reach for an existing
primitive before adding a dependency.

## Tests

`apps/web/package.json` currently declares **no test script and no lint script**. REQ-020 adds `vitest`,
`@testing-library/react`, `playwright` and `eslint`. Until then, `bun run typecheck` and `bun run build`
are the only automated checks available — say so rather than claiming tests pass.

Once the tooling exists, a frontend task carries:

- unit tests for pure functions, formatters and the API client with `fetch` mocked,
- component tests for all four render states,
- a role-based rendering test asserting navigation entries and actions are absent for insufficient roles,
- an end-to-end test of the primary flow, with screenshots.

## Commands

```bash
cd apps/web
bun run dev        # or `make dev` from the repo root, which starts the API too
bun run typecheck
bun run build
```

Vite proxies `/api` to `http://127.0.0.1:4700`, so the console needs the API running. `make dev` from the
repo root starts both.

## Internationalization

Display strings are currently inline English. EvoFlux already ships full i18n, so the team is used to
working in Vietnamese. REQ-021 covers extracting strings, and the cost grows with every screen added
first — if you are adding several screens, raise it before rather than after.
