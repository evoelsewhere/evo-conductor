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

## Model pricing

Conductor prices telemetry from its own models.dev snapshot. Clients report
usage and never cost — there is no cost field on the ingest contract — so
every figure in the system comes from one method and one member's spend is
comparable with another's. A call Conductor cannot price stays unpriced and
is counted as such, rather than falling back to a number computed elsewhere
and quietly averaged into the total. The catalog refreshes
in the background (`CONDUCTOR_MODEL_PRICING_*` in `.env.example`); rate rows
are append-only and effective from the moment they were synced, so a cost
stays explainable by the rate that was in force when its event was reported.

Each call is priced against the rates that actually applied to it, not just
the model's headline price:

- **Long-context bands** (`cost.tiers`, and the older `cost.context_over_200k`
  spelling). Past a prompt-token threshold the provider bills the *whole*
  request at the higher rates — not only the tokens past the threshold — so a
  300K-token Sonnet turn priced at the headline rate reads about a third low.
  The highest threshold a request clears is the one billed.
- **Alternate service tiers** (`experimental.modes`), the fast and priority
  lanes that bill 2–2.5×. A lane wins over a band where it publishes a rate,
  because it is a different product rather than a volume band. EvoFlux names
  the lane a request actually selected, never one that was merely asked for
  and silently dropped — a lane reported for a call served at the ordinary
  rate would overcharge exactly the calls the caller thought were cheaper.

Both are resolved **per call**, from that call's own prompt size and reported
`service_tier`. Pricing a summed usage total is not equivalent once bands
exist — two 150K calls never clear a 200K threshold, but their sum does — so
aggregates are always summed from per-event costs, never priced as a lump.

What Conductor could not price is reported as `unpriced_model_calls` beside
the total. Those calls' tokens are in the token figures; their cost is
missing from the money figure, which is the honest reading — absent, not
free.

Two panels of that endpoint — `resources` and `activity` — are keyed by
resource attribution rather than by event, so a request that used several
resources is counted in full against each of them. Their tokens and cost rank
versions against one another and deliberately **do not sum to `totals`**. The
`members`, `models`, `roles` and `daily` panels are per event and do add up.

To price model calls Conductor never priced — those ingested before a catalog
existed, or whose model was missing from it at the time:

```bash
evo-conductor reprice
```

That only touches rows a rate actually covers. models.dev publishes no
historical prices, so events older than the first sync have no
period-correct rate; `--estimate-pre-catalog` prices them from the earliest
rate on record and stores `pricing_basis = earliest_known` to mark them as
estimates. Neither mode rewrites a cost already computed from the rate in
force.

That last rule has a consequence worth stating plainly: **band and lane
support is forward-looking**. Calls Conductor already priced at the headline
rate before it read bands keep that cost, because rewriting a figure a
member's receipt already reconciled against is the worse failure. Correcting
them needs a deliberate re-pricing of already-priced history, which `reprice`
does not do.


## Spend limits

A limit **detects, it does not enforce**: EvoFlux calls model providers with
its own credentials, so nothing Conductor does can stop a call from being
made — not even revoking a connection secret, which only cuts off sync and
telemetry. A limit produces a status an operator acts on.

Allowances are one per (scope, subject, period), summed with the same cost
expression the dashboards show, over `received_at`:

```bash
evo-conductor limits set --scope project --period month --limit 2500
evo-conductor limits set --scope member --subject <member-id> --period month --limit 250 --warn 60
evo-conductor limits set --scope role --subject admin --period day --limit 40
evo-conductor limits           # every enabled limit against its current period
evo-conductor limits list      # every limit as configured, disabled ones included
evo-conductor limits rm --scope role --subject admin --period day
```

The console manages them at **/app/spend** (admin only), over these routes:

| | |
|---|---|
| `GET /api/spend-limits` | every allowance with its standing this period |
| `PUT /api/spend-limits` | create or replace one |
| `DELETE /api/spend-limits?scope=&subject_id=&period=` | remove one |
| `GET /api/model-pricing` | catalog version, models priced, whether sync is on |
| `POST /api/model-pricing/sync` | fetch the catalog now |
| `POST /api/model-pricing/reprice[?estimate_pre_catalog=true]` | price what was never priced |

The commands stay: an operator locked out of the console still needs them, and
repricing a long backlog outlives the patience of a browser or a proxy.

`set` and `PUT` replace rather than merge, so writing again is how an allowance
changes and writing without `--disabled` re-enables one. Amounts are dollars,
`--warn` a percentage of the allowance (default 80). Volume Conductor could
not price counts as zero against a limit — a limit cannot charge for a cost
nobody computed — so watch `unpriced_model_calls` in the usage summaries
alongside it.


## Microsoft Entra ID (Azure AD) SSO

1. Create an App Registration (Web) in Entra ID.
2. Redirect URI: `http://127.0.0.1:4700/api/auth/sso/callback` (or your public API URL).
3. Create a client secret.
4. In Conductor setup, enable SSO → provider **Microsoft Entra ID**, issuer:
   `https://login.microsoftonline.com/{tenant-id}/v2.0`
5. Set **Public URL** to the web console origin (e.g. `http://127.0.0.1:5174`) so callback can return to `/auth/callback`.

Flow: `GET /api/auth/sso/start` → Entra login → `GET /api/auth/sso/callback` → redirect to the console. Conductor uses authorization code + PKCE, validates state and nonce, and verifies the ID token against provider JWKS, issuer, and audience. The browser session is transferred in a URL fragment (never a query string) and kept in tab-scoped session storage.

Local and temporary passwords must be at least 12 characters. Password resets, password changes, and account disable/enable operations revoke existing browser sessions immediately. Disabling a member also blocks that member's EvoFlux connection secrets.

## EvoFlux realtime integration

Conductor exposes a server-sent events invalidation plane at `GET /api/v1/realtime/events` and a Git-style smart-fetch data plane at `POST /api/v1/resources/fetch`. Both use an `evc_…` bearer secret with the `subscribe_resources` scope. EvoFlux negotiates its member-specific desired commit, downloads only missing immutable objects, verifies a complete staged tree, then switches generations atomically. The legacy full-snapshot endpoint remains temporarily for compatibility.

See [docs/resource-fetch-protocol.md](docs/resource-fetch-protocol.md) for the normative object/checkout contract and [docs/evoflux-integration.md](docs/evoflux-integration.md) for realtime, capacity, reverse-proxy and rollout details.

The governed catalog supports draft/publish/archive lifecycle, version history, role/team/member access policies, idempotent member usage tracking, effectiveness charts and feedback. See [docs/resource-catalog-product.md](docs/resource-catalog-product.md) for the product model and permission matrix.

For a deterministic public-API load test covering 1,000 members, resource sync,
inventory and attributed telemetry, see [docs/fleet-simulator.md](docs/fleet-simulator.md).

For Local, S3, Azure Blob and Git resource storage, credential handling and provider migration guarantees, see [docs/object-storage.md](docs/object-storage.md).

## Env

See `.env.example`.
