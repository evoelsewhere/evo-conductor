# Evo Conductor

Master control plane for **EvoFlux** — centralize agents, skills, MCP, and per-member usage/performance for a software project.

## Package layout

```
evo-conductor/
├── apps/
│   └── web/                 # React console (Vite)
├── crates/
│   ├── conductor-domain/    # pure domain (no I/O)
│   ├── conductor-storage/   # SQLite repos / migrations
│   ├── conductor-auth/      # passwords, JWT, connection tokens
│   └── conductor-server/    # Axum HTTP + binary
├── docs/
├── data/                    # local sqlite (gitignored)
├── Cargo.toml               # Rust workspace
└── Makefile
```

### Rust layers

| Crate | Responsibility |
|---|---|
| `conductor-domain` | Entities, roles, DTOs, domain errors |
| `conductor-storage` | `Db` + `repos::{instance,users,roles,secrets,resources,dashboard}` |
| `conductor-auth` | Argon2, JWT, `evc_` token hashing |
| `conductor-server` | `config` + `http::{routes,extractors,state}` + thin `main` |

### Web layout (`apps/web/src`)

| Path | Responsibility |
|---|---|
| `app/` | Router, boot gate |
| `features/*/` | Feature pages (setup, auth, dashboard, …) |
| `shared/` | API client, UI kit, stores, shell |
| `styles/` | Design tokens (EvoFlux-aligned) |

## Roles

| Primary | Capabilities |
|---|---|
| **admin** | Setup, SSO, members, sub-roles, resource policy, telemetry |
| **contribute** | Publish shared agents/skills/MCP, view team monitoring |
| **user** | Consume catalogs, create secrets, report usage |

Admin defines **sub-roles** (`dev`, `ba`, `tester` by default).

## Quick start

```bash
make dev
```

Starts API (`:4700`) and Vite (`:5174`, proxies `/api`) together. Open http://127.0.0.1:5174

Useful extras: `make reset-db` (fresh setup wizard), `make kill-dev-ports`, `make help`.

## Database

Default is **SQLite**. Switch via `CONDUCTOR_DATABASE_URL`:

```bash
# SQLite (default)
CONDUCTOR_DATABASE_URL=sqlite:data/conductor.db?mode=rwc

# Postgres
CONDUCTOR_DATABASE_URL=postgres://user:pass@127.0.0.1:5432/conductor

# MySQL
CONDUCTOR_DATABASE_URL=mysql://user:pass@127.0.0.1:3306/conductor
```

`GET /api/health` reports the active dialect in `database`.

## Microsoft Entra ID (Azure AD) SSO

1. Create an App Registration (Web) in Entra ID.
2. Redirect URI: `http://127.0.0.1:4700/api/auth/sso/callback` (or your public API URL).
3. Create a client secret.
4. In Conductor setup, enable SSO → provider **Microsoft Entra ID**, issuer:
   `https://login.microsoftonline.com/{tenant-id}/v2.0`
5. Set **Public URL** to the web console origin (e.g. `http://127.0.0.1:5174`) so callback can return to `/auth/callback`.

Flow: `GET /api/auth/sso/start` → Entra login → `GET /api/auth/sso/callback` → redirect to the console. Conductor uses authorization code + PKCE, validates state and nonce, and verifies the ID token against provider JWKS, issuer, and audience. The browser session is transferred in a URL fragment (never a query string) and kept in tab-scoped session storage.

Local and temporary passwords must be at least 12 characters. Password resets, password changes, and account disable/enable operations revoke existing browser sessions immediately. Disabling a member also blocks that member's EvoFlux connection secrets.

## Env

See `.env.example`.
