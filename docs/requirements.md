# Evo Conductor — Project Workspace Requirements

| | |
|---|---|
| Version | 2.0 |
| Created | 2026-08-09 |
| Status | Draft, pending acceptance |
| Baseline | Product specification supplied by the project owner, 2026-08-09 |
| Code reviewed | `evo-conductor` at `18d9fe1`, `evoflux` at branch `thangtq42` |

This document is the source specification. Every statement under "Implementation status" was verified by
reading the referenced file and line; nothing in those subsections is inferred. Sections marked
**Addition** were not present in the supplied baseline and are proposed here.

Each numbered section below maps to one or more requirement documents under
[requirement/](requirement/). See [README.md](README.md) for the acceptance workflow.

---

## 1. Product objective

Evo Conductor shall be the centralized control plane for a software project whose members use EvoFlux.
Each project shall have a shared Conductor workspace where authorized users can:

- Manage project members and access.
- Distribute approved agents, skills, MCP servers, workflows, commands, and documents.
- Monitor EvoFlux adoption, usage, tool activity, and connected installations.
- Define project-wide policies and configuration.
- Review audit history without automatically collecting sensitive source code or conversations.

For V1:

```
One Conductor deployment = one project workspace
```

Multi-project organization support can be introduced later.

### Implementation status

The current code implements this constraint structurally rather than by policy. One deployment can only
ever serve one project:

- `bind_host` and `bind_port` are stored inside the `instance` table
  ([migrate.rs:11-12](../crates/conductor-storage/src/migrate.rs)); server configuration and project
  configuration are the same record.
- `jwt_secret` is also stored on that row and read once at startup with `LIMIT 1`
  ([instance.rs:193](../crates/conductor-storage/src/repos/instance.rs),
  [state.rs:27](../crates/conductor-server/src/http/state.rs)); there is one signing key per process.
- `sso_config` is a hardcoded singleton row addressed by `WHERE id = 1`.
- `UPDATE instance SET ...` is issued with no `WHERE` clause
  ([instance.rs:352](../crates/conductor-storage/src/repos/instance.rs)).
- `users.email`, `sub_roles.slug`, `tags.slug` and `resources(kind, slug)` are all globally unique.

### Addition — prepare for multi-project without building it

Moving from single-project to multi-project later is not an additive change. It touches identity
(globally unique email), authentication (one signing key per process) and every slug namespace. The
reverse migration is trivial. Because the cost is asymmetric and the system currently holds no
production data, the following should be done now even though V1 remains single-project:

- Move `jwt_secret`, `bind_host` and `bind_port` out of `instance` into a separate `server_config` table.
- Keep user identity global, so one person can later belong to several projects with one account.
- Add `project_id` to the business tables while they are still empty.
- Do not build multi-project navigation or UI.

See [REQ-003](requirement/REQ-003-server-project-separation.md).

---

## 2. System model

```
Project administrators
        |
Evo Conductor Web Console
        |
Evo Conductor Rust API
        |
PostgreSQL in production
        |
EvoFlux desktop installations
```

Each member runs EvoFlux locally. EvoFlux connects to the project's Conductor server using a scoped
connection token.

Conductor is not the agent runtime. Agents and tools continue running on the user's machine. Conductor
distributes configuration and receives controlled telemetry.

### Implementation status

The Rust API, web console and storage layer exist and are layered cleanly
(`conductor-domain`, then `conductor-storage`, then `conductor-auth`, then `conductor-server`, see
[architecture.md](architecture.md)). Storage uses `sqlx` with the `Any` driver, so PostgreSQL is already
reachable through configuration.

The EvoFlux side of this diagram does not exist. Searching the entire `evoflux` repository for the string
`conductor` across Python, TypeScript, Rust and Markdown returns zero matches. The integration currently
exists only on the server side.

---

## 3. User roles

### Admin

An Admin shall be able to:

- Configure project identity, branding, SSO, and policies.
- Create, approve, update, enable, and disable members.
- Reset member passwords.
- Manage primary roles, sub-roles, and tags.
- Publish and retire shared resources.
- Define resource access policies.
- View project-wide usage and audit logs.
- Manage data-retention settings.

### Contributor

A Contributor shall be able to:

- View active project members.
- Publish and update shared resources.
- Manage resource tags.
- View project usage and performance dashboards.
- Not change project settings, SSO, or admin membership.

### User

A User shall be able to:

- Authenticate to Conductor.
- Create and revoke personal EvoFlux connection tokens.
- Connect a local EvoFlux installation.
- Consume resources assigned to the user.
- View personal usage and connected devices.
- Not view project-wide telemetry unless explicitly authorized.

### Sub-roles and tags

Sub-roles such as `developer`, `business-analyst`, and `tester` shall support project-specific
classification.

Tags shall support grouping entities such as members, agents, skills, MCP servers, documents, teams and
environments.

Sub-roles and tags shall become usable in resource access policies, not remain display-only metadata.

### Implementation status

The role model is implemented and the capability predicates already exist
([role.rs:33-56](../crates/conductor-domain/src/role.rs)). Sub-roles and tags are implemented, and tag
assignment is already generic over entity type: `entity_type` is a free-form validated string
([access.rs:27-39](../crates/conductor-server/src/http/routes/access.rs)), so tagging a resource works
today without schema changes.

Two gaps contradict the requirement text:

- `can_view_telemetry()` is defined but **is not called anywhere in the codebase**. `GET /api/dashboard`
  requires only an authenticated session, with no role check at all
  ([dashboard.rs:8-13](../crates/conductor-server/src/http/routes/dashboard.rs)). `GET /api/resources`
  behaves the same way. The requirement that a User shall not view project-wide telemetry is therefore
  not enforced.
- Sub-roles and tags are currently display-only, exactly as the requirement warns against. No query
  anywhere joins them to resource visibility.

See [REQ-004](requirement/REQ-004-api-authorization.md) and
[REQ-008](requirement/REQ-008-resource-access-policy.md).

---

## 4. Member lifecycle

### Admin-created member

1. Admin enters the member's email, display name, primary role, sub-roles, and tags.
2. Conductor creates an `invited` account.
3. Conductor generates a one-time temporary password.
4. The temporary password is displayed only once.
5. The member signs in and must set a permanent password.
6. The account becomes `active`.

### SSO-created member

1. User authenticates using the configured OIDC provider.
2. If the email is unknown, Conductor creates a `pending` account.
3. The user is redirected to a pending-approval page.
4. Admin assigns roles and approves the account.
5. The user can then sign in and access the project.

### Disabled member

When a member is disabled:

- Existing browser sessions shall be rejected.
- EvoFlux connection tokens owned by that member shall stop working.
- Historical telemetry and audit records shall remain available according to retention policy.
- The member's resources shall not be deleted automatically.

### Implementation status

Both creation flows are implemented end to end, including the one-time temporary password, the forced
password change, the pending-approval page and the approval action
([users.rs](../crates/conductor-server/src/http/routes/users.rs),
[auth.rs](../crates/conductor-server/src/http/routes/auth.rs)).

The disabled-member requirement is only half met. `UserStatus::can_authenticate()` gates browser login
([user.rs:52](../crates/conductor-domain/src/user.rs)), but connection-token validation never checks the
owner's status ([resources.rs:31-51](../crates/conductor-server/src/http/routes/resources.rs)). A
disabled member's EvoFlux installation keeps synchronizing indefinitely. This is the classic offboarding
hole and it is present today.

See [REQ-005](requirement/REQ-005-member-lifecycle.md).

---

## 5. EvoFlux project connection

A member shall connect EvoFlux to Conductor by providing:

- Conductor server URL.
- Project connection token beginning with `evc_`.
- Optional local workspace association.

EvoFlux shall validate the connection and retrieve:

- Project identity and branding.
- Current member identity and permissions.
- Assigned resources.
- Project policies.
- Telemetry and privacy configuration.

Tokens shall:

- Be shown only once.
- Be hashed at rest.
- Have configurable scopes.
- Support expiration and revocation.
- Be rejected when their owner is disabled.
- Be stored by EvoFlux in the operating system's secure credential store.

Supported scopes should include:

```
subscribe_resources
report_telemetry
sync_inventory
read_documents
```

### Implementation status

Token generation, one-time display, SHA-256 hashing at rest, scopes, expiry and revocation are
implemented ([secret_token.rs](../crates/conductor-auth/src/secret_token.rs),
[secrets.rs](../crates/conductor-server/src/http/routes/secrets.rs)).

Two defects:

- `POST /api/secrets` performs no role check, and when the client omits `scopes` the server **grants all
  three scopes by default** ([secrets.rs:31-38](../crates/conductor-server/src/http/routes/secrets.rs)).
  Any ordinary User can mint a fully privileged token.
- Token validation does not check owner status, as described in section 4.

The `read_documents` scope does not exist yet. The client side of this section does not exist at all.

See [REQ-006](requirement/REQ-006-connection-tokens.md) and
[REQ-011](requirement/REQ-011-client-registration.md).

---

## 6. Resource management

Conductor shall support these resource types:

- Agent
- Skill
- MCP configuration
- Workflow
- Command
- Document or project policy
- Optional reusable prompt/template

Each resource shall contain a stable ID and slug, name and description, resource type, semantic version,
payload or artifact reference, content checksum, owner and publisher, visibility, lifecycle status
(`draft`, `published`, `deprecated`, `archived`), created and updated timestamps, access policy, tags,
and change notes.

Admin and Contributor users shall be able to create a draft, validate its payload, publish a version,
update or deprecate a resource, view version history, roll back to a previous version, and see which
EvoFlux installations have synchronized it.

EvoFlux shall only receive published resources that match the current user's access policy.

### Implementation status

The domain types exist ([resource.rs](../crates/conductor-domain/src/resource.rs)) and `payload` is a
free-form JSON value stored as `TEXT`, so it can already carry a Markdown agent definition with no schema
change.

Everything else in this section is missing. `ResourceRepo` has exactly one method, `list()`
([resource.rs](../crates/conductor-storage/src/repos/resource.rs)). There is no write path of any kind,
so the `resources` table is permanently empty. The console already promises the opposite: the empty state
reads "Contribute role can also publish shared packages"
([resources-page.tsx:38](../apps/web/src/features/resources/pages/resources-page.tsx)).

Specific gaps against the requirement text:

- No lifecycle status column. Nothing distinguishes draft from published.
- No checksum, no change notes, no publisher field.
- `UNIQUE(kind, slug)` plus a single `version` column means an update **overwrites** the previous
  content. There is no history and no rollback.
- `ResourceKind::Command` exists in the enum but is not counted in `ResourceCounts`, which counts only
  agents, skills, mcp and workflows. This is a small defect, but it is the exact failure mode to expect
  when new resource types are added faster than the layers beneath them.

### Addition — validate payload size at publish time

EvoFlux truncates instruction content at 128 KB
([workspace_instructions.py:25](../../evoflux/app/agent/hooks/workspace_instructions.py)) and at 64 KB for
per-repository `AGENTS.md` ([multi_repo_context.py:16](../../evoflux/app/agent/hooks/multi_repo_context.py)).
Truncation happens silently on the member's machine. If Conductor does not reject oversized payloads at
publish time, an administrator will never learn that half of a published document is being discarded.

### Addition — MCP requires a stricter path than other resource types

An MCP server definition contains an executable command. Distributing MCP configuration to member
machines is remote code execution by configuration. A bad prompt degrades answers; a bad MCP definition
starts an unknown process on every machine in the project. These two cases must not share a trust level
even though they share a table.

See [REQ-007](requirement/REQ-007-resource-lifecycle.md) and
[REQ-010](requirement/REQ-010-mcp-distribution-safety.md).

---

## 7. Resource access policy

A resource may be targeted using primary roles, sub-roles, tags, explicit member IDs, or all project
members.

Example:

```
Resource: production-database-mcp
Allowed primary roles: admin, contribute
Required tags: backend
Excluded users: contractors
```

Access checks must be enforced by the Rust API. Frontend route hiding is not sufficient security.

### Implementation status

None of this exists. `subscribe` returns the complete catalog to any valid token
([resources.rs:53](../crates/conductor-server/src/http/routes/resources.rs)), ignoring both `visibility`
and tags.

The raw material is already present: `visibility` is stored per resource, `tag_assignments` accepts
arbitrary entity types, and `user_sub_roles` exists. Only the joining query and the policy table are
missing.

See [REQ-008](requirement/REQ-008-resource-access-policy.md).

---

## 8. Inventory synchronization

EvoFlux shall periodically report its local project inventory: EvoFlux version, operating system,
installation or device identifier, last connection time, active local workspace identifier, installed or
synchronized agents, installed skills, configured MCP servers, workflow versions, and resource sync
errors.

The inventory endpoint shall support idempotent updates so repeated heartbeats do not create duplicate
records.

Admin and Contributor users shall be able to identify online and offline installations, outdated EvoFlux
versions, missing required resources, resource-version drift, and failed synchronization.

### Implementation status

A `member_inventory` table exists with a small subset of these fields
([migrate.rs:124-131](../crates/conductor-storage/src/migrate.rs)) and a `MemberPresence` type is defined
([telemetry.rs](../crates/conductor-domain/src/telemetry.rs)). There is no endpoint that writes to it.

The consequence is visible today: the dashboard computes `members_online` from
`member_inventory` ([dashboard.rs](../crates/conductor-storage/src/repos/dashboard.rs)), and because the
table is never written, that figure is permanently zero. The monitoring screen currently displays a
fabricated number.

The existing table is also keyed by `user_id` alone, which cannot represent one member with two machines.
The requirement calls for an installation identifier, so this needs a `client_installations` table keyed
by installation rather than by user.

See [REQ-013](requirement/REQ-013-inventory-synchronization.md).

---

## 9. Usage and telemetry

EvoFlux shall send batched telemetry containing: user ID derived from the connection token, installation
ID, session ID, local project or workspace ID, model provider and model identifier, input and output
token counts, estimated or reported cost, tool name and category, MCP server and MCP tool name, tool-call
status and duration, number of active agents, session start and end timestamps, error category, and
EvoFlux version.

Telemetry events shall support client-generated event IDs, idempotent ingestion, batch submission, retry
after temporary network failure, configurable retention, and server-side aggregation.

### Implementation status

A `telemetry_events` table exists but carries only `tokens_in`, `tokens_out`, `tool_calls`,
`active_agents` and `reported_at` ([migrate.rs:133-143](../crates/conductor-storage/src/migrate.rs)).
It has no tool name, no model, no installation, no session times and no error category. There is no
ingestion endpoint, no aggregation, and no client.

The table also has **no index of any kind**, while five indexes were created for `users` and `tags` in the
same migration. Any query filtered by member or by date will perform a full table scan.

### Addition — offline buffering and clock skew

EvoFlux is a local-first desktop application and will regularly be offline. Two consequences follow that
the baseline does not state:

- Events must be queued locally and replayed. Without client-generated event IDs and server-side
  de-duplication, a replay after reconnection double-counts every event in the queue.
- The client clock cannot be trusted. Each event needs both `client_reported_at` and a
  server-assigned `server_received_at`, and aggregation must key on server time.

### Addition — daily aggregation is required, not an optimization

Charts must read from a `usage_aggregates` table rather than scanning raw events. At one event per turn
across a team, the raw table reaches millions of rows within weeks. Building the aggregate later means
rewriting every query that was written against the raw table first.

See [REQ-014](requirement/REQ-014-telemetry-ingestion.md) and
[REQ-016](requirement/REQ-016-usage-aggregation-dashboards.md).

---

## 10. Privacy requirements

By default, EvoFlux must not upload prompt or response content, source code, terminal output, tool
arguments containing project data, document content, environment variables, API keys or provider
credentials, or local file paths beyond an explicitly approved normalized identifier.

Conductor should receive metadata and metrics by default.

If a project requires detailed audit content, that collection must be explicitly enabled by an Admin,
clearly shown to members, limited by policy, redacted before transmission, covered by a retention period,
and recorded in the audit log.

LLM provider credentials should remain local to EvoFlux unless Conductor later introduces a dedicated
encrypted secret-management system.

### Implementation status

The existing `TelemetrySnapshot` type already respects this boundary: it carries counters only and no
content ([telemetry.rs](../crates/conductor-domain/src/telemetry.rs)). This is the correct starting point
and should be preserved as the schema grows.

### Addition — members must be able to see their own record

A monitoring system that members cannot inspect will be worked around rather than trusted, and the
resulting data becomes worthless. The cheapest structural defense is symmetry: anything an administrator
can see about a member, that member shall be able to see about themselves. Enforcing this as an automated
test turns the privacy policy from a written promise into a property of the system.

### Addition — the tension between "what was it used for" and "no content"

Answering what a member used the system for cannot be done from counters alone. Three collection levels
are possible, and one must be chosen deliberately rather than drifted into:

| Level | Collected | Answers "used for what" | Cost |
|---|---|---|---|
| L0 | Mode, agent or prompt used, tool mix, counts, durations | At the level of work category | Safe but vague |
| L1 | L0 plus agent-generated session title, task name, repository identifier if enabled | At the level of concrete task | Titles may leak incidental context |
| L2 | Full prompt and response content | Completely | Becomes a surveillance system |

L1 is recommended as the default. L2 must satisfy the four conditions the baseline already states in this
section, and must never become a configuration flag that can be flipped without process.

See [REQ-015](requirement/REQ-015-privacy-controls.md) and
[REQ-019](requirement/REQ-019-data-retention.md).

---

## 11. Dashboard requirements

### Project overview

The dashboard shall display total, active, pending, invited, and disabled members; currently connected
EvoFlux installations; active connection tokens; resource counts by type; resource synchronization
health; SSO status; and recent administrative activity.

### Usage dashboard

Authorized users shall be able to filter by date range, member, team or tag or sub-role, model and
provider, agent, tool, MCP server, and EvoFlux installation.

Metrics shall include input and output tokens, estimated cost, sessions, tool calls, tool success and
failure rate, average tool duration, active agents, usage trend over time, and highest-usage members and
resources.

A regular User should only see personal usage unless granted broader permission.

### Implementation status

`DashboardSummary` provides counts only, and as noted in section 8 one of those counts is always zero
([dashboard.rs](../crates/conductor-storage/src/repos/dashboard.rs)). There is no usage dashboard, no
filtering and no time series. `ResourceCounts` omits `Command`.

### Addition — cost requires a priced model table

The baseline lists estimated cost as a metric but does not state where the price comes from. Token counts
are a technical number; management asks about money. A `model_pricing` table is required, and it must be
versioned by effective date so that historical periods are costed at the price that applied then rather
than being silently restated when a provider changes rates. Models with no price must display as
"unpriced" rather than as zero.

### Addition — distinguish "no data" from "zero"

An empty monitoring screen must state why it is empty: nobody has connected yet, versus no activity in
the selected range. A dashboard that shows zero forever is worse than no dashboard, and the current code
already demonstrates that failure.

See [REQ-016](requirement/REQ-016-usage-aggregation-dashboards.md) and
[REQ-017](requirement/REQ-017-cost-estimation.md).

---

## 12. Document management

Conductor shall support project documents such as coding standards, architecture guidelines, security
policies, migration rulebooks, team onboarding instructions, and MCP usage policies.

Documents shall support Markdown and file attachments, versioning, tags, role-based access, publication
status, checksums and synchronization state, and optional local caching in EvoFlux.

Document content must not be confused with user-generated session artifacts. Only explicitly published
project documents should be synchronized.

### Implementation status

Nothing exists. There is no document type, table or endpoint.

### Addition — how documents reach the agent, and the trap to avoid

EvoFlux already consumes project instruction files; this does not need to be built. `WorkspaceInstructionsHook`
appends the `AGENTS.md` of every workspace root to the system prompt of every model call, loads nested
directories on demand, and blocks a mutating tool call once so the model is forced to read newly
applicable rules before editing ([workspace_instructions.py:30-70](../../evoflux/app/agent/hooks/workspace_instructions.py)).

Conductor therefore only has to place the file correctly. There are three candidate locations and only
one is correct:

- **Do not write `AGENTS.override.md`.** The name suggests augmentation, but the loader returns either the
  override or the standard file, never both
  ([workspace_instructions.py:194-199](../../evoflux/app/agent/hooks/workspace_instructions.py)). Writing
  this file silently discards the project's own instructions.
- **Do not overwrite `AGENTS.md`.** It normally lives inside the repository and is tracked by git.
- **Write into a Conductor-owned directory outside the repository and register it as an extra workspace
  root.** The hook merges roots as `[workspace, *extra]`
  ([workspace_instructions.py:44-48](../../evoflux/app/agent/hooks/workspace_instructions.py)), so
  Conductor content is injected alongside project content, touching neither git nor the project's own
  instructions.

Note that `extra_workspace_paths` is also a sandbox root
([sandbox.py:120-123](../../evoflux/app/agent/sandbox.py)), so agents will be able to read that directory.

See [REQ-009](requirement/REQ-009-document-management.md) and
[REQ-012](requirement/REQ-012-resource-sync-client.md).

---

## 13. Audit logging

Conductor shall record security and administrative actions: member created, approved, updated, enabled or
disabled; role or tag assignment changed; connection token created or revoked; project settings changed;
SSO configuration changed; resource published, updated, deprecated or archived; retention or telemetry
policy changed.

Every audit record shall contain actor, action, target type and ID, timestamp, result, safe change
summary, and request correlation ID.

Secrets and raw passwords must never appear in audit records.

### Implementation status

There is **no audit table anywhere in the migration**. A few actions emit a `tracing` line to stdout, for
example [setup.rs:90-94](../crates/conductor-server/src/http/routes/setup.rs), which is not queryable and
not durable. The `users` table carries `invited_by`, `approved_at` and `approved_by`, which are isolated
fragments of the same idea.

For a system that holds the team's accounts, tokens and permissions, and that is about to push
configuration onto other people's machines, this is a governance gap rather than a deferrable feature. It
is a prerequisite for resource publishing, not a follow-up to it.

### Addition — record failed authorization attempts, and audit reads of audit data

Two events belong in the log that the baseline does not list:

- Actions rejected for insufficient permission, which is the signal of permission probing.
- Viewing another member's individual usage data. If per-member audit exists, reading it is itself an
  action that should be attributable.

See [REQ-018](requirement/REQ-018-audit-logging.md).

---

## 14. Required backend APIs

The target API should include at least:

```
# EvoFlux connection
POST /api/v1/client/register
POST /api/v1/client/heartbeat
PUT  /api/v1/client/inventory
POST /api/v1/telemetry/batch

# Resource synchronization
GET  /api/v1/resources
GET  /api/v1/resources/changes?cursor=...
GET  /api/v1/resources/{id}/versions/{version}

# Resource administration
POST   /api/resources
PATCH  /api/resources/{id}
POST   /api/resources/{id}/publish
POST   /api/resources/{id}/deprecate
GET    /api/resources/{id}/versions

# Monitoring
GET /api/usage/summary
GET /api/usage/members
GET /api/usage/tools
GET /api/usage/mcp
GET /api/inventory
GET /api/audit-events
```

### Implementation status

Of the endpoints listed above, exactly one exists in any form: `GET /api/v1/subscribe/resources`
([routes/mod.rs:61](../crates/conductor-server/src/http/routes/mod.rs)), which corresponds to
`GET /api/v1/resources` but returns the unfiltered catalog. Every other endpoint in this section is
unimplemented.

The existing router already separates session-authenticated routes from token-authenticated ones by
convention rather than by structure. As the `/api/v1/client/*` family is added, that separation should
become explicit, because the two families have different authentication, different error semantics and
different rate characteristics.

### Addition — document management and personal usage endpoints

The baseline API list omits two areas it requires elsewhere:

```
# Documents (section 12)
GET    /api/v1/documents
GET    /api/v1/documents/{id}/versions/{version}
POST   /api/documents
PATCH  /api/documents/{id}
POST   /api/documents/{id}/publish

# Personal transparency (section 3, section 10)
GET /api/usage/me
GET /api/inventory/me
```

---

## 15. Database requirements

For production, Conductor should use PostgreSQL. SQLite may remain supported for development and small
demonstrations.

The target data model should contain:

`instance`, `users`, `sub_roles`, `user_sub_roles`, `tags`, `tag_assignments`, `connection_secrets`,
`client_installations`, `client_heartbeats`, `resources`, `resource_versions`,
`resource_access_policies`, `resource_sync_state`, `documents`, `telemetry_events`, `usage_aggregates`,
`audit_events`.

Database changes should use versioned migrations rather than only runtime `CREATE TABLE` and best-effort
`ALTER TABLE` statements.

### Implementation status

Nine of the seventeen target tables exist: `instance`, `users`, `sub_roles`, `user_sub_roles`, `tags`,
`tag_assignments`, `connection_secrets`, `resources`, `telemetry_events`. A tenth, `member_inventory`,
exists and is superseded by `client_installations`. A legacy `user_tags` table is migrated into
`tag_assignments` at startup.

The migration mechanism is exactly what this section warns against: an array of
`CREATE TABLE IF NOT EXISTS` statements followed by an array of `ALTER TABLE` statements whose errors are
**discarded with `let _ = ...`** ([migrate.rs:166](../crates/conductor-storage/src/migrate.rs)). There is
no `schema_version` table, so the system cannot report which migrations have been applied, and a failed
migration is indistinguishable from a successful one.

This must be replaced before the seven new tables are added, not after.

### Addition — protect configuration secrets

The `sso_config` table stores the OIDC client secret in a column named `client_secret_enc`. The name
implies encryption. The value is written straight through with no transformation
([setup.rs:71-85](../crates/conductor-server/src/http/routes/setup.rs)), and searching `crates/` for
`encrypt`, `aes` or `cipher` returns zero matches. The secret is stored in plaintext under a name that
states otherwise.

Passwords are correctly hashed with Argon2 and connection tokens with SHA-256; this one path is the
exception. Because the OIDC secret must be recoverable for token exchange
(`sso_runtime()`), it requires symmetric encryption rather than hashing. If encryption is deferred, the
column must at minimum be renamed so it stops misleading future readers.

### Addition — model pricing table

`model_pricing`, versioned by effective date, is required by section 11. It is absent from the target
list.

See [REQ-001](requirement/REQ-001-versioned-migrations.md) and
[REQ-002](requirement/REQ-002-configuration-secret-protection.md).

---

## 16. V1 acceptance criteria

The first complete integration shall be considered successful when:

1. Admin can create or approve a member.
2. The member can sign in and create an `evc_` token.
3. The member can connect EvoFlux to the project.
4. EvoFlux displays the connected project identity.
5. EvoFlux downloads only resources assigned to that member.
6. EvoFlux periodically sends a heartbeat and inventory.
7. A completed EvoFlux session sends privacy-safe usage telemetry.
8. Admin can see the member online and view usage totals.
9. Revoking the token immediately blocks further synchronization.
10. Disabling the member invalidates browser and EvoFlux access.
11. No prompt, source code, tool argument, or credential is uploaded by default.
12. All administrative changes appear in the audit log.

The current codebase has largely implemented the member, role, tag, project-settings, and
connection-token foundations. Resource publishing, EvoFlux client integration, inventory
synchronization, telemetry ingestion, document management, and detailed dashboards remain the main
implementation work.

### Addition — foundation work that must precede the acceptance run

Four items are not visible in the acceptance list but block it. Criterion 12 cannot pass without an audit
table; criteria 5 and 10 cannot pass without API-enforced authorization; and none of the seven new tables
can be added safely on the current migration mechanism.

| Prerequisite | Requirement | Reason |
|---|---|---|
| Versioned migrations | [REQ-001](requirement/REQ-001-versioned-migrations.md) | Seven new tables are required by section 15 |
| Configuration secret protection | [REQ-002](requirement/REQ-002-configuration-secret-protection.md) | Plaintext OIDC secret under a misleading column name |
| API-enforced authorization | [REQ-004](requirement/REQ-004-api-authorization.md) | Criteria 5 and 10; section 7 states this explicitly |
| Audit logging | [REQ-018](requirement/REQ-018-audit-logging.md) | Criterion 12 |

### Addition — no automated test currently protects any of these criteria

The Rust workspace contains zero tests (`#[test]` and `#[tokio::test]` both return no matches across
`crates/`), and `apps/web/package.json` declares no test tooling and no lint script. Twelve acceptance
criteria that are verified only by hand will not stay verified. See
[REQ-020](requirement/REQ-020-automated-testing-ci.md).

---

## Requirement index

| Section | Requirement documents |
|---|---|
| 1, 15 | REQ-001, REQ-002, REQ-003 |
| 3, 7 | REQ-004, REQ-008 |
| 4, 5 | REQ-005, REQ-006 |
| 6, 12 | REQ-007, REQ-009, REQ-010 |
| 5, 8, 12 | REQ-011, REQ-012, REQ-013 |
| 9, 10, 11 | REQ-014, REQ-015, REQ-016, REQ-017, REQ-019 |
| 13 | REQ-018 |
| Cross-cutting additions | REQ-020, REQ-021, REQ-022, REQ-023 |

Full list with status: [README.md](README.md).
