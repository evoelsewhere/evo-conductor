# REQ-012 — Resource synchronization client

| | |
|---|---|
| ID | REQ-012 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Accepted — cursor client implemented; smart-fetch checkout migration remains |
| Priority | P0 |
| Build order | Step 13 of 23 |
| Spec section | [requirements.md sections 6 and 12](../requirements.md) |
| Source | Baseline specification sections 6, 12 and 14; EvoFlux Portable Agent Plugins update 2026-08-11 |
| Depends on | REQ-007, REQ-008, REQ-011 |
| Blocks | V1 acceptance criteria 6, 12 and 14 |
| Repositories | `evoflux` primarily, `evo-conductor` for the change endpoint |
| Design | [DES-007](../design/09-DES-007-governed-resource-delivery.md) sections 5.3, 6.2 and 9 — Approved 2026-08-11 |

## 1. Context

This requirement delivers the visible benefit of the whole system: a member opens EvoFlux and the
project's approved Agents, standalone Skills, Portable Agent Plugins, prompts and documents are
available. A Portable Agent Plugin is one governed package which may contribute several Skills and
declared tool servers; it is not the legacy EvoFlux Python-hook plugin system.

The consumption side already exists in EvoFlux, so the work is retrieval, placement and conflict
handling. The single largest risk is damaging the user's own files, which would destroy trust in the
integration immediately and permanently.

## 2. Requirement

EvoFlux shall retrieve the server-resolved Beta or Published resources, plugin artifacts and documents permitted for the token
owner, shall materialize them through the existing agent, Skill and portable plugin runtimes, and shall
never modify repository files or overwrite user-owned content. Conductor owns desired versions and
assignment; EvoFlux owns local trust decisions, credentials, mutable plugin data and observed runtime
state. Every managed Agent, standalone Skill and Plugin shall retain the server-issued project identity
that delivered it; an EvoFlux installation shall never identify, reconcile or display a managed resource
by kind/slug alone.

### 2.1 Version discovery, diff and pull contract

Conductor is authoritative for the desired published version. EvoFlux shall not decide whether a
version is newer by comparing semantic-version strings: a rollback may intentionally publish an older
package as a new immutable version. Identity and change detection use the server-issued `resource_id`,
server-issued `version_id` and published content/artifact SHA-256. Slug is display and lookup metadata,
not reconciliation identity.

`GET /api/v1/resources/changes?cursor=<cursor>` shall return authorized changes in a versioned envelope:

```json
{
  "schema_version": 1,
  "project_id": "prj_01J...",
  "next_cursor": "cursor_43",
  "has_more": false,
  "changes": [
    {
      "project_id": "prj_01J...",
      "resource_id": "res_01J...",
      "version_id": "rv_01J...",
      "kind": "plugin",
      "slug": "release-audit",
      "version": "1.2.0",
      "release_channel": "beta",
      "content_sha256": "sha256:...",
      "artifact_sha256": "sha256:...",
      "artifact_size": 182400,
      "minimum_evoflux_version": "0.9.0",
      "download_url": "/api/v1/resources/res_01J.../versions/rv_01J.../artifact",
      "requires_trust": true,
      "tombstone": false
    }
  ]
}
```

The envelope schema version applies to every change. The envelope and every change carry the same
server-issued `project_id`, derived from the authenticated connection token. EvoFlux shall compare that
value with the project identity returned during registration and reject the entire page as
`project_scope_mismatch` if it is absent, inconsistent or different; the body never selects project
authorization. `release_channel` is `beta` or `published` and is
the version Conductor resolved for the authenticated token owner; it is not selected by the client.
`has_more` requires the client to continue from
`next_cursor` until the page is exhausted. Fields that do not apply to an inline text resource may be
absent, but every delivered version has one canonical SHA-256. Download URLs are authorized references,
not durable identity, and may expire. A tombstone carries stable resource identity and the removed
version/policy revision without a payload.

EvoFlux shall persist managed state keyed by `(project_id, resource_id)`, including last desired and applied
`version_id`, semantic version, release channel, applied SHA-256, ownership marker, managed local target, plugin
installation ID when applicable, reconciliation state and the last committed cursor. It shall evaluate a
change as follows:

1. If desired `version_id`, release channel and SHA-256 equal applied state, do nothing.
2. If `version_id` or release channel changes but SHA-256 is identical, update desired metadata without
   rewriting content or repeating executable trust.
3. If SHA-256 changes, retrieve the complete payload/artifact into staging, verify size and digest, then
   validate it before touching the active version. V1 does not use binary delta patches.
4. Before replacing an existing managed target, compare its actual local SHA-256 with the last applied
   SHA-256. Equality proves that the target is still Conductor-owned and unchanged; inequality produces
   `ownership_conflict` and preserves the local target.
5. A same-kind/same-slug local resource with no matching ownership record is user-owned. It is never
   silently adopted or overwritten.
6. Non-executable resources apply atomically at a safe boundary. Executable resources stage as
   `trust_pending` or `update_pending`; the previous trusted version remains active until acceptance.
7. Persist observed state and advance the cursor only after the change has reached a durable applied,
   pending-review, conflict, declined, incompatible or removed state. A failed or interrupted change is
   replayed idempotently on the next request.

When an administrator changes the authenticated member's Beta selection, Conductor shall emit a new
desired-state change even if the resource access policy itself did not change. Selection resolves to the
active Beta; deselection resolves to the active Published version, or to a tombstone if no Published
fallback exists. EvoFlux treats that change exactly like any other immutable version transition and does
not retain a local preference for the Beta channel.

Diff is a review surface rather than an apply mechanism. For agents and standalone Skills, EvoFlux shall
show canonical text/frontmatter and added, removed or changed files. For plugins it shall compare package
metadata, file inventory, contributed Skills and declared tool servers, commands/arguments, working directories,
remote hosts, environment-field names and capabilities. Package bytes, credentials and environment
values are never rendered as a raw diff or reported to Conductor.

### 2.2 Project ownership and local namespace

EvoFlux shall maintain a dedicated logical managed root for each project, keyed by immutable
`project_id`, for example `{CONFIG_DIR}/conductor/projects/<project_id>/`. Project slug and display name
may be shown to the member but shall not be used as the directory identity because they can change or
collide. Agent, Skill and Plugin ownership metadata shall contain `project_id`, `resource_id`,
`version_id`, kind and applied digest. Plugin registry/install mappings shall carry the same project
scope even when the existing runtime uses a separate physical package store.

V1 permits one active Conductor project per EvoFlux installation. Replacing the connection token with a
token for another project triggers registration before synchronization, creates or selects the new
project namespace, and disables/unmounts Conductor-managed Agents, Skills, Plugins and documents from the
previous project. Previously cached bytes may remain for offline rollback, but they cannot be discovered,
loaded, activated, updated or reported as belonging to the new project. Reconnecting the original project
may reuse only ownership state whose `project_id` matches exactly.

### 2.3 Current transport evolution

The cursor change feed remains the implemented EvoFlux compatibility client. Conductor now additionally
implements `POST /api/v1/resources/fetch`, a Git-style `have_commit`/`have` negotiation over the complete
member-specific Agent/Skill/Plugin tree. It returns a deterministic desired commit/tree, changed entries,
managed tombstones and only missing content-addressed objects. New EvoFlux clients shall migrate to this
contract and activate one fully verified staged generation atomically; cursor delivery remains supported
until fleet convergence. [resource-fetch-protocol.md](../resource-fetch-protocol.md) is normative for the
new checkout algorithm.

## 3. Implementation status

| Implemented | Missing or incomplete |
|---|---|
| EvoFlux has typed schema-v2 change pages, stable `(project_id, resource_id)` managed state, durable cursor, digest verification and project-scoped Agent/Skill activation | EvoFlux does not yet call Conductor's newer smart-fetch endpoint or atomically switch a complete desired-tree generation |
| Governed reconciliation preserves user-owned content, detects local drift/ownership conflicts, keeps last-known-good state and processes authorized tombstones | A real two-project packaged switch smoke and same-slug cross-repository fixture remain |
| Plugin artifacts are downloaded, size/SHA checked, revalidated and staged through the existing Plugin platform with stable installation mapping | Binary bundle support remains outside the current UTF-8 authoring contract |
| Plugin first install/update uses trust/update-pending states, preserves prior trusted runtime/private data and reports safe inventory | A signed third-party provenance model remains deferred |
| EvoFlux settings and Intelligence surfaces show project identity, sync state, resource/version/channel, diff/review actions and conflicts | Full Playwright coverage for every pending/error/project-switch state is not committed |
| Conductor provides cursor changes, direct effective-version authorization, immutable artifacts, realtime invalidation and smart-fetch `have` negotiation | Cursor and smart-fetch are both live during migration; the client currently uses cursor delivery |
| Current focused EvoFlux tests cover governed reconciliation, runtime provenance, telemetry and review UI | No single automated test boots both repositories and proves the complete checkout/trust/inventory flow |

### Acceptance progress

| AC | State |
|---|---|
| AC-1, AC-4–AC-18, AC-20–AC-33, AC-35–AC-37, AC-39–AC-53 | Implemented or substantially implemented on the current EvoFlux feature branch using cursor delivery |
| AC-2, AC-3, AC-19, AC-34, AC-38, AC-54 | Partial; documentation/workspace-root proof, smart-fetch checkout and packaged cross-repository proof remain |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Synchronization runs on connection, on a configurable interval, and on manual request |
| AC-2 | Resources of type `agent` and standalone `skill` are materialized with Conductor ownership metadata, including `project_id` and `resource_id`, into project-scoped managed locations consumed by EvoFlux's existing agent and Skill loaders; documents and context resources are written to the matching Conductor project directory |
| AC-3 | The Conductor-owned directory is registered as an extra workspace root, so its `AGENTS.md` reaches the system prompt |
| AC-4 | EvoFlux never writes `AGENTS.override.md`, verified by an explicit test asserting the file is not created |
| AC-5 | EvoFlux never writes inside the user's repository working tree |
| AC-6 | A file modified locally is not overwritten; the conflict is reported naming the file and the nature of the difference |
| AC-7 | Only the effective Beta or Published version permitted by the access policy is written, satisfying V1 acceptance criterion 6 |
| AC-8 | Checksums are used to skip unchanged content instead of rewriting files on every cycle |
| AC-9 | `GET /api/v1/resources/changes?cursor=` returns only changes since the cursor, and the client persists the cursor |
| AC-10 | When Conductor is unreachable, EvoFlux continues to operate using the previously synchronized content |
| AC-11 | A revoked token stops synchronization on the next cycle and informs the user, satisfying V1 acceptance criterion 12 |
| AC-12 | A synchronization status view shows last run time, resources received, changes applied and any errors |
| AC-13 | Resources removed from the member's permitted set are removed locally, and the removal is reported |
| AC-14 | When a new version of an already-synchronized resource is published, EvoFlux retrieves the desired version within one synchronization cycle. Non-executable resources converge immediately; executable resources report `trust_pending` or `update_pending` until accepted, then converge at the next safe boundary |
| AC-15 | A permitted published `plugin` resource is downloaded as an artifact, checked against the published size and SHA-256 digest, revalidated by EvoFlux, and installed through the existing portable plugin platform rather than by directly copying package files |
| AC-16 | First receipt of a Conductor-managed Plugin creates or stages it disabled and exposes EvoFlux's static trust review; synchronization alone never starts its declared processes or enables its Skills |
| AC-17 | After the member trusts and enables a plugin, a published update preserves the stable local installation mapping, credentials and `PLUGIN_DATA`; a failed validation or update leaves the prior runnable version intact |
| AC-18 | A changed plugin artifact requires a new visible review before the new version becomes active. Declining or deferring the update keeps the prior trusted version and reports `update_pending`, not `in_sync` |
| AC-19 | Archiving or removing assignment of a Conductor-managed plugin disables its runtime and removes only the Conductor-owned package version; EvoFlux preserves installation data and credentials unless the member explicitly deletes them locally |
| AC-20 | A user-owned Agent, Skill or Plugin installation with the same name is never overwritten or silently adopted. EvoFlux reports a named ownership conflict and preserves both the local object and last-known-good managed state |
| AC-21 | Plugin credentials, credential values, mutable installation data and local package paths are never uploaded to Conductor or delivered in a resource manifest |
| AC-22 | An installation that cannot run a plugin's schema or minimum EvoFlux version reports `incompatible` and continues operating without repeatedly downloading or partially installing the artifact |
| AC-23 | Every change response carries `schema_version`, authenticated `project_id`, `next_cursor` and `has_more`; every entry repeats the matching `project_id` and carries stable `resource_id` and immutable `version_id`, kind, slug, semantic version, server-resolved release channel, applicable content/artifact SHA-256 and size, minimum compatible EvoFlux version, trust requirement and tombstone state; reconciliation never uses slug as identity |
| AC-24 | EvoFlux persists managed state keyed by `(project_id, resource_id)`, including desired/applied version IDs, semantic version, release channel, applied SHA-256, ownership marker, local project-scoped target, applicable plugin installation ID, state and committed cursor |
| AC-25 | The client follows the decision matrix in section 2.1: identical version/channel/digest is skipped; a version or channel metadata change with identical digest does not rewrite or re-prompt; a changed digest is staged and validated; Conductor's desired immutable version wins over semantic-version ordering |
| AC-26 | Before replacement, EvoFlux compares actual local SHA-256 with its last applied SHA-256. A mismatch or a same-name object without ownership metadata returns `ownership_conflict` and no local content is overwritten or adopted |
| AC-27 | The update review shows canonical Agent/Skill text and file changes, or Plugin manifest, file inventory, Skills, declared tool servers and executable trust-surface changes, without exposing package bytes, credentials or environment values |
| AC-28 | V1 downloads a complete payload/artifact into staging, verifies size and SHA-256, validates it and performs an atomic switch with rollback; partial/binary delta application is not used |
| AC-29 | The client advances and durably persists the change cursor only after each returned change reaches a durable applied, `trust_pending`, `update_pending`, removed, conflict, declined or incompatible state; interrupted work is replayed idempotently and cannot skip a version |
| AC-30 | Archive, unassignment and loss of access are returned as authorized tombstones. EvoFlux removes or disables only the matching Conductor-owned object and reports the observed result |
| AC-31 | Every non-tombstone change includes `release_channel` as `beta` or `published`; the server chooses the channel from authenticated member identity, resource policy and beta assignment, and rejects direct retrieval of a version that is not effective for that member |
| AC-32 | Adding a member to Beta emits the Beta version on the next cursor cycle; removing or invalidating the target emits the Published fallback, or a tombstone when none exists; replay and cursor-commit rules remain idempotent across both transitions |
| AC-33 | A Beta artifact follows the same digest, compatibility, static trust and explicit local activation rules as Published; beta targeting never implies local trust or enablement |
| AC-34 | End-to-end tests use two member tokens against one resource and prove only the selected eligible member receives Beta while the other receives Published, then both converge correctly after promotion or target removal |
| AC-35 | EvoFlux rejects a changes page, artifact reference or tombstone whose project ID is missing or differs from the project returned by registration, records `project_scope_mismatch`, advances no cursor and modifies no managed resource |
| AC-36 | Agent, Skill and Plugin managed locations, ownership markers and Plugin installation mappings retain `project_id`; the EvoFlux resource/status UI displays the connected project name and exposes the stable project ID in details or diagnostics |
| AC-37 | Replacing the active token with one for another project disables or unmounts the former project's managed resources before activating the new namespace; cached prior-project bytes remain isolated and are never adopted, updated or reported under the new project |
| AC-38 | Cross-repository tests create the same Agent, Skill and Plugin slugs in two projects and prove each token receives, stores, loads, inventories and removes only the `(project_id, resource_id)` objects belonging to its authenticated project |
| AC-39 | Agent mode `work|coding` is materialized in the correct runtime namespace; removing a mode deletes only a managed copy whose ownership and digest still match |
| AC-40 | Skill mode uses the canonical EvoFlux sidecar, and managed mode/invocation policy is read-only through EvoFlux |
| AC-41 | An effective Agent retains every built-in/default and managed tool/Skill/MCP entry; local settings are an ordered, deduplicated additive union and cannot subtract the managed base |
| AC-42 | Local Agent additions are keyed by stable project/resource identity, survive version updates and are re-applied after each atomic managed switch |
| AC-43 | Tombstone or unassignment removes the managed layer but preserves built-in/default entries, local additions and user-owned resources |
| AC-44 | The EvoFlux managed-Agent UI clearly separates the locked managed base from locally editable model/additions; raw managed source is not writable |
| AC-45 | Provider badges/details show project, version and mode; a Coding-only Agent never lends managed provenance to a Work object with the same slug |
| AC-46 | Regression tests prove managed A+B plus local C becomes A+B+C, and an update to A+B+D still retains C without duplication |
| AC-47 | A legacy package without a mode sidecar remains compatible as Both; invalid sidecars are rejected before local mutation or cursor commit |
| AC-48 | An older installation missing one mode copy replays the authoritative feed and backfills the same version; a locally edited target still becomes `ownership_conflict` |
| AC-49 | Intelligence/status shows installed/current separately from latest desired version for Agent, Skill and Plugin resources |
| AC-50 | A new desired version creates a visible banner; info/review shows description, every skipped changelog, channel and SemVer gap before Pull |
| AC-51 | An update to an installed version is never auto-applied. Local `POST /api/settings/conductor/resources/{resource_id}/pull` explicitly refetches the authorized payload, verifies it and performs the atomic apply |
| AC-52 | Deprecating the installed version emits a change and shows a non-permanently-dismissible `Update required` state with reason; failed Pull preserves last-known-good |
| AC-53 | Plugin Pull only downloads and stages the new version disabled; local trust approval remains a separate activation step and inventory transition |
| AC-54 | A smart-fetch client sends the active `have_commit` and managed `have` set, verifies the returned complete-tree commit and missing Bundle V2 objects, then atomically switches one staged generation; a failure leaves the active generation and inventory acknowledgement unchanged |

## 5. Out of scope

- Automatic activation of Portable Agent Plugins. Receipt and staging are
  covered here; executable trust and activation are governed by
  [REQ-010](19-REQ-010-plugin-distribution-safety.md).
- Distribution of legacy Python hook files from `app/agent/plugins`; only Portable Agent Plugins 1.0 are
  in scope.
- Uploading local inventory, covered by [REQ-013](14-REQ-013-inventory-synchronization.md).
- Arbitrary per-member version pinning. The server-resolved explicit-member Beta channel in
  [REQ-007](09-REQ-007-resource-lifecycle.md) is in scope.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Overwriting a user's own work | High | AC-4, AC-5, AC-6 |
| 2 | `AGENTS.override.md` is used because the name appears correct, silently discarding project instructions | High | AC-4 makes this a test, not a convention |
| 3 | EvoFlux becomes dependent on network availability | Medium | AC-10 |
| 4 | Agents can read the Conductor-owned directory because it is also a sandbox root ([sandbox.py:120-123](../../../evoflux/app/agent/sandbox.py)) | Low | Document the behaviour so it is not a surprise |
| 5 | Deletion logic removes files the user created in the managed directory | Medium | AC-13 limits removal to previously synchronized paths recorded in local state |
| 6 | A package update destroys local credentials or plugin data | High | AC-17 and AC-19 preserve installation-scoped private state |
| 7 | Conductor publication becomes remote execution without member consent | High | AC-16 and AC-18 require local trust before activation |
| 8 | A same-name local plugin is mistaken for the managed installation | High | AC-20 uses stable ownership and installation mappings rather than names alone |
| 9 | A rollback is ignored because its semantic version sorts below the installed version | High | AC-23 and AC-25 use immutable desired version identity, not client-side semantic ordering |
| 10 | Cursor advances before an artifact is applied, permanently skipping a failed update | High | AC-29 commits the cursor only after durable idempotent processing |
| 11 | Current `kind/slug` state adopts or overwrites an unrelated local object | High | AC-24 and AC-26 require stable resource identity and ownership proof |
| 12 | EvoFlux caches a Beta after the user is removed from its target list | High | AC-31, AC-32 and AC-34 require a channel-aware change and fallback/tombstone convergence |
| 13 | Same-name Agent, Skill or Plugin from another project is adopted, activated or deleted | High | AC-23, AC-24 and AC-35–AC-38 use immutable project-scoped identity across contract, storage and runtime |

## 7. Open questions

- What is the default synchronization interval?
- Should plugin artifact downloads use the same cursor endpoint with a signed/authorized artifact URL,
  or stream from Conductor directly? The client contract must remain identical for either storage
  implementation.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-11 | Added Portable Agent Plugin pull, update, trust, ownership and offline-safe reconciliation requirements; reconciled current branch implementation | Codex |
| 2026-08-11 | Added stable identity, cursor commit, version decision matrix, diff preview and atomic pull contract after QA review | Codex |
| 2026-08-11 | Added server-resolved Beta channel delivery, target removal fallback and two-member isolation tests | Codex |
| 2026-08-11 | Added project-scoped resource identity, local namespace isolation and safe project-switch behavior for EvoFlux | Codex |
| 2026-08-11 | Accepted into the coordinated governed-resource design by project-owner request | Codex |
| 2026-08-14 | Reconciled the implemented EvoFlux governed reconciler/trust UI and added the newer Conductor smart-fetch migration contract | Codex |
