# EvoFlux resource bundle v2 contract

Status: **Conductor emits v2 descriptors for new Agent, Skill and Plugin releases; EvoFlux consumption is pending**

`ResourceBundleV2` is the canonical content-addressed wire descriptor for files delivered from Conductor to EvoFlux. It is deliberately limited to `agent`, `skill` and `plugin`; governed `workflow` and `command` records keep their existing delivery behavior.

## Target modes

Agent and Skill drafts use `.evoflux.json` to declare where they are available:

```json
{
  "modes": ["work", "coding", "aim"]
}
```

The array must be non-empty, contain each selected value once, and use only `work`, `coding` or `aim`. Missing mode metadata on a newly scaffolded resource defaults to all three modes. EvoFlux should treat the list as an allow-list, not as display-only metadata.

## Canonical JSON shape

New immutable versions expose `bundle_v2` on the version descriptor returned by `GET /api/v1/resources/{resource_id}/versions/{version_id}` and on Conductor's authenticated version-list response:

```json
{
  "schema_version": 2,
  "kind": "skill",
  "slug": "release-audit",
  "version": "1.4.0",
  "artifact_sha256": "64 lowercase hexadecimal characters",
  "artifact_size": 2741,
  "artifact_media_type": "application/vnd.evoflux.resource+json",
  "tree_sha256": "64 lowercase hexadecimal characters",
  "files": [
    {
      "path": ".evoflux.json",
      "sha256": "64 lowercase hexadecimal characters",
      "size": 56,
      "media_type": "application/json",
      "executable": false
    },
    {
      "path": "SKILL.md",
      "sha256": "64 lowercase hexadecimal characters",
      "size": 985,
      "media_type": "text/markdown",
      "executable": false
    }
  ]
}
```

Wire types:

```text
ResourceBundleV2 {
  schema_version: 2
  kind: "agent" | "skill" | "plugin"
  slug: string
  version: strict SemVer 2.0 string
  artifact_sha256: lowercase hex SHA-256
  artifact_size: non-negative integer bytes
  artifact_media_type: media type
  tree_sha256: lowercase hex SHA-256
  files: FileManifestEntry[]
}

FileManifestEntry {
  path: safe relative UTF-8 path
  sha256: lowercase hex SHA-256 of exact file bytes
  size: non-negative integer bytes
  media_type: media type inferred by Conductor
  executable: boolean
}
```

`files` is sorted by path using bytewise UTF-8 ordering. Every path is relative to the resource root. Absolute paths, traversal, symlinks, duplicate paths and case-insensitive collisions are rejected before release.

## Tree digest algorithm

`tree_sha256` covers file identity and execution-relevant metadata, independently of ZIP entry order. Compute SHA-256 over these UTF-8 bytes:

```text
"evoflux-resource-tree-v2\n"
for each FileManifestEntry sorted by path:
  path + NUL
  + sha256 + NUL
  + decimal(size) + NUL
  + media_type + NUL
  + ("1" if executable else "0") + LF
```

No trailing normalization, Unicode normalization, newline conversion or path separator conversion is allowed. The file digest always covers the exact bytes represented by the released version.

## Artifact semantics

| Kind | `artifact_media_type` | Current artifact semantics |
|---|---|---|
| Plugin | `application/vnd.evoflux.plugin+zip` | SHA-256 and size of the immutable ZIP returned by the artifact endpoint |
| Agent / Skill | `application/vnd.evoflux.resource+json` | SHA-256 and size of Conductor's canonical content-only JSON before the descriptor is attached |

For Plugin, EvoFlux must verify `artifact_sha256` before extraction and then verify every extracted file against `files` and `tree_sha256`. Plugins still require local trust review before enablement.

For Agent and Skill, `tree_sha256` plus each file digest is the interoperable verification contract. Conductor does not yet expose their canonical JSON as a separate byte-for-byte artifact; therefore EvoFlux must not hash a reserialized HTTP object and compare it with `artifact_sha256`.

## Change-feed descriptor

`GET /api/v1/resources/changes` keeps legacy `sha256` and `size` fields and additively emits these optional fields for v2 releases:

```json
{
  "bundle_schema_version": 2,
  "artifact_sha256": "...",
  "tree_sha256": "...",
  "artifact_media_type": "application/vnd.evoflux.resource+json",
  "file_count": 4
}
```

The change feed intentionally omits the full file manifest to keep polling bounded. After detecting a new `version_id`, EvoFlux fetches the version descriptor, validates `bundle_v2`, stages files in a temporary directory, verifies all digests, and atomically swaps the staged tree into place. Inventory acknowledgement happens only after that swap.

## Backward compatibility

- Existing `sha256`, `size`, `payload` and artifact routes are unchanged.
- All new delivery fields are optional on outer descriptors and omitted for legacy records, tombstones, Workflow and Command.
- Change-feed consumers must ignore unknown optional object fields; only an unsupported `schema_version` is a protocol incompatibility.
- EvoFlux may continue its v1 path when `bundle_schema_version` or `bundle_v2` is absent.
- Unknown future bundle schema versions must be retained as unavailable, not partially installed.

## Current implementation gaps

The v2 schema is additive and requires no database migration because Conductor stores the descriptor with the immutable version payload. The following work remains before claiming full binary bundle support:

1. Preserve ZIP Unix mode bits and binary file bytes. The current editor/import path accepts UTF-8 text only, so Conductor truthfully emits `executable: false` for every file.
2. Expose immutable Agent and Skill artifact bytes through the artifact endpoint, with the same pre-extraction size and digest guarantees as Plugin.
3. Validate all SHA-256 fields and media-type grammar at the domain boundary instead of relying only on Conductor-generated values.
4. Add EvoFlux staging, verification, atomic activation, rollback and inventory reporting. This repository intentionally does not modify EvoFlux yet.
5. Move bundle metadata into dedicated storage columns only if querying by tree digest becomes a demonstrated need; the current payload-backed representation avoids a premature artifact-store migration.
