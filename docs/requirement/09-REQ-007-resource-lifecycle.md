# REQ-007 — Resource model, versioning and lifecycle

| | |
|---|---|
| ID | REQ-007 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Accepted — substantial implementation landed; residual criteria remain |
| Priority | P0 |
| Build order | Step 9 of 23 |
| Spec section | [requirements.md section 6](../requirements.md) |
| Source | Baseline specification section 6; EvoFlux Agent, Skill and Portable Agent Plugin authoring review 2026-08-11 |
| Depends on | REQ-001, REQ-004, REQ-018 |
| Blocks | REQ-008, REQ-010, REQ-012 |
| Repositories | `evo-conductor` |
| Design | [DES-007](../design/09-DES-007-governed-resource-delivery.md) — Approved 2026-08-11 |

## 1. Context

This is the core value of Conductor: standardizing agents, prompts, skills, portable Agent Plugins and
configuration across a project. EvoFlux now implements the portable core of Agent Plugins 1.0: a package
has a root `plugin.json`, immediate-child Skills and an optional `mcp.json`, and a deterministic
`.evoplugin` file is a ZIP wrapper around that package
([agent-plugins.md](../../../evoflux/documents/architecture/agent-plugins.md)).

Most resource content is structured text, but the source shape differs by kind. EvoFlux stores an agent
as one Markdown file whose YAML frontmatter carries configuration and whose body is the system prompt
([config.py](../../../evoflux/app/agent/config.py)). A standalone Skill is a directory bundle rooted at
`SKILL.md`, with optional `agents/`, `references/`, `scripts/`, `assets/` and `evals/` content
([skill-installer/SKILL.md](../../../evoflux/app/agent/builtin_skills/skill-installer/SKILL.md)). A Portable
Agent Plugin is a directory rooted at `plugin.json`, with immediate-child Skills and optional `mcp.json`
([package-contract.md](../../../evoflux/app/agent/builtin_skills/plugin-development/references/package-contract.md)).

EvoFlux already exposes the authoring interaction to follow. Its plugin workspace uses Monaco, a bounded
file tree, dirty-state protection, save, validation and deterministic pack actions
([PluginWorkspaceEditor.tsx](../../../evoflux/web/src/components/PluginWorkspaceEditor.tsx)); its backend
limits editing to UTF-8 regular files under a normalized package root and prevents traversal, symlink
editing and deletion of root `plugin.json`
([workspace.py](../../../evoflux/app/plugin_platform/workspace.py)). Conductor needs the same package
semantics and safety boundaries, adapted to a server-owned draft rather than an arbitrary host path.

## 2. Requirement

Conductor shall store resources with a full metadata set and an explicit lifecycle, shall provide
kind-specific authoring guides and templates, shall create a mutable draft source workspace from a
template or uploaded package, shall create an immutable version on every beta or publish action, shall
allow rollback, and shall validate source before release. Published versions shall be distributed to all
members permitted by the resource access policy. Beta versions shall be distributed only to explicitly
selected permitted members.

Resource types shall be Agent, standalone Skill, Portable Agent Plugin package, workflow, command,
document or project policy, and reusable prompt template. `plugin` shall be the first-class governed
Plugin kind: one package can contain several Skills and declared tool-server configurations and has one
package version and artifact digest.

Every resource, mutable Draft, immutable version, release-channel binding, Beta target and access policy
shall belong to exactly one server-issued `project_id`. A resource cannot be moved between projects after
creation. Slug uniqueness is scoped to `(project_id, kind, slug)`, while the server-issued `resource_id`
remains the durable identity used by APIs and synchronization. The authenticated project context selects
the project; management APIs shall reject a conflicting client-supplied `project_id` rather than using it
to choose another project.

For `kind = plugin`, Conductor shall store immutable `.evoplugin`/ZIP artifacts or an immutable artifact
reference. Every Beta or Published version shall record the Agent Plugins schema version, package name/version,
artifact size, SHA-256 digest and minimum compatible EvoFlux version. Credentials and installation data
are never part of the resource artifact.

### 2.1 Canonical source guides

The Resource Studio shall display a versioned guide and downloadable starter template for the selected
kind. The normative content starts in
[resource-authoring-guide.md](../resource-authoring-guide.md); the in-product guide and server validator
shall describe the same contract:

| Kind | Canonical editable source | Minimum valid structure |
|---|---|---|
| `agent` | One UTF-8 Markdown file | YAML frontmatter with `name` and a valid `role`; optional `description`, model, fallback model, thinking level, response-interface switch, tools, tool opt-outs, Skills and Plugin-provided server names; Markdown body is the system prompt. The frontmatter name shall match the resource slug. |
| `skill` | Directory bundle | Root `SKILL.md` with only portable `name` and `description` frontmatter and a non-empty instruction body. Name is 1–64 lowercase letters/digits joined by single hyphens and matches the bundle directory. Optional resources follow EvoFlux's `agents/`, `references/`, `scripts/`, `assets/` and `evals/` conventions. |
| `plugin` | Agent Plugins 1.0 directory package | Root `plugin.json`; optional immediate-child `skills/<name>/SKILL.md`; optional root `mcp.json`. `.evoplugin` is a deterministic ZIP wrapper, not another manifest format. |

The Agent guide shall distinguish Conductor's single-resource validation from EvoFlux's team-directory
invariant: a package can be a valid agent resource without proving that a future recipient directory has
exactly one `role: lead`. EvoFlux remains responsible for reporting any deployment conflict with the
recipient's effective team.

### 2.2 Create, upload, extract and validate

An authorized author shall be able to start from a guide template, upload a direct source file where the
kind permits it, or upload a `.zip`; plugin upload shall also accept `.evoplugin`. Archive import shall:

1. Write the upload to quarantine without executing, importing or rendering package code.
2. Reject the archive before extraction when it has an absolute/traversal path, duplicate or
   case-fold-colliding path, symlink, unsupported entry type, excessive compressed/expanded size, too
   many entries or a suspicious compression ratio.
3. Normalize at most one unambiguous wrapper directory. The selected-kind validator then expects an
   agent archive to contain exactly one root `.md` source file, a Skill archive to contain root
   `SKILL.md`, and a plugin archive to contain root `plugin.json`; violations are repairable content
   diagnostics rather than archive-safety failures.
4. Extract regular files into a server-owned draft workspace addressed by resource ID, never into the
   Conductor source tree or an operator-supplied filesystem path.
5. Run the kind validator and return structured diagnostics containing severity, stable code, file,
   line/field when available, message, suggested correction and a link to the matching guide section.

Archive-safety errors reject import and create no editable workspace. A structurally safe upload that
does not match the selected guide shall still open under the author-selected kind as a Draft, show the
errors and allow the author to add, rename or remove files until it is valid. Errors block Beta and Publish. Warnings allow Save and may
be acknowledged for release unless [REQ-010](19-REQ-010-plugin-distribution-safety.md) classifies them as a
security blocker. The original upload may be retained as audit evidence with its checksum, but the
editable draft tree becomes the canonical source for later packaging.

### 2.3 Resource Studio editor

The Conductor Resource Studio shall follow EvoFlux's Plugin Workspace Editor interaction and visual
language: responsive file tree, Monaco source editor with syntax selection by file type, selected-file
path, line numbers, search, dirty marker, `Ctrl/Cmd+S`, Save status, new file/directory, rename/delete with
confirmation, validation, diagnostics navigation and an unsaved-navigation guard. Agent resources may
also expose a Form/Raw view using the same fields as EvoFlux's agent editor, but Raw is the canonical
lossless source.

The editor shall read and write only normalized relative paths inside the resource's draft workspace.
It shall not accept a client-provided absolute root. UTF-8 text is editable; binary assets remain visible
in the tree with type and size but are not decoded into the editor. File count, per-file size and total
workspace size are bounded server-side. Saving a file reruns validation and returns current diagnostics.

### 2.4 Save, Draft, Beta and Published semantics

`Save` persists the mutable draft source tree and does not create desired state for EvoFlux. A resource
may have one current mutable Draft. `Beta` and `Publish` each build a deterministic artifact or canonical
payload from the fully saved draft, validate it, compute its SHA-256 and create an immutable content
snapshot. A release-channel binding then points `beta` or `published` at that version ID. Released bytes
are never edited in place: editing a beta, published or deprecated version creates or updates a separate
Draft.

The UI state shall be `draft`, `beta`, `published` or `deprecated`; Beta and Published are release-channel
bindings over immutable versions rather than mutable copies of the payload. Resource lifecycle state
remains `draft`, `published`, `deprecated` or `archived`. The supported transitions are:

```text
draft --release to selected members--> beta --promote same bytes--> published --> deprecated
draft --------------------publish-----------------------> published
beta/published/deprecated --edit or restore--> new draft
resource --archive--> archived
```

V1 shall allow at most one active Beta and one active Published version per resource. A Beta stores an
explicit set of member IDs. The set can contain only active members who already pass the resource access
policy; it cannot broaden normal access. Targeting is by member, not connection token or installation,
so every authorized installation owned by a selected member resolves the same Beta. For a selected member the active Beta is desired; every other
authorized member receives the active Published version. If no Published version exists, non-selected
members receive no version. Removing a beta member or retiring the Beta returns that member to the
Published version, or sends a tombstone when there is no Published fallback. Promotion creates the
Published binding to the same version ID, preserves the immutable bytes and digest, retires the Beta
binding and records an audited transition; it does not rewrite or duplicate version content.

### 2.5 Automatic and manual semantic versioning

Version allocation is server-owned and occurs only when a Beta or direct Published action creates a new
immutable version from the saved Draft. `Save`, validation, Beta audience changes, deprecation and archive
do not increment it. Promoting an existing Beta to Published reuses the same version ID and semantic
version because no new content snapshot is created.

The default release mode is `auto`. The first released version of a resource is `0.1.0`. Each later
automatic release selects the next greater patch version from the highest semantic-version precedence
already allocated for that resource. If the highest version is a prerelease, its matching stable version
is the next automatic candidate when that is greater; build metadata never counts as an increment. A
rollback may reuse older content, but it still receives a new greater semantic version and immutable
version ID.

Resource Studio shall show the current highest version, the server-calculated next version and an
`Auto`/`Manual` control. Manual mode accepts a trimmed version only after strict SemVer 2.0 parsing. It
rejects a `v` prefix, whitespace, missing components, leading zeroes, invalid prerelease/build identifiers,
a duplicate, or any version whose SemVer precedence is not greater than every version already allocated
for that resource. Manual major/minor bumps are allowed; after one is released, later automatic releases
continue from it.

The server recalculates and allocates the version transactionally during release. A validation failure,
authorization failure or storage error creates no version and consumes no number. Concurrent releases
for one resource yield at most one successful allocation; a stale request receives `409 version_conflict`
with the refreshed next version rather than silently choosing another number.

For a Plugin, the immutable artifact's `plugin.json.version` shall exactly equal the allocated resource
version. Auto mode shows the manifest version change in the release preview and applies it
deterministically inside the release transaction before final validation and digesting; if release fails,
the Draft and manifest remain unchanged. Manual mode requires the saved manifest and requested version to
match exactly. Agent and Skill source formats do not gain a Conductor-specific embedded version field.

## 3. Implementation status

The governed catalog implementation is now in the main Conductor source. Drafts and immutable releases
are deterministic ZIP objects outside SQL; SQL keeps content-addressed keys, digests, sizes and Bundle V2
manifests. Local, S3-compatible, Azure Blob and Git storage backends share that contract, and the admin
storage migration verifies every copied object before switching the live backend.

| Implemented | Remaining gap |
|---|---|
| Real `plugin` domain/API/UI terminology with legacy `mcp` parsing/backfill compatibility | A complete legacy-data compatibility report when an old row cannot be converted safely |
| Project-scoped resource/version/channel/Beta/change/inventory schema; strict SemVer; transactional Auto/Manual release | General versioned migrations and true multi-project server membership remain REQ-001/REQ-003 gaps |
| Agent/Skill/Plugin guides, starter templates, direct/ZIP import, safe extraction, editable object-backed Draft tree, Monaco Resource Studio and structured static diagnostics | Binary authoring/executable-bit support and the complete shared cross-repo fixture corpus are deferred |
| Immutable Bundle V2 artifacts, SHA-256/tree manifests, authorized descriptor/artifact reads, ETag caching, cursor changes and Git-style smart fetch negotiation | The artifact route currently reads an object into memory before responding; streaming proof remains open |
| Beta/Published effective resolution, active-member targeting, version history, deprecate, restore-to-Draft, archive, monitoring, feedback and inventory views | Beta target validation checks active membership but not full policy eligibility; same-version Beta promotion is not a dedicated source transition |
| Resource Studio exposes Auto/Manual release, validation, draft editing, version lifecycle and monitoring | Frontend unit/e2e infrastructure is absent under REQ-020 |
| The 2026-08-14 Rust workspace run passes 94 tests, including archive import, lifecycle, Bundle V2, smart fetch and storage migration coverage | Project-wide audit events, Plugin Admin-only publication, credential-pattern scanning and full PostgreSQL/cross-repo proof remain open |

### Acceptance progress

| AC | State |
|---|---|
| AC-1, AC-4, AC-5, AC-7, AC-9, AC-10, AC-12, AC-14–AC-16, AC-19–AC-25, AC-27, AC-30, AC-31, AC-33, AC-34, AC-36, AC-37 | Implemented in current source/tests |
| AC-2, AC-3, AC-6, AC-8, AC-11, AC-17, AC-18, AC-26, AC-28, AC-29, AC-32, AC-35, AC-38 | Partial or blocked by the residual gaps above |
| AC-13 | Not complete; REQ-018 remains open |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | `POST /api/resources` creates a resource and draft workspace; `PATCH /api/resources/{id}` updates metadata; Save updates draft source; Beta and Publish create immutable versions; Deprecate and Archive perform validated lifecycle transitions |
| AC-2 | Each resource carries a stable server-issued resource ID and slug, name, description, type, owner, visibility, lifecycle status, timestamps and tags; each immutable version carries its own server-issued version ID, semantic version, payload/artifact reference, content checksum, publisher and change notes |
| AC-3 | Resource lifecycle status is one of `draft`, `beta`, `published`, `archived`; immutable version state is one of `draft`, `beta`, `published`, `deprecated`; invalid transitions return `409` |
| AC-4 | Every Beta or direct Publish creates an immutable content row in `resource_versions` and a channel binding; released payload/artifact bytes are never modified or deleted |
| AC-5 | `GET /api/resources/{id}/versions` returns the history, and rollback republishes a prior version as a new version rather than mutating history |
| AC-6 | Only Admin and Contributor may create or update drafts; a User receives `403`. Publication applies the stricter per-kind rules in [REQ-010](19-REQ-010-plugin-distribution-safety.md) |
| AC-7 | A payload exceeding the size limit is rejected at publish time with a message stating both the limit and the actual size |
| AC-8 | Server limits are explicit and compatible with the current editable bundle contract: archive upload 20 MiB, extracted Draft 50 MiB, 2,000 files, editable UTF-8 file 1 MiB, and standalone/plugin `SKILL.md` 512 KiB; any tighter EvoFlux runtime limit is enforced before activation |
| AC-9 | For `kind = agent`, the payload is validated as parseable YAML frontmatter plus a body; a malformed payload is rejected |
| AC-10 | A content checksum is computed and stored, and is exposed to clients for change detection |
| AC-11 | `ResourceCounts` counts every resource type, including `command` and any type added later |
| AC-12 | Distribution returns the active Beta only to its explicit eligible members and the active Published version to every other permitted member; Draft and Deprecated versions are never desired state |
| AC-13 | Save, Beta release/target change/promotion, Publish, update, deprecate, archive and rollback are recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)); high-frequency editor keystrokes are not audit events |
| AC-14 | `plugin` is the only Plugin resource kind; the domain/API/UI never expose `mcp` as a product kind, and existing legacy rows are migrated or rejected with an explicit compatibility error |
| AC-15 | Publishing a plugin version accepts a `.evoplugin` or compatible ZIP artifact only after validating its root `plugin.json`, package layout, declared semantic version, size limits and Agent Plugins schema version |
| AC-16 | Every Beta or Published plugin version stores an immutable artifact and SHA-256 digest; downloading the effective authorized version returns bytes whose digest and length match the released metadata |
| AC-17 | A plugin version records its package name/version and minimum compatible EvoFlux version, and Conductor rejects a version whose resource slug/package identity changes unexpectedly |
| AC-18 | Plugin packages and resource payloads never contain member credentials, generated credential values or mutable installation data |
| AC-19 | The console exposes a versioned Agent, Skill and Plugin guide plus downloadable starter files/archives matching the canonical structures in section 2.1 |
| AC-20 | An author can create from a template, upload the direct source accepted for the kind, or upload ZIP; plugin also accepts `.evoplugin`; one wrapper directory is normalized and the result opens as an editable draft workspace |
| AC-21 | Archive tests prove absolute/traversal paths, duplicate and case-fold-colliding paths, symlinks, unsupported entries, excessive file count/size and suspicious compression ratios are rejected before extraction and leave no draft files behind |
| AC-22 | A safely extracted but invalid package returns structured error/warning diagnostics with code, file, line/field where available, fix guidance and guide link; errors block Beta/Publish while warnings follow the acknowledgement policy |
| AC-23 | Resource Studio uses Monaco and a responsive file tree, supports save/dirty protection, create/rename/delete, binary metadata, validation and diagnostic navigation, and confines all server writes to normalized paths below the resource draft root |
| AC-24 | `Ctrl/Cmd+S` persists a Draft without distributing it; unsaved navigation warns; Save success/error and current validation state are visible and accessible without relying on color alone |
| AC-25 | Beta and Publish package only the last fully saved, validated Draft, compute deterministic bytes and SHA-256, and leave the Draft independently editable; a released version can only be changed by creating a new version |
| AC-26 | A Beta requires an explicit non-empty member-ID set; the API rejects inactive, cross-project or policy-ineligible members, applies the selection consistently to all installations owned by each selected member, and records the target-set change in the audit log |
| AC-27 | For an eligible selected member Beta overrides Published; non-selected members never receive Beta; removal from Beta falls back to Published or returns a tombstone when no Published version exists |
| AC-28 | Promotion from Beta to Published points the Published channel at the same immutable version ID, preserves artifact/payload bytes and digest, retires the Beta binding, records an auditable transition and makes the version desired for the normal access-policy audience |
| AC-29 | Contract fixtures prove every starter package documented as valid passes both the Conductor validator and the corresponding EvoFlux parser/validator, while shared invalid fixtures produce compatible diagnostic categories |
| AC-30 | Import and validation are static: Conductor never starts Plugin-declared processes, imports package Python/JavaScript or resolves package-provided code during upload, editing, validation or packaging |
| AC-31 | Every resource, Draft, version, channel binding, Beta target and policy is associated with exactly one `project_id`; the API derives that project from authenticated context and rejects cross-project resource IDs or a conflicting body/query project ID |
| AC-32 | Two projects may use the same kind and slug without collision; listing, lookup, version history, publication, artifact download and audit tests prove each request returns or mutates only the authenticated project's resource |
| AC-33 | The first Beta or direct Published release defaults to `0.1.0`; every later auto release creates the next greater patch version from the resource's highest allocated SemVer precedence, while Save, audience edits, deprecation and archive allocate nothing |
| AC-34 | Manual mode accepts only strict SemVer 2.0 whose precedence is greater than every allocated version for that resource; invalid, duplicate, equal/lower-precedence, prefixed or whitespace-bearing input returns a field-specific `422` without changing the Draft or version history |
| AC-35 | Beta-to-Published promotion preserves the same immutable version ID, semantic version, bytes and digest; rollback of old content creates a new greater version instead of reusing or decreasing a version number |
| AC-36 | Version selection and immutable-version creation are one transaction: failed releases consume no version, concurrent releases cannot duplicate an allocation, and stale auto/manual requests receive `409 version_conflict` plus the refreshed next version |
| AC-37 | A Plugin release artifact has `plugin.json.version` exactly equal to the allocated version. Auto mode previews and transactionally applies that manifest change before validation/digesting; manual mismatch returns `422 manifest_version_mismatch`; any failure leaves the Draft unchanged |
| AC-38 | Resource Studio defaults to Auto, displays highest and next version, validates Manual inline and on the server, and the release audit event records mode, prior highest, requested manual value when present, allocated version and immutable version ID |

## 5. Out of scope

- Plugin activation constraints, which are stricter; see
  [REQ-010](19-REQ-010-plugin-distribution-safety.md).
- Documents, which have their own model; see [REQ-009](18-REQ-009-document-management.md).
- Access targeting; see [REQ-008](10-REQ-008-resource-access-policy.md).
- Percentage, cohort, tag-based or time-window canary rollout. V1 Beta targets explicit member IDs only.
- Collaborative real-time editing, arbitrary server filesystem access and Git/registry import.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | A bad prompt edit changes agent behaviour across the whole team with no traceability | High | AC-4, AC-5 and AC-13 |
| 2 | Published content is silently truncated on member machines | Medium | AC-7 and AC-8 reject at the source |
| 3 | A new resource type is added while lower layers are not updated | Medium | AC-11, which addresses a defect already present with `command` |
| 4 | Version and plugin artifact history grows without bound | Medium | Keep immutable history but define artifact retention/object-storage policy in design and measure real package sizes |
| 5 | A package accepted by Conductor is rejected by EvoFlux because their validators drift | High | AC-15 records the schema version; cross-repo contract tests use the same fixture artifacts |
| 6 | A plugin artifact is substituted after publication | High | AC-16 makes the version immutable and verifies SHA-256 at delivery and installation |
| 7 | A malicious ZIP writes outside draft storage or exhausts server resources | High | AC-21 rejects unsafe paths, links, collisions and archive bombs before extraction |
| 8 | The guide says a package is valid but EvoFlux rejects it | High | AC-29 makes guide examples and cross-repo fixtures part of the validator contract |
| 9 | A Beta leaks to a member who was not selected | High | AC-26 and AC-27 apply targeting server-side and require negative endpoint tests |
| 10 | Saving an invalid draft unexpectedly changes active clients | High | AC-24 and AC-25 separate mutable Save from immutable release actions |
| 11 | Same-name resources from different projects collide or are published to the wrong clients | High | AC-31 and AC-32 make project ownership immutable and test cross-project isolation |
| 12 | Two publishers allocate the same next version or a failed release leaves a skipped number | High | AC-33 and AC-36 make allocation server-owned and transactional |
| 13 | Plugin manifest version differs from catalog/version history and EvoFlux installs ambiguous bytes | High | AC-37 makes the artifact manifest, immutable record and digest one atomic release result |

## 7. Open questions

- Deprecation removes that version from desired state immediately; if another active channel version
  exists, resolution falls back to it. Archive additionally closes editing and hides the resource from
  the normal catalog while preserving history and audit evidence.
- Where are plugin artifacts stored in V1: database blob, local object directory or external object
  storage? The design must keep the download contract independent of that choice.
- Should a warning require one acknowledgement per version or one acknowledgement per diagnostic code?
  Recommendation: persist acknowledgements by draft revision and diagnostic code so any content change
  reruns review.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-11 | Added first-class Portable Agent Plugin lifecycle and reconciled the catalog implementation status | Codex |
| 2026-08-11 | Added EvoFlux-compatible authoring guides, safe ZIP-to-draft import, Monaco Resource Studio, structured validation and explicit-member Beta releases | Codex |
| 2026-08-11 | Standardized the product model on `plugin` as the only governed executable package kind; retained legacy technical names only as migration/file-format evidence | Codex |
| 2026-08-11 | Made project ownership part of every resource lifecycle object and required same-slug cross-project isolation | Codex |
| 2026-08-11 | Added server-owned automatic patch versioning, strict manual SemVer validation, concurrency rules and Plugin manifest synchronization | Codex |
| 2026-08-11 | Accepted for coordinated design and task planning by project-owner request | Codex |
| 2026-08-14 | Reconciled the landed Resource Studio, Bundle V2/object storage, release, smart-fetch and monitoring source; retained verified residual gaps | Codex |
