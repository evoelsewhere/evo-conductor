# Evo Conductor — Project Workspace Requirements

| | |
|---|---|
| Version | 2.6 |
| Created | 2026-08-09 |
| Updated | 2026-08-11 |
| Status | Draft, pending acceptance |
| Baseline | Product specification supplied by the project owner, 2026-08-09 |
| Baseline code reviewed | `evo-conductor` at `18d9fe1`, `evoflux` at branch `thangtq42` |
| Plugin/sync extension reviewed | `evo-conductor` at `f2f7320`, `evoflux` at `919d8ede`, 2026-08-11 |
| Authoring/ZIP/editor review | `evoflux` at `a671d344`: Agent, Skill, Plugin validator/installer/workspace and Monaco editor sources, 2026-08-11 |

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
- Distribute approved Agents, standalone Skills, Portable Agent Plugins, workflows,
  commands, and documents.
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
  [state.rs:35](../crates/conductor-server/src/core/state.rs#L35)); there is one signing key per process.
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
- Treat `(project_id, resource_id)` as managed resource identity across Conductor manifests, EvoFlux
  ownership state, local managed namespaces and inventory; never infer project ownership from slug.
- Do not build multi-project navigation or UI.

See [REQ-003](requirement/06-REQ-003-server-project-separation.md).

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

The current EvoFlux feature branch implements the client side under `app/conductor` plus settings and
Intelligence UI: registration/heartbeat, OS credential storage, project-scoped cursor reconciliation,
Agent/Skill activation, Plugin trust/update review, inventory and privacy-safe telemetry. Conductor's
newer smart-fetch checkout is not yet consumed by EvoFlux, and packaged two-repository E2E remains open.

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

Tags shall support grouping entities such as members, Agents, Skills, Plugins,
documents, teams and environments.

Sub-roles and tags shall become usable in resource access policies, not remain display-only metadata.

### Implementation status

The role model is implemented and the capability predicates already exist
([role.rs:33-56](../crates/conductor-domain/src/role.rs)). Sub-roles and tags are implemented, and tag
assignment is already generic over entity type: `entity_type` is a free-form validated string
([access.rs:27-39](../crates/conductor-server/src/http/routes/access.rs)), so tagging a resource works
today without schema changes.

The current authorization picture is mixed:

- `GET /api/dashboard` still requires only an authenticated session and does not call
  `can_view_telemetry()` ([dashboard.rs](../crates/conductor-server/src/http/routes/dashboard.rs)).
- Resource listing, effective-version delivery, installations and analytics now apply actor/capability or
  ownership checks. Sub-roles and tags participate in the allow-only resource audience resolver.
- Focused negative tests exist beside high-risk features, but no generated inventory proves every mounted
  session route against Admin, Contributor and User.

See [REQ-004](requirement/02-REQ-004-api-authorization.md) and
[REQ-008](requirement/10-REQ-008-resource-access-policy.md).

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

See [REQ-005](requirement/07-REQ-005-member-lifecycle.md).

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

See [REQ-006](requirement/08-REQ-006-connection-tokens.md) and
[REQ-011](requirement/12-REQ-011-client-registration.md).

---

## 6. Resource management

Conductor shall support these resource types:

- Agent
- Skill
- Portable Agent Plugin package (`plugin.json`, optional `skills/*` and `mcp.json`)
- Workflow
- Command
- Document or project policy
- Optional reusable prompt/template

Each resource shall contain an immutable server-issued project ID and resource ID plus a project-scoped
slug, name and description, resource type, owner, visibility, lifecycle status (`draft`, `published`,
`deprecated`, `archived`), created and updated timestamps, access policy and tags. Each resource shall
also have one mutable source Draft. Each
immutable version shall contain its own server-issued version ID, semantic version, payload or artifact
reference, content checksum, publisher and change notes. Separate release-channel bindings select one
active `beta` version and one active `published` version; a released version with no active binding is
`deprecated` for distribution purposes.

Admin and Contributor users shall be able to create a draft, validate its payload, update or deprecate a
resource, view version history, roll back to a previous version, and see which EvoFlux installations have
synchronized it. Contributors may publish non-executable resources they own; only Admin may publish
executable Plugin versions under [REQ-010](requirement/19-REQ-010-plugin-distribution-safety.md).

EvoFlux shall receive only the server-resolved released version that matches the current user's access
policy: active Beta for an explicitly selected eligible member, otherwise active Published. Beta may
only narrow normal access and never grant it.

A Portable Agent Plugin shall be the first-class `plugin` resource. Its
immutable version shall reference a `.evoplugin`/ZIP artifact and record package name/version, Agent
Plugins schema version, artifact size, SHA-256 digest and minimum compatible EvoFlux version. Conductor
shall never store plugin credential values or mutable installation data in the published artifact.

### Addition — EvoFlux-compatible authoring, ZIP import and Beta release

Conductor shall provide a versioned, kind-specific guide and downloadable starter template for Agent,
standalone Skill and Portable Agent Plugin resources. The guide shall follow the formats parsed by
EvoFlux rather than inventing a Conductor-only shape:

- Agent: one Markdown file with YAML frontmatter and system-prompt body. The supported fields mirror
  EvoFlux `AgentConfig`; the frontmatter name matches the resource slug.
- Skill: a bundle rooted at `SKILL.md` with portable `name` and `description` frontmatter, non-empty
  instructions and optional `agents/`, `references/`, `scripts/`, `assets/` and `evals/` files.
- Plugin: an Agent Plugins 1.0 directory rooted at `plugin.json`, optional immediate-child
  `skills/<name>/SKILL.md` and optional root `mcp.json`; `.evoplugin` is a deterministic ZIP wrapper.

An author shall be able to start from a template, upload a direct source supported for the kind, or
upload ZIP; plugin also accepts `.evoplugin`. Conductor shall quarantine and safely inspect the archive,
reject traversal/absolute paths, duplicate or case-fold collisions, symlinks, unsupported entries and
archive bombs, normalize at most one wrapper directory, then extract regular files into a server-owned
Draft workspace. It shall never extract into its source tree, accept an absolute editor root or execute
package code during inspection.

A safely extracted package that does not match the guide shall open in the editor with structured
errors/warnings containing file, line/field where possible, fix guidance and guide links. Archive-safety
errors reject the import entirely. Content errors allow Save but block Beta and Publish. Probable embedded
credential values also block release and are masked in diagnostics and logs.

Resource Studio shall follow EvoFlux's code-authoring behavior: Monaco, responsive file tree, syntax by
file type, dirty/saved/error state, `Ctrl/Cmd+S`, create/rename/delete, unsaved-navigation protection,
validation and jump-to-diagnostic. UTF-8 text is editable; binary assets remain visible but are not
decoded. All paths and file/total sizes are bounded and enforced by the Rust API.

Save only updates the mutable Draft. Beta and Publish build deterministic bytes from the last fully saved
valid Draft, compute SHA-256, create immutable versions and bind them to a release channel. Editing a
released version creates a new Draft. V1 supports at most one active Beta and one active Published
version per resource. Beta targets a
non-empty explicit set of active, policy-eligible member IDs. Selected members receive Beta; other
eligible members receive Published. Removing a selection falls back to Published or produces a tombstone
when no Published version exists. Promotion points Published at the same immutable version ID, retires
the Beta binding, preserves bytes/digest and is audited.

### Addition — server-owned automatic version increments

Creating a new immutable Beta or direct Published version from a saved Draft shall allocate its semantic
version on the server. The default is `auto`: first release `0.1.0`, then the next greater patch version
from the highest SemVer precedence already allocated for that resource. Save, validation, Beta audience
changes, deprecation and archive do not increment. Beta-to-Published promotion preserves the same version
ID/version/bytes; publishing older content as rollback creates a new greater version.

Resource Studio shows the highest and next version and offers `Auto` or `Manual`. Manual input must be
strict SemVer 2.0, unique and greater in precedence than every allocated version; invalid prefixes,
whitespace, missing components, leading zeroes, malformed identifiers and equal/lower versions are
rejected with a field error. The server recalculates and allocates transactionally so failure consumes no
number and concurrent/stale releases cannot duplicate or skip silently.

For Plugin releases, `plugin.json.version`, the immutable resource version and the packaged artifact
must match. Auto mode previews and atomically applies the manifest change before final validation and
digesting; manual mode requires an exact match. A failed release leaves both Draft and version history
unchanged.

Guide templates and validation shall share cross-repository fixtures: every documented valid starter
passes both Conductor and the corresponding EvoFlux parser/validator.

### Addition — stable version discovery, diff and pull semantics

Conductor shall expose authorized desired-state changes using an opaque cursor. The response envelope
shall carry `schema_version`, authenticated `project_id`, `next_cursor`, `has_more` and ordered `changes`;
the client follows pages until `has_more` is false. Every change shall repeat the matching project ID and
carry stable server-issued resource ID, immutable version ID, resource kind and slug, semantic version,
server-resolved release channel (`beta` or `published`),
applicable content or artifact SHA-256 and size, minimum
compatible EvoFlux version, trust requirement and tombstone state. Slug and semantic-version ordering
are not reconciliation identity: Conductor may intentionally roll back by publishing prior content as a
new immutable version.

EvoFlux shall persist managed state by `(project_id, resource_id)`, including desired/applied version IDs, semantic
version, release channel, last applied digest, ownership marker, managed local target, plugin installation
ID when applicable, reconciliation state and committed cursor. It shall skip identical content, update
metadata without rewriting when only version identity or channel changed, and download a complete changed payload/artifact
into staging before digest verification, validation and atomic replacement. V1 does not use binary delta
patches.

Before replacement, EvoFlux shall compare the actual local digest with its last applied digest. A
mismatch, or a same-kind/same-slug local object without matching ownership state, is an
`ownership_conflict`; the user-owned object is neither adopted nor overwritten. Text resources shall
offer canonical content/file diff. Plugin review shall show manifest, file inventory, contributed Skills,
declared tool servers and executable trust-surface changes. Credentials, environment values and raw package
bytes remain local and are not included in reports.

The client shall commit the next cursor only after every returned change has been recorded durably as
applied, pending trust/update, removed, conflict, declined or incompatible. Interrupted work is replayed
idempotently. Archive, unassignment and loss of access are represented by tombstones that can remove or
disable only the matching Conductor-owned object.

### Addition — project-aware EvoFlux ownership and isolation

Every Conductor-managed Agent, standalone Skill and Plugin in EvoFlux shall retain its delivering
`project_id` in local ownership metadata, managed state, local namespace, Plugin installation mapping and
inventory. The managed logical root is keyed by the immutable project ID, not project slug or resource
name. The UI shows the connected project name and exposes the stable project ID in resource details or
diagnostics.

V1 still permits one active Conductor project per EvoFlux installation. If a member replaces the token
with one for another project, EvoFlux shall register the new project first, disable or unmount the old
project's managed resources, and reconcile into a separate project namespace. Cached old-project content
may remain for rollback but cannot be discovered, activated, updated, removed or reported under the new
project. A manifest, artifact or tombstone whose project ID does not match registration is rejected
without advancing the cursor. Cross-repository tests shall use identical Agent, Skill and Plugin slugs in
two projects and prove complete pull, runtime, inventory and removal isolation.

### Implementation status

Conductor now implements `plugin` as a governed product kind, safe Agent/Skill/Plugin import, object-backed
Drafts, Resource Studio, strict server-owned SemVer release allocation, immutable versions/channels,
allow-only audience resolution, Local/S3/Azure/Git artifacts, authorized download, cursor changes and
smart fetch. EvoFlux implements project-scoped managed state, digest/ownership checks, Agent/Skill atomic
activation and Plugin trust/update staging through its existing Plugin platform.

Remaining lifecycle gaps are general audit coverage, embedded credential-value scanning, Admin-only
Plugin publication, policy-aware Beta-target validation, streaming large artifact responses,
PostgreSQL concurrency proof, the EvoFlux smart-fetch generation client and one packaged cross-repository
security/convergence E2E.

### Addition — validate payload size at publish time

EvoFlux truncates each injected `AGENTS.md`/instruction file at 128 KiB
([workspace_instructions.py:25](../../evoflux/app/agent/hooks/workspace_instructions.py)); its Skill and
Plugin validators apply their own file and package limits. Truncation happens on the member's machine. If
Conductor does not reject kind-specific oversized payloads at release time, an administrator will never
learn that part of a released resource is being discarded or that EvoFlux will reject its artifact.

### Addition — Plugins require a stricter path

A Portable Agent Plugin may include technical `mcp.json` declarations, scripts and remote hosts.
Distributing it to member machines can become remote code execution by configuration. A bad prompt
degrades answers; a bad executable Plugin starts an unknown process on every machine in the project.
Plugins must not share the trust level of text resources. Delivery may stage a package, but activation
requires EvoFlux's local static trust review and explicit member confirmation. Credentials stay local.

See [REQ-007](requirement/09-REQ-007-resource-lifecycle.md) and
[REQ-010](requirement/19-REQ-010-plugin-distribution-safety.md).

---

## 7. Resource access policy

A resource may be targeted using primary roles, sub-roles, tags, explicit member IDs, or all project
members.

Example:

```
Resource: production-database-plugin
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

See [REQ-008](requirement/10-REQ-008-resource-access-policy.md).

---

## 8. Inventory synchronization

EvoFlux shall periodically report its local project inventory: EvoFlux version, operating system,
installation or device identifier, last connection time, active local workspace identifier, installed or
synchronized Agents, standalone Skills, Plugin resource/version/digest and non-sensitive trust state,
workflow versions, and resource sync errors. The report and every managed resource row retain the
authenticated project ID; idempotency and desired-versus-observed joins use project-scoped keys.

The inventory endpoint shall support idempotent updates so repeated heartbeats do not create duplicate
records.

Admin and Contributor users shall be able to identify online and offline installations, outdated EvoFlux
versions, missing required resources, resource-version drift, and failed synchronization.

### Implementation status

`client_installations` and project-scoped `installation_resource_inventory` now represent multiple
installations per member. Registration/heartbeat maintain last-seen state; EvoFlux submits idempotent
desired-versus-observed inventory and Conductor exposes authorized inventory queries. Core state includes
desired/applied version, channel, digest, trust/sync state, client version and sanitized errors.

Fleet-wide filters, outdated/missing compliance summaries, complete member UI coverage and a packaged
multi-installation E2E remain incomplete. Online counts must continue to use real heartbeat timestamps,
not a client-claimed boolean.

See [REQ-013](requirement/14-REQ-013-inventory-synchronization.md).

---

## 9. Usage and telemetry

EvoFlux shall send batched telemetry containing: user ID derived from the connection token, installation
ID, session/request/run/event correlation, local project or workspace ID, model provider and requested or
response model identifier, input/output/cache/reasoning/tool-use token counts, estimated or reported cost
with provenance, tool name and category, Agent/Skill/Plugin project/resource/version/relation identity,
Plugin installation and contributed Skill/tool identity, request/model/tool status and duration, active
agents, session start/end, sanitized error category, and EvoFlux version.

Telemetry events shall support client-generated event IDs, idempotent ingestion, batch submission, retry
after temporary network failure, configurable retention, and server-side aggregation.

### Addition — auditable member/resource usage without work content

One user request can activate an Agent and several Skills, execute Plugin-contributed tools, and make
several model calls or retries. The wire and storage model shall preserve separate request, run, model
call, tool call and resource-attribution grains. Requests count distinct terminal `request_id`; model
calls and retries count their own events; token and estimated-cost totals sum model-call facts once;
request success/error comes from the terminal request; tool success/error comes from each tool call.

Every governed resource reference uses `(project_id, resource_id, version_id)` plus kind and attribution
relation. Member/project identity and the primary-role/sub-role/tag snapshot are derived from authenticated
server state, not client claims. An identical local or cross-project name is not attributed to a governed
resource. Project totals count each request/model/tool fact once; per-resource attribution may overlap and
must say so rather than adding Agent, Skill and Plugin totals together.

This metadata shall answer who used which resource/version, when it was reported and received, from which
installation, with which model/tool calls, tokens, estimate, duration and outcome. It shall not contain
prompts, responses, reasoning text, tool arguments/results, file contents/paths or credentials.

### Implementation status

The current pipeline uses typed request/resource/model/tool grains with idempotent batch ingestion,
token-derived member/project identity, installation ownership, server-validated managed-resource/version
attribution, role snapshots, categorized tokens, provider/model/tool, outcomes, duration, cost provenance
and both reported/received timestamps. Tables have member/project/time/resource indexes; EvoFlux uses a
durable bounded outbox and a privacy-safe allowlist.

Remaining gaps include a single canonical time policy for personal versus portfolio queries, exposing
dropped-event counts, treating permanent 4xx failures as terminal, completing run/parent/session/cost
dimensions, differentiating L1/L2 fields on the client and a live cross-repository replay/load proof.

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

See [REQ-014](requirement/15-REQ-014-telemetry-ingestion.md) and
[REQ-016](requirement/17-REQ-016-usage-aggregation-dashboards.md).

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

The current telemetry domain has expanded beyond the original counter-only `TelemetrySnapshot` into
typed request/resource/model/tool events, but it preserves the same boundary: no prompt, response,
reasoning, source, tool argument/result, credential or absolute path appears in the wire contract
([telemetry.rs](../crates/conductor-domain/src/telemetry.rs)).

### Addition — members must be able to see their own record

A monitoring system that members cannot inspect will be worked around rather than trusted, and the
resulting data becomes worthless. The cheapest structural defense is symmetry: anything an administrator
can see about a member, that member shall be able to see about themselves. Enforcing this as an automated
test turns the privacy policy from a written promise into a property of the system.

### Addition — the tension between "what was it used for" and "no content"

Current source defines three privacy-safe collection levels. They change whether telemetry is disabled or
how much operational attribution is included; none permits prompt, response, reasoning, tool arguments,
headers, environment values or source content:

| Level | Collected | Answers "used for what" | Cost |
|---|---|---|---|
| L0 | No usage events | No usage answer | Maximum privacy; no operational analytics |
| L1 | Counters and bounded operational metadata: event grain, outcome, provider/model, token categories, duration and cost provenance | Operational usage and reliability | Default privacy-safe analytics surface |
| L2 | L1 plus extended server-validated resource/version/relation and organizational attribution | Managed-resource effectiveness | More metadata, still no work content |

The rollout default remains an owner decision. Adding any future content-capture mode requires a separate
requirement, explicit member notice/consent, narrowly scoped readers and retention; it is not L2.

See [REQ-015](requirement/11-REQ-015-privacy-controls.md) and
[REQ-019](requirement/21-REQ-019-data-retention.md).

---

## 11. Dashboard requirements

### Project overview

The dashboard shall display total, active, pending, invited, and disabled members; currently connected
EvoFlux installations; active connection tokens; resource counts by type; resource synchronization
health; SSO status; and recent administrative activity.

### Usage dashboard

Authorized users shall be able to filter by today/current week/current month/last 7/30/90 days or custom
`from`/`to`; member; recorded primary role, sub-role or tag; Agent, Skill or Plugin resource and immutable
version; provider/model; event type, request/tool outcome and sanitized error; tool; EvoFlux installation
and client version. Filter state is encoded in the URL and shared by every KPI, chart, ranking and table.

Metrics shall include distinct requests, attributed resource uses, model calls, tool calls, request and
tool success/error/cancelled rates, separated input/output/cache/reasoning/tool-use tokens, estimated cost
and unpriced calls, total and per-request averages, average and p95 duration, active agents, trend over
time, and highest-usage members and resources. Metric denominators and cost provenance shall be visible.

Reusable accessible charts shall cover stacked request outcome trend, stacked token trend, estimated-cost
trend, provider/model distribution, Agent/Skill/Plugin attributed share and ranking, resource/model
success/error and duration, and top members/resources with role breakdown. Chart selection filters a
server-paginated activity table.

Activity rows show reported/received time, member and recorded role, resource kind/name/version/relation,
request outcome, model calls, provider/model, token categories, estimated cost/source, duration and safe
error category. Request detail correlates Agent runs, standalone or Plugin-provided Skills,
Plugin-contributed tools, model retries and per-call tokens/cost/duration without work content. The member
detail integrates installations, tokens, resource usage, charts and activity; resource detail shows
members, roles, versions, adoption, usage and failures.

A regular User should only see personal usage unless granted broader permission.

### Implementation status

The console now includes personal/member/resource analytics plus portfolio Analytics Studio with typed
widgets/filters and optimistic-revision saved views. It exposes request/resource/model/tool grains,
managed version attribution, token categories, estimated cost source and unpriced calls. Queries still
read raw events rather than a `usage_aggregates` table/job, some filter/view combinations remain partial,
and current-scale performance/retention/export proof is still open.

### Addition — cost requires a priced model table

The baseline lists estimated cost as a metric but does not state where the price comes from. Token counts
are a technical number; management asks about money. A `model_pricing` table is required, and it must be
versioned by effective date so that historical periods are costed at the price that applied then rather
than being silently restated when a provider changes rates. Models with no price must display as
"unpriced" rather than as zero.

### Addition — distinguish "no data" from "zero"

An empty monitoring screen must state why it is empty: nobody has connected yet, versus no activity in
the selected range. Current analytics surfaces have explicit empty/loading/error states; their remaining
UI tests must preserve that distinction.

See [REQ-016](requirement/17-REQ-016-usage-aggregation-dashboards.md) and
[REQ-017](requirement/20-REQ-017-cost-estimation.md).

---

## 12. Document management

Conductor shall support project documents such as coding standards, architecture guidelines, security
policies, migration rulebooks, team onboarding instructions, and Plugin usage policies.

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

See [REQ-009](requirement/18-REQ-009-document-management.md) and
[REQ-012](requirement/13-REQ-012-resource-sync-client.md).

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
- Viewing another member's attributed resource/request usage detail. Reading it is itself an action that
  must be attributable without copying the viewed usage values into the administrative audit log.

See [REQ-018](requirement/05-REQ-018-audit-logging.md).

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
GET  /api/v1/resources/{id}/versions/{version_id}
GET  /api/v1/resources/{id}/versions/{version_id}/artifact

# Resource administration
POST   /api/resources
PATCH  /api/resources/{id}
GET    /api/resources/guides/{kind}
GET    /api/resources/templates/{kind}
POST   /api/resources/{id}/draft/import
GET    /api/resources/{id}/draft/tree
GET    /api/resources/{id}/draft/files/{path}
PUT    /api/resources/{id}/draft/files/{path}
POST   /api/resources/{id}/draft/entries
DELETE /api/resources/{id}/draft/entries/{path}
POST   /api/resources/{id}/draft/validate
POST   /api/resources/{id}/draft/diagnostics/{code}/acknowledge
POST   /api/resources/{id}/versions
POST   /api/resources/{id}/versions/{version_id}/artifact
POST   /api/resources/{id}/beta
PUT    /api/resources/{id}/beta/members
POST   /api/resources/{id}/publish
POST   /api/resources/{id}/deprecate
GET    /api/resources/{id}/versions

# Monitoring
GET /api/usage/summary
GET /api/usage/members
GET /api/usage/resources
GET /api/usage/activity
GET /api/usage/requests/{request_id}
GET /api/usage/timeseries
GET /api/usage/models
GET /api/usage/tools
GET /api/usage/plugins
GET /api/inventory
GET /api/audit-events
```

Beta and Publish requests that create a version carry `version_mode: auto|manual` and an optional manual
version. Their response returns the allocated semantic version, immutable version ID and refreshed next
version. Promotion identifies an existing Beta version ID and does not accept or allocate another
version.

### Implementation status

The current Conductor router exposes registration/heartbeat, governed import/Draft/release/access/version
lifecycle, HMAC cursor changes, smart fetch, authorized immutable artifacts, inventory, telemetry,
personal/member/resource/portfolio analytics and saved views. EvoFlux currently consumes the cursor
contract; migration to atomic smart-fetch generation checkout remains. Document management and a general
audit API are still absent.

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
`resource_access_policies`, `resource_sync_state`, `documents`, `telemetry_events`,
`telemetry_event_resources`, normalized `usage_aggregates`, `model_pricing`, `audit_events`.

Database changes should use versioned migrations rather than only runtime `CREATE TABLE` and best-effort
`ALTER TABLE` statements.

### Implementation status

The current schema now includes installation registration/last-seen, project-scoped governed resources,
Draft/version/channel/Beta/policy/change/inventory records, typed telemetry attribution and analytics
saved views. Heartbeat is represented on `client_installations` rather than a separate history table.
Document storage, normalized usage aggregates, server model pricing and a general audit log remain
missing. A legacy `user_tags` table is migrated into `tag_assignments` at startup.

The migration mechanism is exactly what this section warns against: an array of
`CREATE TABLE IF NOT EXISTS` statements followed by an array of `ALTER TABLE` statements whose errors are
**discarded with `let _ = ...`** ([migrate.rs:166](../crates/conductor-storage/src/migrate.rs)). There is
no `schema_version` table, so the system cannot report which migrations have been applied, and a failed
migration is indistinguishable from a successful one.

This must be replaced before further production schema evolution. Governed-delivery tables have already
landed on the best-effort mechanism, increasing the need for an explicit baseline/version ledger and
tested upgrade path rather than making the requirement obsolete.

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

`model_pricing`, versioned by effective date, is required by section 11. It is present in the target
model above but absent from the current schema.

See [REQ-001](requirement/03-REQ-001-versioned-migrations.md) and
[REQ-002](requirement/04-REQ-002-configuration-secret-protection.md).

---

## 16. V1 acceptance criteria

Revised by the project owner on 2026-08-09, expanding the original twelve criteria to sixteen. The
owner's revision numbers this section 17; it is kept as section 16 here to preserve the numbering of the
sections above, which have not changed.

The V1 integration shall be considered complete when:

| # | Criterion | Covered by |
|---|---|---|
| 1 | Admin can create or approve a member | [REQ-005](requirement/07-REQ-005-member-lifecycle.md) |
| 2 | The member can sign in and create an `evc_` token | [REQ-006](requirement/08-REQ-006-connection-tokens.md) AC-1, AC-3 |
| 3 | The member can connect EvoFlux to the project | [REQ-011](requirement/12-REQ-011-client-registration.md) AC-1, AC-2 |
| 4 | EvoFlux displays the correct project name and logo | [REQ-011](requirement/12-REQ-011-client-registration.md) AC-6 |
| 5 | EvoFlux retrieves the member's correct role and policy | [REQ-011](requirement/12-REQ-011-client-registration.md) AC-4, AC-5 |
| 6 | EvoFlux downloads only the resources the member may use | [REQ-008](requirement/10-REQ-008-resource-access-policy.md), [REQ-012](requirement/13-REQ-012-resource-sync-client.md) AC-7 |
| 7 | EvoFlux sends a heartbeat on a regular interval | [REQ-011](requirement/12-REQ-011-client-registration.md) AC-8, AC-12 |
| 8 | EvoFlux sends inventory without creating duplicate records | [REQ-013](requirement/14-REQ-013-inventory-synchronization.md) AC-2 |
| 9 | After a session, EvoFlux sends privacy-safe usage telemetry | [REQ-014](requirement/15-REQ-014-telemetry-ingestion.md), [REQ-015](requirement/11-REQ-015-privacy-controls.md) AC-3 |
| 10 | Admin sees the member online on the dashboard | [REQ-013](requirement/14-REQ-013-inventory-synchronization.md) AC-4, [REQ-016](requirement/17-REQ-016-usage-aggregation-dashboards.md) AC-4 |
| 11 | Admin can audit which Agent/Skill/Plugin versions a member used, when, their recorded role, request/model/tool outcomes, token usage, model calls and estimated cost, alongside that member's connection tokens | [REQ-006](requirement/08-REQ-006-connection-tokens.md) AC-8, [REQ-014](requirement/15-REQ-014-telemetry-ingestion.md) AC-6, AC-11–AC-16, [REQ-016](requirement/17-REQ-016-usage-aggregation-dashboards.md) AC-5, AC-6, AC-12–AC-25 |
| 12 | Revoking a token removes EvoFlux access immediately | [REQ-006](requirement/08-REQ-006-connection-tokens.md) AC-6, [REQ-012](requirement/13-REQ-012-resource-sync-client.md) AC-11 |
| 13 | Disabling a member blocks both browser session and EvoFlux connection | [REQ-005](requirement/07-REQ-005-member-lifecycle.md) AC-1, AC-2, AC-3 |
| 14 | A newly released agent, standalone Skill or Portable Agent Plugin receives the correct automatically incremented or validated manual version and is synchronized by EvoFlux at the correct project and Published/Beta version; same-name resources from another project remain isolated, Beta is isolated to selected eligible members and executable plugin components remain disabled until locally trusted | [REQ-007](requirement/09-REQ-007-resource-lifecycle.md) AC-4, AC-12, AC-14–AC-38, [REQ-008](requirement/10-REQ-008-resource-access-policy.md) AC-11–AC-14, [REQ-012](requirement/13-REQ-012-resource-sync-client.md) AC-8, AC-9, AC-14–AC-54, [REQ-010](requirement/19-REQ-010-plugin-distribution-safety.md) AC-3, AC-13–AC-16 |
| 15 | Source code, prompts, tool arguments and credentials are not uploaded by default | [REQ-014](requirement/15-REQ-014-telemetry-ingestion.md) AC-9, [REQ-015](requirement/11-REQ-015-privacy-controls.md) AC-3, AC-10 |
| 16 | Every administrative change appears in the audit log | [REQ-018](requirement/05-REQ-018-audit-logging.md) AC-3 |

### What the revision settles

Two criteria resolve questions that were previously left open in the requirement documents:

- Criterion 11 states that **Admin** views an individual member's usage. This answers the open question
  in [REQ-004](requirement/02-REQ-004-api-authorization.md) and
  [REQ-016](requirement/17-REQ-016-usage-aggregation-dashboards.md) for Admin. Whether Contributor also has
  per-member drill-down, or only project totals, is still unstated.
- Criterion 15 establishes a minimum acceptance boundary. The reconciled REQ-015/source contract is
  stricter: prompt, response, reasoning, tool arguments, credentials and source content are excluded at
  every current L0/L1/L2 level.

### Criteria that required a sharper acceptance criterion

Every criterion above was already within the scope of an existing requirement, but three were covered
only indirectly. Acceptance criteria were added so each has something concrete to test against:

| # | Gap | Added |
|---|---|---|
| 7 | The heartbeat endpoint and its idempotency were specified, but no criterion required the client to send on an interval | [REQ-011](requirement/12-REQ-011-client-registration.md) AC-12 |
| 11 | Tokens and usage were covered separately, but no criterion required one fully attributed member/resource view with filters, charts and request drill-down | [REQ-014](requirement/15-REQ-014-telemetry-ingestion.md) AC-11–AC-16, [REQ-016](requirement/17-REQ-016-usage-aggregation-dashboards.md) AC-12–AC-25 |
| 14 | Checksums and cursor-based change retrieval were specified, but no criterion required transactional version allocation, project-scoped stable-ID reconciliation, safe cursor commit, ownership-aware diff, project/Beta isolation, mode/additive-overlay behavior, explicit update review or smart-fetch generation convergence | [REQ-007](requirement/09-REQ-007-resource-lifecycle.md) AC-33–AC-38, [REQ-012](requirement/13-REQ-012-resource-sync-client.md) AC-14, AC-23–AC-54 |

The current codebase has implemented most of the member/token, governed catalog, registration, cursor
reconciliation, Plugin trust, inventory, telemetry and analytics product path with stable project/resource
identity. The main remaining work is foundation hardening (versioned migrations, OIDC secret encryption,
dashboard authorization and general audit), EvoFlux smart-fetch checkout, aggregate/retention/cost policy,
document/model policy/i18n features and complete PostgreSQL/frontend/packaged cross-repository proof.

### Addition — foundation work that must precede the acceptance run

Four items are not visible in the acceptance list but block it. Criterion 16 cannot pass without an audit
table; criteria 6 and 13 cannot pass without API-enforced authorization; and none of the seven new tables
can be added safely on the current migration mechanism.

| Prerequisite | Requirement | Reason |
|---|---|---|
| Versioned migrations | [REQ-001](requirement/03-REQ-001-versioned-migrations.md) | Seven new tables are required by section 15 |
| Configuration secret protection | [REQ-002](requirement/04-REQ-002-configuration-secret-protection.md) | Plaintext OIDC secret under a misleading column name |
| API-enforced authorization | [REQ-004](requirement/02-REQ-004-api-authorization.md) | Criteria 6 and 13; section 7 states this explicitly |
| Audit logging | [REQ-018](requirement/05-REQ-018-audit-logging.md) | Criterion 16 |

### Addition — automated coverage is substantial but incomplete

The Rust workspace currently passes 94 tests across domain, storage and Axum integration surfaces.
Conductor web passes typecheck/build but still has no Vitest, Playwright, ESLint or CI workflow, and the
backend suite lacks the exhaustive route-role inventory, nextest/JUnit/coverage and PostgreSQL matrix.
Cross-repository behavior has focused suites and a documented fleet simulator, but not one packaged live
two-application E2E. See
[REQ-020](requirement/01-REQ-020-automated-testing-ci.md).

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
