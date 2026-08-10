# DES-007 — Governed Agent, Skill and Plugin delivery

| | |
|---|---|
| ID | DES-007 |
| Created | 2026-08-11 |
| Updated | 2026-08-11 |
| Status | Approved (2026-08-11; project owner directed implementation) |
| Primary requirement | [REQ-007](../requirement/09-REQ-007-resource-lifecycle.md) |
| Coordinated requirements | [REQ-008](../requirement/10-REQ-008-resource-access-policy.md), [REQ-010](../requirement/19-REQ-010-plugin-distribution-safety.md), [REQ-012](../requirement/13-REQ-012-resource-sync-client.md), [REQ-013](../requirement/14-REQ-013-inventory-synchronization.md) |
| References | [architecture.md](../architecture.md), [resource-authoring-guide.md](../resource-authoring-guide.md), [BASE-CONVENTIONS](../base/BASE-CONVENTIONS.md) |
| Tasks | [TSK-007-01 through TSK-007-12](../task/09-REQ-007-governed-resource-delivery/) — Todo |

## 1. Goal

Deliver one project-scoped contract by which Conductor authors, validates, versions, targets and audits
Agent, standalone Skill and Portable Agent Plugin resources, while EvoFlux safely discovers, reviews,
pulls, activates and reports their observed state. The design must satisfy:

- immutable project/resource/version identity and transactionally allocated SemVer;
- safe ZIP-to-Draft authoring and Monaco Resource Studio editing;
- server-resolved Beta/Published delivery without access-policy bypass;
- artifact integrity, compatibility and local Plugin trust;
- ownership-aware, offline-safe and project-isolated reconciliation;
- privacy-safe desired-versus-observed inventory.

The design intentionally coordinates five requirements. Splitting storage, change-feed, trust and
inventory contracts into independent designs would allow mutually incompatible identity and state
machines.

## 2. Decisions and options considered

| Concern | Option | Advantages | Disadvantages | Outcome |
|---|---|---|---|---|
| Draft storage | JSON in database | Simple backup | Poor fit for trees/binary assets and Monaco | Rejected |
| Draft storage | Server-owned filesystem workspace | Natural file operations, bounded extraction and deterministic packaging | Requires configured data directory and cleanup | Selected for V1 |
| Artifact storage | Database BLOB | Transactional with metadata | Large packages inflate DB and backups | Rejected |
| Artifact storage | `ArtifactStore` abstraction with content-addressed filesystem backend | Local V1 works with SQLite/PostgreSQL; future S3 backend does not change API | Multi-replica production needs shared backend | Selected |
| Sync | Full snapshot by kind/slug | Already partially implemented | Cannot safely represent tombstones, Beta fallback or project switching | Rejected after compatibility period |
| Sync | Ordered cursor change feed by stable IDs | Incremental, replayable, audit-friendly | More state and tests | Selected |
| Plugin activation | Server-controlled enablement | Central convenience | Remote execution without local consent | Rejected |
| Plugin activation | Stage disabled, show static trust diff, require local approval | Preserves local security boundary | Adds pending state | Selected |
| Design structure | One design per endpoint family | Smaller documents | Duplicates wire identities and state transitions | Rejected |
| Design structure | One coordinated epic design with layer-specific tasks | One source of truth for cross-repo invariants | Larger traceability table | Selected |

## 3. System boundaries and invariants

1. Conductor owns project membership, access policy, Draft source, immutable versions, release channels,
   desired state, audit records and artifact metadata.
2. EvoFlux owns local activation, Plugin credentials, `PLUGIN_DATA`, local conflicts and runtime state.
3. Managed identity is `(project_id, resource_id)`; desired content identity is `version_id` plus SHA-256.
   Kind, slug, display name, path and SemVer are never reconciliation keys.
4. All management writes derive project context from the browser session. All client reads/writes derive
   project/member context from the connection token. A conflicting body/path identifier is rejected.
5. A release creates immutable bytes and a channel binding. Save and target edits never mutate released
   bytes or allocate a version.
6. Plugin download never means Plugin activation. Trust is local and repeatable when executable surface
   changes.

## 4. Data model and migrations

This design depends on versioned migrations and project separation from REQ-001/REQ-003. Names below are
logical; the implementation must use SQL portable across PostgreSQL and SQLite through SQLx Any.

```sql
ALTER TABLE resources ADD COLUMN project_id UUID NOT NULL;
ALTER TABLE resources ADD COLUMN draft_revision BIGINT NOT NULL DEFAULT 0;
ALTER TABLE resources ADD COLUMN draft_status TEXT NOT NULL DEFAULT 'empty';
ALTER TABLE resources ADD COLUMN highest_semver TEXT;
CREATE UNIQUE INDEX resources_project_kind_slug_uq
  ON resources(project_id, kind, slug);

ALTER TABLE resource_versions ADD COLUMN project_id UUID NOT NULL;
ALTER TABLE resource_versions ADD COLUMN content_sha256 TEXT NOT NULL;
ALTER TABLE resource_versions ADD COLUMN content_size BIGINT NOT NULL;
ALTER TABLE resource_versions ADD COLUMN artifact_key TEXT;
ALTER TABLE resource_versions ADD COLUMN artifact_schema_version TEXT;
ALTER TABLE resource_versions ADD COLUMN minimum_evoflux_version TEXT;
CREATE UNIQUE INDEX resource_versions_semver_uq
  ON resource_versions(project_id, resource_id, version);

CREATE TABLE resource_release_channels (
  project_id UUID NOT NULL,
  resource_id UUID NOT NULL,
  channel TEXT NOT NULL CHECK (channel IN ('beta', 'published')),
  version_id UUID NOT NULL,
  updated_by UUID NOT NULL,
  updated_at TIMESTAMP NOT NULL,
  PRIMARY KEY (project_id, resource_id, channel)
);

CREATE TABLE resource_beta_members (
  project_id UUID NOT NULL,
  resource_id UUID NOT NULL,
  user_id UUID NOT NULL,
  assigned_by UUID NOT NULL,
  assigned_at TIMESTAMP NOT NULL,
  PRIMARY KEY (project_id, resource_id, user_id)
);

CREATE TABLE resource_changes (
  sequence BIGINT PRIMARY KEY,
  project_id UUID NOT NULL,
  resource_id UUID NOT NULL,
  effective_user_id UUID,
  change_kind TEXT NOT NULL,
  version_id UUID,
  channel TEXT,
  created_at TIMESTAMP NOT NULL
);

CREATE TABLE installation_resource_inventory (
  project_id UUID NOT NULL,
  installation_id UUID NOT NULL,
  resource_id UUID NOT NULL,
  desired_version_id UUID,
  applied_version_id UUID,
  release_channel TEXT,
  content_sha256 TEXT,
  plugin_installation_id TEXT,
  observed_state TEXT NOT NULL,
  error_category TEXT,
  observed_at TIMESTAMP NOT NULL,
  PRIMARY KEY (project_id, installation_id, resource_id)
);
```

Additional constraints and indexes:

- every child row has a composite project/resource foreign key or an equivalent transactionally checked
  constraint supported by both dialects;
- `resource_access_rules` gains `project_id`, an `effect` enum (`allow`, `exclude`) and project-scoped
  subject validation;
- Plugin replaces the legacy technical resource kind. Existing rows are migrated only when their payload
  is a valid portable package reference; otherwise startup/migration reports a compatibility error;
- change-feed indexes cover `(project_id, effective_user_id, sequence)` and
  `(project_id, resource_id, sequence)`;
- inventory indexes cover project/member, online state, client version, observed state and error category.

Draft metadata is stored in the database, while files live below:

```text
<conductor-data>/projects/<project_id>/resources/<resource_id>/draft/
<conductor-data>/artifacts/sha256/<first-two>/<digest>
```

Paths are generated from server IDs only. No API accepts an absolute root. Artifact writes use a staging
file, fsync, digest verification and atomic rename before the database transaction publishes metadata.
Orphan staging files are swept; immutable content-addressed artifacts are garbage-collected only when no
version references them and retention permits it.

EvoFlux adds a versioned local table (or equivalent durable store) keyed by
`(project_id, resource_id)` with desired/applied version IDs, SemVer, channel, digest, ownership marker,
managed target, Plugin installation ID, state, diagnostic and committed cursor. Legacy kind/slug state is
migrated only when ownership is unambiguous; otherwise it becomes an explicit conflict.

## 5. Domain and state machines

### 5.1 Resource and release state

```text
Draft --Beta(new immutable version)--> Beta binding
Draft --Publish(new immutable version)--> Published binding
Beta --Promote(same version ID/bytes)--> Published binding
Any released version --Edit/Restore--> Draft
Resource --Archive--> tombstones for every effective recipient
```

`VersionMode::Auto` is the default. Within the release transaction the server locks or compare-and-swaps
the resource version head, parses all allocated values with strict SemVer 2.0 and chooses `0.1.0` for the
first release or the next patch candidate after the highest precedence. A highest prerelease promotes to
its matching stable version when greater; build metadata does not create precedence. Manual values must
be valid, unique and greater than every allocated version. Stale/concurrent release returns
`409 version_conflict` with refreshed `highest_version` and `next_version`. Failures create neither a
version row nor a consumed number.

For Plugin Auto releases, packaging applies the allocated value to a temporary manifest view, validates
and digests that view, then atomically persists the matching Draft update and immutable artifact. Manual
release requires the saved manifest value to match.

### 5.2 Effective audience

The server first evaluates project membership, active status, private ownership, exclusion and allow
rules. Only then does it resolve channel:

```text
not eligible                        -> no desired state / authorized tombstone
eligible + explicit valid Beta      -> active Beta version
eligible + not selected for Beta    -> active Published version
eligible + no effective version     -> no desired state
```

Beta target changes emit per-user effective changes. They never grant access. Direct metadata/artifact
reads re-run the same resolver; knowledge of a version ID is insufficient authorization.

### 5.3 EvoFlux reconciliation states

`pending`, `staged`, `trust_pending`, `update_pending`, `applied`, `in_sync`, `declined`, `incompatible`,
`ownership_conflict`, `project_scope_mismatch`, `error`, `removed`.

The client stages a complete payload/artifact, verifies size and SHA-256, validates, then atomically
switches. It compares the current local digest with the last-applied digest before replacement. A changed
user-owned or modified managed target is never overwritten or adopted. Plugin install/update reuses the
existing EvoFlux installer so installation identity, credentials and `PLUGIN_DATA` survive. Changed
trust surface requires a new review; the previous runnable version stays active until approval.

## 6. API contract

### 6.1 Management APIs

| Method | Path | Auth | Role | Purpose |
|---|---|---|---|---|
| `GET` | `/api/resources/guides/{kind}` | session | Admin/Contributor | Versioned guide and limits |
| `GET` | `/api/resources/templates/{kind}` | session | Admin/Contributor | Starter source/package |
| `POST` | `/api/resources/{id}/draft/import` | session | Admin/owner Contributor | Quarantine and safe extraction |
| `GET/PUT` | `/api/resources/{id}/draft/files/{path}` | session | Admin/owner Contributor | Read/write UTF-8 file |
| `POST/DELETE` | `/api/resources/{id}/draft/entries` | session | Admin/owner Contributor | Create/rename/delete entry |
| `POST` | `/api/resources/{id}/draft/validate` | session | Admin/owner Contributor | Structured diagnostics |
| `POST` | `/api/resources/{id}/versions` | session | Admin/owner Contributor | Release Beta or Published |
| `PUT` | `/api/resources/{id}/beta/members` | session | Admin/eligible owner | Replace explicit Beta audience |
| `POST` | `/api/resources/{id}/publish` | session | Admin/eligible owner | Direct publish or promote Beta |
| `GET` | `/api/resources/{id}/audience-preview` | session | Admin/owner Contributor | Policy/channel/effective version preview |

Release request:

```json
{
  "channel": "beta",
  "version_mode": "auto",
  "manual_version": null,
  "draft_revision": 17,
  "changelog": "Pilot improved release validation",
  "beta_member_ids": ["user-uuid"]
}
```

Release response:

```json
{
  "resource_id": "resource-uuid",
  "version_id": "version-uuid",
  "version": "0.4.3",
  "channel": "beta",
  "sha256": "64-hex",
  "size": 48122,
  "highest_version": "0.4.3",
  "next_version": "0.4.4"
}
```

### 6.2 Client APIs

| Method | Path | Scope | Purpose |
|---|---|---|---|
| `GET` | `/api/v1/resources/changes?cursor=&limit=` | `subscribe_resources` | Ordered authorized desired-state changes |
| `GET` | `/api/v1/resources/{id}/versions/{version_id}` | `subscribe_resources` | Effective version metadata/text payload |
| `GET` | `/api/v1/resources/{id}/versions/{version_id}/artifact` | `subscribe_resources` | Authorized immutable Plugin bytes |
| `PUT` | `/api/v1/client/inventory` | `sync_inventory` | Idempotent observed state |

Change page:

```json
{
  "schema_version": 2,
  "project_id": "project-uuid",
  "next_cursor": "opaque-signed-sequence",
  "has_more": false,
  "changes": [{
    "project_id": "project-uuid",
    "resource_id": "resource-uuid",
    "version_id": "version-uuid",
    "kind": "plugin",
    "slug": "release-audit",
    "version": "0.4.3",
    "release_channel": "beta",
    "sha256": "64-hex",
    "size": 48122,
    "minimum_evoflux_version": "0.9.0",
    "trust_required": true,
    "tombstone": false
  }]
}
```

The cursor is opaque and bound to project/member/schema. The client persists it only after every change
has a durable terminal/pending result. A page replay is idempotent. The compatibility snapshot remains
read-only for one release and never advertises Plugin artifacts; schema-v2 capable clients use changes.

### 6.3 Error contract

| Status/code | Meaning |
|---|---|
| `400 invalid_archive` | Malformed ZIP/container |
| `403 forbidden` | Role, ownership, scope or effective-version denial |
| `409 draft_revision_conflict` | Save/release used stale Draft revision |
| `409 version_conflict` | Stale/concurrent version allocation |
| `409 ownership_conflict` | Local object cannot be safely adopted or overwritten |
| `413 archive_limit_exceeded` | Entry/byte/compression limit exceeded |
| `422 validation_failed` | Draft errors block release |
| `422 manifest_version_mismatch` | Plugin manifest and requested/allocated version differ |
| `422 incompatible_client` | Schema/minimum client unsupported |
| `422 project_scope_mismatch` | Project ID or resource ownership disagrees with token |

Errors expose stable codes and safe diagnostics, never filesystem roots, raw SQL, credentials, headers or
package content.

## 7. Backend changes

| Layer | Main locations | Change |
|---|---|---|
| Domain | `crates/conductor-domain/src/resource.rs` | Plugin kind, strict enums, SemVer release request/result, channels, diagnostics, change and inventory types |
| Storage | `crates/conductor-storage/src/migrate.rs`, `repos/resource.rs` | Versioned schema, project-scoped transactions, audience resolver, change feed and inventory repositories |
| Server core | `crates/conductor-server/src/core/config.rs`, `state.rs` | Data-root and `ArtifactStore` dependencies |
| HTTP | `crates/conductor-server/src/http/routes/resources.rs` plus focused route modules | Thin authoring, release, changes, artifact and inventory handlers |
| Tests | crate integration tests | Migration, transaction, authorization and cursor proofs |

Package validation is a Rust/static service behind a trait with kind-specific validators. It must share
fixtures with EvoFlux; it must not shell out to package code. Route files should be split before adding
the endpoint set instead of extending the current monolithic resources route.

## 8. Conductor console changes

| Route/screen | Components | Required states |
|---|---|---|
| `/app/resources/{kind}` | catalog list and resource page | loading, empty, error, forbidden, populated |
| `/app/resources/{kind}/{id}/edit` | guide panel, file tree, Monaco editor, diagnostic drawer | importing, dirty, saving, invalid, warning, valid, conflict |
| Release dialog | Auto/Manual version, Beta member selector, manifest diff, changelog | stale, validation error, version conflict, success |
| Access/audience | allow/exclude rules and effective audience preview | policy audience, Beta audience, ineligible target, fallback |
| Versions | immutable history, channel badges, promote/deprecate/restore | no Beta, Beta only, Published, archived |
| Monitoring | adoption and desired-versus-observed inventory | pending trust, drift, incompatible, conflict, error |

Use existing shared UI primitives and EvoFlux visual language. Monaco is lazy-loaded only on the editor
route. All product constants/enums live in domain-specific shared constant modules; no hard-coded kind,
channel, state or error literals in components.

## 9. EvoFlux changes

| Area | Main locations | Change |
|---|---|---|
| Contract | `app/conductor/models.py`, `client.py`, constants | Schema-v2 change pages, artifact stream, inventory and typed errors |
| Durable state | versioned local migration/model | Project-scoped managed state and committed cursor |
| Reconcile | `app/conductor/reconciler.py`, `service.py` | ID-based staging, diff, atomic switch, tombstones and project switching |
| Plugin | `app/plugin_platform/validator.py`, `installer.py`, `trust.py` | Reuse validation/install/update and add Conductor ownership mapping |
| Runtime | existing Agent/Skill/Plugin loaders | Discover only active managed namespace; do not add parallel runtime |
| UI | Conductor settings/status and resource detail | project identity, pending review, diff, conflict and retry actions |

Replacing the active project first registers the new token, then disables/unmounts the old project's
managed namespace, and only then activates the new namespace. Cached bytes may remain isolated for
rollback. Conductor unavailability never blocks EvoFlux startup or previously trusted content.

## 10. Security and privacy

- Every management endpoint tests Admin, owner Contributor, non-owner Contributor and User. Client
  endpoints test malformed/wrong-scope/expired/revoked token, disabled owner and cross-project IDs.
- ZIP inspection rejects traversal, absolute/backslash ambiguity, duplicates, case-fold collisions,
  symlinks, devices, too many entries, excessive sizes and suspicious compression ratio before extraction.
- Draft and artifact locations are never returned as absolute paths.
- Plugin static review lists executable commands, remote hosts, environment field names and capabilities;
  values are never sent to Conductor.
- Inventory is built from a typed allowlist and contains no package bytes, Skill content, command args,
  environment/header values, credentials, `PLUGIN_DATA` or absolute paths.
- Artifact responses use `nosniff`, fixed content disposition, digest/length and authorization on every
  request. No public artifact URL bypasses effective-version policy.
- Audit covers import, Save, warning acknowledgement, release, target change, promotion, deprecation,
  archive, access change, cross-member preview and denied administrative action.

## 11. Performance and operational limits

- Default change page 100, maximum 500; indexed sequence query target p95 below 200 ms for 100k changes.
- Draft editor target: 2,000 entries, 1 MiB editable file, kind-specific package limits no greater than
  EvoFlux, tree response paginated/lazy when necessary.
- Artifact streams; it is never buffered wholly in Axum or Python. Digest verification is incremental.
- Version allocation locks/compares only one resource row and release-channel rows.
- Inventory upsert is one transaction per installation, maximum 2,000 observed resources, with lightweight
  heartbeat separate from inventory.
- Filesystem `ArtifactStore` is supported only for one server process or a shared mounted data root.
  Multi-replica rollout requires a shared object-store implementation and transactional outbox.

## 12. Rollout and rollback

1. Land prerequisite versioned/project-scoped migration support.
2. Add schema and read-compatible domain types without serving schema v2.
3. Add authoring/artifact backend behind `governed_resource_delivery` feature flag.
4. Ship Conductor Resource Studio; existing JSON resources remain readable but cannot be converted to
   Plugin without explicit import/validation.
5. Ship EvoFlux schema-v2 client with old snapshot fallback for Agent/Skill only.
6. Enable Beta for internal members, then Published delivery after cross-repo tests.
7. Enable inventory reconciliation and retire the compatibility snapshot after one supported client cycle.

Rollback disables new release creation and schema-v2 advertisement, leaves immutable rows/artifacts in
place, and returns clients to last-known-good managed content. Migrations are forward-only; destructive
down migration is not used. A bad release is rolled back by creating a new greater version from known
content, never by mutating history.

## 13. Test strategy

| Layer | Required proof |
|---|---|
| Rust domain | strict SemVer, automatic candidate, state transitions, diagnostic/error serialization |
| Rust storage | fresh/upgrade SQLite and PostgreSQL migrations; project constraints; concurrent allocation; audience and cursor queries |
| Axum routes | every role/scope/owner state, direct artifact denial, archive limits, stale Draft/version conflicts |
| React | all editor/release/access states, keyboard behavior, accessible diagnostics and mobile layout |
| EvoFlux pytest | durable cursor, digest/conflict matrix, atomic Agent/Skill replacement, Plugin trust/update, project switching and offline replay |
| Cross-repo | same slugs in two projects, two members with Beta/Published, Plugin trust pending, promotion, target removal, tombstone and inventory convergence |
| Security | malicious ZIP corpus, secret markers, credential/path absence and authorization matrix |
| UI E2E | create/import/edit/validate/Beta/Publish and EvoFlux pull/review/activate; desktop/mobile screenshots |

Shared fixture packages live in a repository-neutral test-fixture directory copied or pinned into both
suites with the same SHA-256. Each documented starter must pass both validators.

## 14. Traceability

| Requirement criteria | Components | Tasks |
|---|---|---|
| REQ-007 AC-1–AC-13 | resource domain, Draft/version storage, release API | TSK-007-01, 02, 03, 04 |
| REQ-007 AC-14–AC-30 | Plugin artifact store, validators, Resource Studio | TSK-007-02, 03, 04, 07 |
| REQ-007 AC-31–AC-38 | project constraints and SemVer transaction | TSK-007-01, 04, 12 |
| REQ-008 AC-1–AC-14 | policy resolver, Beta audience and preview | TSK-007-05, 07, 12 |
| REQ-010 AC-1–AC-16 | static safety, artifact authorization and local trust | TSK-007-02, 09, 12 |
| REQ-012 AC-1–AC-14 | sync scheduling, managed locations and status | TSK-007-06, 08, 10 |
| REQ-012 AC-15–AC-34 | Plugin staging/update and Beta convergence | TSK-007-06, 08, 09, 10, 12 |
| REQ-012 AC-35–AC-38 | project isolation and switching | TSK-007-08, 12 |
| REQ-013 AC-1–AC-17 | inventory ingest, collector and health views | TSK-007-11, 12 |

## 15. Task breakdown

| Task | Layer | Description | Depends on |
|---|---|---|---|
| TSK-007-01 | BE | Add project-scoped resource/release schema and domain | prerequisite REQ-001/003 |
| TSK-007-02 | BE | Build safe Draft workspace, archive import and validators | 01 |
| TSK-007-03 | BE | Add content-addressed Plugin artifact storage | 01, 02 |
| TSK-007-04 | BE | Implement transactional Auto/Manual releases and lifecycle | 01–03 |
| TSK-007-05 | BE | Resolve access, Beta audience and effective versions | 01, 04 |
| TSK-007-06 | BE | Expose cursor changes and authorized artifact delivery | 03–05 |
| TSK-007-07 | FE | Build Resource Studio, release and audience UI | 02, 04, 05 |
| TSK-007-08 | EvoFlux | Persist project-scoped managed state and reconcile Agent/Skill | 06 |
| TSK-007-09 | EvoFlux | Integrate Plugin staging, trust and atomic update | 06, 08 |
| TSK-007-10 | EvoFlux FE | Build sync status, diff and trust-review experience | 08, 09 |
| TSK-007-11 | Cross-repo | Implement privacy-safe desired-versus-observed inventory | 05, 08, 09 |
| TSK-007-12 | QA/Infra | Prove security, Beta, version, project isolation and compatibility | 01–11 |

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-11 | Created coordinated cross-repository design from accepted resource-delivery requirements | Codex |
| 2026-08-11 | Task planning created as an explicit owner-requested exception; tasks remain blocked until design approval | Codex |
| 2026-08-11 | Approved when the project owner directed implementation, build and full-stack verification | Codex |
