# REQ-010 — Plugin distribution safety

| | |
|---|---|
| ID | REQ-010 |
| Created | 2026-08-09 |
| Updated | 2026-08-11 |
| Status | Accepted (2026-08-11; owner requested design and task planning) |
| Priority | P0 |
| Build order | Step 19 of 23 |
| Spec section | [requirements.md section 6](../requirements.md), addition |
| Source | Risk analysis 2026-08-09; EvoFlux Portable Agent Plugins update 2026-08-11 |
| Depends on | REQ-007, REQ-012 |
| Blocks | Portable Agent Plugin activation; V1 acceptance criterion 14 for executable resources |
| Repositories | `evo-conductor` and `evoflux` |
| Design | [DES-007](../design/09-DES-007-governed-resource-delivery.md) sections 5.3 and 10 — Approved 2026-08-11 |

## 1. Context

EvoFlux supports Portable Agent Plugins 1.0: a package with `plugin.json`, optional Skills and optional
technical `mcp.json` declarations
([agent-plugins.md](../../../evoflux/documents/architecture/agent-plugins.md)). Conductor governs that
whole artifact as one Plugin resource. Plugins differ from Agents and standalone Skills in one decisive
respect: they may declare executable commands, arguments, working directories, environment-field names
and remote hosts.

Distributing a Plugin can therefore become remote code execution by configuration. A bad prompt degrades
answers; a malicious Plugin package can start an unknown process on every member machine. It must not
share the trust level of a normal text resource.

The exposure is real today rather than theoretical: any authenticated user can currently mint a
fully scoped token, and there is no audit log.

## 2. Requirement

Portable Agent Plugin packages shall follow a stricter publication and activation path than
non-executable resources. Publication shall be limited to Admin. A member's
EvoFlux installation shall never activate a newly received or changed executable resource without a
local static trust review and explicit confirmation. Conductor may assign and deliver desired state; it
may not supply local credentials or bypass the member's trust decision.

Upload, extraction, editing and validation in Conductor shall remain static. Conductor shall inspect
archive metadata, manifests, declarations, text and paths without importing package modules, starting
declared processes, resolving remote URLs or running bundled scripts. Archive-safety violations are
rejected before extraction under [REQ-007](09-REQ-007-resource-lifecycle.md). Content diagnostics may be
repaired in the isolated Draft, but an executable resource with a fatal manifest or declared-server diagnostic or a
suspected embedded credential value cannot enter Beta or Published.

## 3. Implementation status

| Implemented | Missing or incorrect |
|---|---|
| A legacy technical `ResourceKind::Mcp` variant exists in the catalog ([resource.rs](../../crates/conductor-domain/src/resource.rs)) | `ResourceKind::Plugin`, legacy-data migration, artifact governance and Admin-only executable publication |
| EvoFlux's portable plugin platform validates packages, rejects unsafe archives, computes a content digest and performs atomic managed updates ([validator.py](../../../evoflux/app/plugin_platform/validator.py), [installer.py](../../../evoflux/app/plugin_platform/installer.py)) | Conductor-to-EvoFlux plugin artifact delivery and stable managed ownership mapping |
| EvoFlux builds a static trust disclosure for executable commands, remote hosts, environment fields and capabilities ([trust.py](../../../evoflux/app/plugin_platform/trust.py)) | A Conductor sync state that distinguishes staged, trust-pending, update-pending, active and declined |
| EvoFlux applies normal tool permissions after Plugin activation | The current Conductor reconciler has a legacy configuration path without a Conductor-specific Plugin trust gate ([reconciler.py:163-173](../../../evoflux/app/conductor/reconciler.py)) |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Only Admin may publish a resource of type `plugin`; Contributor receives `403` even when they own the Draft, and no separate executable product kind is exposed |
| AC-2 | The console displays an explicit executable-resource warning before publishing a Plugin artifact and summarizes whether local commands, remote hosts, environment fields or capabilities are declared |
| AC-3 | EvoFlux stages a newly received Plugin disabled and waits for explicit local confirmation before starting a process or exposing its contributed Skills/tools |
| AC-4 | The confirmation screen displays exact executable commands and arguments, working directories, remote hosts, environment-field names, declared capabilities and affected Skills/tool servers; it never displays credential or environment values |
| AC-5 | Declining is remembered for the resource version and digest, so the user is not prompted on every synchronization; Conductor sees only the non-sensitive state `declined` |
| AC-6 | Any changed Plugin artifact digest triggers a new review before the new version activates; the previously trusted version remains active until the update is accepted or revoked |
| AC-7 | When an Admin archives or unassigns a Plugin, EvoFlux disables and stops its managed runtime on the next synchronization without deleting locally held credentials or mutable Plugin data |
| AC-8 | Publication, modification, assignment and retirement of Plugins are recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |
| AC-9 | Conductor manifests and artifacts contain no credential values, generated credential files, global tool-server secrets or mutable `PLUGIN_DATA`; those remain owned by the EvoFlux installation |
| AC-10 | EvoFlux verifies the published artifact digest and reruns its own archive/package validator before trust review; digest or validation failure cannot modify the current installation |
| AC-11 | `plugin` in this requirement means a Portable Agent Plugin package. Conductor never distributes or activates legacy Python hook files from EvoFlux's `app/agent/plugins` directory |
| AC-12 | Enabling a Plugin or one of its declared servers does not bypass EvoFlux's normal tool permission pipeline, sandbox boundaries or per-agent selection rules |
| AC-13 | Upload and validation never execute or import package code, start Plugin-declared commands, run scripts, contact package-declared hosts or interpolate package environment values; an integration test uses a package with observable side effects and confirms none occur |
| AC-14 | Archive-safety failures reject extraction; fatal `plugin.json`/`mcp.json` diagnostics remain editable in an isolated Draft but block Beta and Publish; nonfatal warnings are visibly distinguished and require the [REQ-007](09-REQ-007-resource-lifecycle.md) acknowledgement policy |
| AC-15 | Conductor scans releasable source for probable embedded credential values. A match blocks Beta/Publish, identifies only file and location/category, masks the value in UI/log/audit output and guides the author to declared credential fields or placeholders |
| AC-16 | Beta uses exactly the same Admin-only publication permission, executable warning and local EvoFlux trust gate as Published; selecting beta members never starts a runtime or records trust on their behalf |

## 5. Out of scope

- Signing or external provenance verification of Plugin packages. Conductor's authenticated publisher,
  immutable artifact and SHA-256 are required for V1; third-party signatures remain P2.
- Sandboxed execution of Plugin-declared servers, which belongs to EvoFlux rather than to Conductor.
- Central distribution of legacy Python hooks or arbitrary frontend/application code.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | A compromised Admin account can run code on every machine in the project | High | AC-3 and AC-4 keep the user as the final gate |
| 2 | Users approve prompts reflexively without reading them | Medium | AC-4 shows the real command; AC-6 re-prompts only on genuine change |
| 3 | The confirmation step is seen as friction and removal is requested | Medium | AC-5 limits prompting to actual changes |
| 4 | A trusted plugin update swaps in a different executable payload | High | AC-6 and AC-10 bind review and activation to the immutable artifact digest |
| 5 | Credentials leak into a centrally published package or inventory report | High | AC-4 and AC-9 prohibit values at every wire boundary |
| 6 | Validation itself executes attacker-controlled package code on the Conductor server | High | AC-13 requires static-only inspection with a side-effect regression fixture |
| 7 | A validation diagnostic leaks the secret it detected | High | AC-15 returns masked location/category metadata only |

## 7. Open questions

- Should a "trust this Conductor completely" mode exist that bypasses AC-3? Recommendation: no. This is
  the final boundary between centralized configuration management and remote control of a member's
  machine, and it should not be configurable away.
- Should a Skill-only plugin update require renewed confirmation when its executable trust surface is
  unchanged? Recommendation: show the update diff and require acceptance for V1; optimize the prompt only
  after the trust model has an explicit signed component-digest contract.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-11 | Expanded safety coverage to Portable Agent Plugins and aligned it with EvoFlux's trust-review platform | Codex |
| 2026-08-11 | Added static-only ZIP/editor validation, masked secret blocking and Beta safety parity | Codex |
| 2026-08-11 | Renamed the requirement and standardized all product-facing safety behavior on Plugin | Codex |
| 2026-08-11 | Accepted into the coordinated governed-resource design by project-owner request | Codex |
