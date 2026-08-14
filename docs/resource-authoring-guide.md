# Resource authoring guide — Agent, Skill and Portable Agent Plugin

Status: **Normative content implemented by the current Conductor Resource Studio and validators; shared
cross-repository fixture and security-hardening gaps remain tracked in TSK-007-02/07/12.**

This guide defines the source that Conductor accepts for governed Agent, standalone Skill and Portable
Agent Plugin resources. The structures follow EvoFlux's current parsers and authoring surfaces. Conductor
must version this guide together with its validators and downloadable starter packages; see
[REQ-007](requirement/09-REQ-007-resource-lifecycle.md).

## Choose the resource kind first

| Kind | Use it for | Upload forms |
|---|---|---|
| Agent | One team member definition: role, model, tools, Skills, Plugin-provided server selection and system prompt | `.md` or `.zip` containing exactly one root `.md` |
| Skill | One reusable instruction bundle loaded on demand, with optional references, scripts, assets and UI metadata | `.zip` rooted at `SKILL.md`; direct `SKILL.md` is accepted for a text-only Skill |
| Portable Agent Plugin | One Agent Plugins 1.0 package that may contribute several Skills and declared tool servers | `.evoplugin` or `.zip` rooted at `plugin.json` |

Choose the kind in Resource Studio before uploading. Conductor validates against that selected contract
and does not silently turn a Skill into a Plugin. Plugin is the only governed executable package kind.

Project ownership is catalog metadata, not author-controlled source. Authors shall not place a
`project_id` inside Agent frontmatter, `SKILL.md` or `plugin.json` to select delivery. Conductor assigns
the resource to the authenticated project, and the sync contract later delivers that immutable project
identity to EvoFlux. This allows identical resource names in different projects without changing the
portable Agent, Skill or Plugin file format.

## Agent

An Agent is one UTF-8 Markdown file. YAML frontmatter configures the agent and the Markdown body is its
system prompt. A minimal member Agent is:

```markdown
---
name: release-reviewer
role: member
description: Reviews release evidence and reports concrete blockers.
---

You are the release reviewer. Inspect the supplied evidence, separate blockers from follow-up work, and
report the exact checks that support each conclusion.
```

Supported frontmatter fields mirror EvoFlux `AgentConfig`:

| Field | Rule |
|---|---|
| `name` | Required by the Conductor guide and must equal the resource slug/file stem |
| `role` | `lead` or `member`; defaults to `member` in EvoFlux, but Conductor templates write it explicitly |
| `description` | Optional human-readable purpose |
| `model`, `fallback_model` | Optional `provider:model` identifiers |
| `thinking_level` | Optional level supported by the selected model |
| `responses_api` | Optional boolean provider-interface preference |
| `tools`, `tools_opt_out`, `skills` | Optional lists of names |
| `mcp` | Optional technical EvoFlux field containing Plugin-provided server names; this literal key is retained for file-format compatibility and is not a separate Conductor resource kind |

Conductor validates one Agent resource. EvoFlux still validates the effective destination team, including
its exactly-one-lead invariant and whether referenced models/tools exist. A locally conflicting team does
not authorize Conductor to overwrite user-owned Agent files.

## Standalone Skill

A Skill is a directory bundle rooted at `SKILL.md`:

```text
release-audit/
├── SKILL.md
├── agents/
│   └── evoflux.yaml          # optional EvoFlux interface metadata
├── references/               # optional on-demand knowledge
├── scripts/                  # optional deterministic helpers
├── assets/                   # optional output/template assets
└── evals/
    └── trigger-cases.json    # optional trigger boundary cases
```

The portable `SKILL.md` frontmatter contains only `name` and `description`. The directory and
frontmatter names match and use 1–64 lowercase letters/digits joined by single hyphens:

```markdown
---
name: release-audit
description: Audit release readiness when a user asks for evidence, blockers, or a go/no-go review.
---

# Release audit

1. Identify the release scope and required evidence.
2. Check each gate and record observable evidence.
3. Separate blockers, accepted risk, and follow-up work.
4. Return a go/no-go recommendation with unresolved uncertainty.
```

The description and instruction body are non-empty; description is at most 1,024 characters and
`SKILL.md` is at most 512 KiB. Optional resources should be linked from `SKILL.md` at the step that needs
them. Put EvoFlux UI metadata in `agents/evoflux.yaml`, not in portable frontmatter.

## Portable Agent Plugin

A Plugin is an Agent Plugins 1.0 package. It is distinct from EvoFlux's legacy trusted Python hooks:

```text
release-audit/
├── plugin.json
├── skills/
│   └── release-audit/
│       ├── SKILL.md
│       ├── references/       # optional
│       ├── scripts/          # optional
│       └── assets/           # optional
└── mcp.json                  # optional
```

Minimal `plugin.json`:

```json
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "release-audit",
  "version": "0.1.0",
  "description": "Audit a release with a guided Skill and declared tools.",
  "author": {
    "name": "Example Team"
  },
  "extensions": {}
}
```

Plugin names are 1–64 lowercase ASCII letters, digits, hyphens or dots; start and end with a letter or
digit; and contain neither `--` nor `..`. Use semantic versions. `author` is an object. Unknown root
manifest fields produce warnings; platform-specific declarations belong under `extensions`.

Only immediate directories below `skills/` are discovered as Plugin Skills. Each discovered Skill
follows the standalone `SKILL.md` name/description/body rules. A package with no Skill and no declared server
is valid. `.evoplugin` is a deterministic ZIP wrapper around this directory, not a second manifest
format.

## Upload and extraction

Resource Studio applies these steps:

1. Upload to quarantine.
2. Inspect archive metadata without executing package code.
3. Reject unsafe archives before extraction.
4. Normalize at most one unambiguous wrapper directory; missing or extra required root files become
   editable validation diagnostics.
5. Extract regular files into the resource's server-owned Draft workspace.
6. Validate and open the Draft in the editor.

An archive is rejected without an editor workspace when it contains an absolute or traversal path,
duplicate or case-fold-colliding path, symlink, unsupported entry type, too many entries, excessive
compressed/expanded bytes, or a suspicious compression ratio. Current Plugin compatibility targets are
2,000 entries, 200 MiB expanded, 50 MiB compressed and a 200:1 limit for entries larger than 1 MiB;
Conductor may impose a lower documented server limit but never a higher limit that EvoFlux cannot accept.

A safe archive with content errors opens as a Draft under the kind selected before upload. For example,
a missing `plugin.json`, several root Agent Markdown files, a name mismatch or invalid JSON is editable;
Beta and Publish remain blocked until the package matches the selected guide.

## Validation messages

| Severity | Result | Example |
|---|---|---|
| Security rejection | Import stops; no Draft files are retained | Traversal path, symlink or archive bomb |
| Error | Draft can be saved and edited; Beta/Publish are blocked | Missing `plugin.json`, invalid frontmatter, name mismatch |
| Warning | Draft can be saved; release requires acknowledgement unless security policy blocks it | Unknown `plugin.json` root field |
| Valid | Beta/Publish actions are available subject to role/access policy | All structural and release checks pass |

Each diagnostic identifies a stable code, severity, file, line or field when available, explanation,
suggested correction and this guide's matching section. Suspected secret values are never echoed: the
diagnostic shows only the masked category and location and blocks release.

Conductor validation is static. It never imports Python/JavaScript, starts a Plugin-declared command, runs a bundled
script, resolves a declared host or substitutes environment values. EvoFlux revalidates every received
artifact and applies its separate local trust gate before executable content becomes active.

## Edit the extracted Draft

Resource Studio uses the EvoFlux authoring interaction as its reference:

- responsive file tree and Monaco source editor;
- syntax mode chosen from the selected file;
- selected path, line numbers, search and jump-to-diagnostic;
- dirty/saved/error state and `Ctrl/Cmd+S`;
- create, rename and delete with confirmation;
- unsaved-navigation protection;
- binary assets shown by name, type and size but not decoded as text.

The browser never sends an absolute workspace root. Every read/write targets a normalized relative path
under the authenticated resource's Draft workspace. Server-side file, entry and total-size limits are
authoritative.

## Save, Beta and Publish

### Version assignment

Resource Studio uses `Auto` versioning by default. The first immutable release is `0.1.0`; each later
Beta or direct Published release receives the next greater patch version calculated by the server. Save,
validation and Beta member changes do not increment. Promoting an existing Beta reuses its immutable
version and does not bump again.

Choose `Manual` only when intentionally making a major, minor or explicit patch release. The value must
be strict SemVer 2.0, unique and greater than every version already allocated for this resource. Inputs
such as `v1.2.3`, `1.2`, `01.2.3`, whitespace-padded values, duplicates and lower versions are rejected
with an inline field error. A failed release does not consume a version.

For Plugins, the released `plugin.json.version` must equal the Conductor version. Auto mode shows this
manifest change in the release preview and includes it in the validated artifact digest. Manual mode
requires the saved manifest value to match exactly. Agent and Skill source files do not need an embedded
Conductor version.

| Action | Meaning | Delivered to EvoFlux? |
|---|---|---|
| Save Draft | Persist the mutable source workspace and rerun validation | No |
| Release Beta | Freeze a deterministic immutable version and select explicit eligible member IDs | Only those selected members |
| Publish | Freeze a valid Draft directly to Published, or promote the same immutable Beta version | All members permitted by resource policy |
| Deprecate | Retire a release-channel binding while preserving version history | No; members fall back to another active channel or receive a tombstone |
| Archive | Close normal authoring/distribution while preserving history and audit records | No |

Beta selection never grants resource access. Every selected ID must be an active member of the project
who already passes the resource access policy. A selected member receives the active Beta; other eligible
members receive the active Published version. Selection is by member, so all of that member's authorized
installations resolve the same channel; tokens and installations are not separate beta targets. If there is no Published version, non-selected members
receive nothing. Removing a selected member returns them to Published or produces a tombstone when no
Published fallback exists.

Promotion points the Published channel at the same immutable Beta version ID, preserves bytes and
SHA-256, retires the Beta binding and records an audit event. Editing Beta, Published or Deprecated
content always starts a new Draft; it never changes released bytes.

## Source references

- EvoFlux Agent schema: [`app/agent/config.py`](../../evoflux/app/agent/config.py)
- EvoFlux Skill authoring contract: [`skill-installer/SKILL.md`](../../evoflux/app/agent/builtin_skills/skill-installer/SKILL.md)
- EvoFlux Plugin package contract: [`package-contract.md`](../../evoflux/app/agent/builtin_skills/plugin-development/references/package-contract.md)
- EvoFlux Plugin editor: [`PluginWorkspaceEditor.tsx`](../../evoflux/web/src/components/PluginWorkspaceEditor.tsx)
- EvoFlux safe Plugin workspace: [`workspace.py`](../../evoflux/app/plugin_platform/workspace.py)
- Conductor lifecycle requirement: [REQ-007](requirement/09-REQ-007-resource-lifecycle.md)
- Access and Beta isolation: [REQ-008](requirement/10-REQ-008-resource-access-policy.md)
- Client delivery: [REQ-012](requirement/13-REQ-012-resource-sync-client.md)
- Executable safety: [REQ-010](requirement/19-REQ-010-plugin-distribution-safety.md)
