# EvoFlux resource smart-fetch protocol

Status: **Conductor implemented; EvoFlux checkout adapter pending**
Protocol schema: `1`

This is the normative delivery contract for Agent, Skill and Plugin bundles. REST is the authenticated transport. Consistency comes from content-addressed objects, complete desired-tree commits, `have` negotiation, verification and atomic checkout—not from replaying REST mutations in order.

## Why the snapshot client is retired

The legacy EvoFlux client downloads `/api/v1/subscribe/resources` on every poll, compares an ETag only after downloading the response, then rewrites resources individually. It does not use the durable change feed, immutable artifacts or inventory acknowledgement. Its expected Agent and Skill payload shapes also differ from Conductor's file-bundle contract.

That model has four correctness problems:

- reconnect cost grows with the full catalog even when nothing changed;
- a failure can expose a checkout containing only part of a release;
- deletes are not bounded by managed ownership and can affect local content;
- a received row is treated as applied even when its files, dependencies or trust state were not validated.

Git itself commonly uses HTTP. The relevant design is its object and checkout model:

| Git concept | Resource delivery equivalent |
|---|---|
| remote ref / HEAD | member-specific desired commit |
| commit | `commit.id` for one complete desired checkout |
| tree | sorted accessible Agent, Skill and Plugin entries |
| blob | immutable Bundle V2 ZIP addressed by `artifact_sha256` |
| `have` / `want` negotiation | request `have`; response changed entries and missing objects |
| index | EvoFlux staging directory plus verified manifest |
| worktree | active EvoFlux managed generation |
| atomic ref update | switch the active-generation pointer after full validation |

This first version deliberately does not implement Git packfiles or binary deltas. SHA-addressed object reuse removes redundant bundle downloads; pack negotiation can be added without changing checkout semantics.

## Control plane

`GET /api/v1/realtime/events` is an optional low-latency invalidation channel. On connection it emits `control.hello`, then:

```text
event: resources.head
data: {"reason":"initial","fetch_url":"/api/v1/resources/fetch"}
```

A release, archive or relevant access change emits `resources.changed`. Lag emits `control.resync_required`. These events never carry authored file bytes and are not a durable mutation log. A missed event is harmless because a later smart fetch computes the authoritative desired tree. Clients must also fetch periodically or during heartbeat recovery so policy changes cannot remain stale.

## Negotiate a checkout

```http
POST /api/v1/resources/fetch HTTP/1.1
Authorization: Bearer evc_<prefix>_<secret>
Content-Type: application/json
```

```json
{
  "installation_id": "f81d4fae-7dec-11d0-a765-00a0c91e6bf6",
  "have_commit": "1f1b...64-lowercase-hex...c92a",
  "have": [
    {
      "resource_id": "7f99e8b5-8540-49c6-976e-828a82793a67",
      "version_id": "67e21b4e-1a92-4424-b732-5910e1a75ad7",
      "artifact_sha256": "e36a...64-lowercase-hex...7c11"
    }
  ]
}
```

The installation must belong to the authenticated member and project. `have` contains only objects in Conductor's managed registry; it is capped at 5,000 unique resource IDs. On first checkout, send `have_commit: null` and an empty `have` array.

Example response:

```json
{
  "schema_version": 1,
  "project_id": "4cbad77b-f907-4f40-b975-b2997b1a3743",
  "base_commit": "1f1b...c92a",
  "commit": {
    "id": "a7ea...f0d2",
    "tree_sha256": "d74a...a991",
    "sequence": 184
  },
  "up_to_date": false,
  "entries": [
    {
      "resource_id": "7f99e8b5-8540-49c6-976e-828a82793a67",
      "version_id": "5d5010d9-c48c-4749-af95-47213dd93a2e",
      "kind": "skill",
      "slug": "release-audit",
      "version": "1.4.0",
      "release_channel": "published",
      "bundle": { "schema_version": 2, "artifact_sha256": "..." },
      "minimum_evoflux_version": null,
      "trust_required": false
    }
  ],
  "tombstones": [],
  "objects": [
    {
      "artifact_sha256": "e36a...7c11",
      "size": 2741,
      "media_type": "application/vnd.evoflux.resource+zip",
      "href": "/api/v1/resources/7f99e8b5-8540-49c6-976e-828a82793a67/versions/5d5010d9-c48c-4749-af95-47213dd93a2e/artifact"
    }
  ]
}
```

`entries` is the changed part of the tree, not the complete tree. `objects` is deduplicated by artifact digest and excludes objects the client declared in `have`. `tombstones` applies only to resource IDs the client declared as Conductor-managed. If `have_commit` equals the desired commit, `up_to_date` is true and all three plan arrays are empty.

`commit.sequence` is an observed database watermark for diagnostics. It is not the commit identity and must not be used to decide content equality.

## Commit and tree identity

All strings below are exact UTF-8 bytes. `frame(value)` is an unsigned 64-bit big-endian byte length followed by `value`.

`commit.tree_sha256` is SHA-256 of:

```text
"evoflux-resource-tree-v1\0"
for each desired entry sorted by kind, slug, resource_id:
  frame(kind)
  frame(resource_id UUID text)
  frame(version_id UUID text)
  frame(slug)
  frame(version SemVer text)
  frame(release_channel)
  frame(bundle.artifact_sha256)
  frame(bundle.tree_sha256)
```

`commit.id` is SHA-256 of:

```text
"evoflux-resource-commit-v1\0"
frame(commit.tree_sha256 lowercase hexadecimal text)
```

Cross-language golden vector: for one `skill` entry with resource UUID `00000000-0000-0000-0000-000000000001`, version UUID `00000000-0000-0000-0000-000000000002`, slug `audit`, version `1.2.3`, channel `published`, artifact digest `"a" × 64` and inner tree digest `"b" × 64`, the outer tree is `43a48be42482e92625801c5b1abdf7093128a0a96b3c3e886c73380d76045237` and commit is `7e35c6857cf1f439057ca31d8692f9cc3d29a0e06ba0d7c0d1d2cedc618febdd`.

The Bundle V2 file-tree digest is a separate inner digest defined in [resource-bundle-v2.md](resource-bundle-v2.md). The outer tree proves which versions form the desired checkout; the inner tree proves each extracted bundle.

## Download immutable objects

Fetch every `objects[].href` with the same bearer secret. The response includes:

```http
ETag: "sha256:<artifact_sha256>"
Cache-Control: private, max-age=31536000, immutable
```

`If-None-Match` is supported. The client must still hash received bytes and compare them to `artifact_sha256`; an HTTP cache is not an integrity boundary. Local, S3, Azure Blob and Git are storage implementations behind the same object key and digest contract.

## Required EvoFlux checkout algorithm

1. Register the installation and load its durable managed registry, active commit and object cache.
2. Trigger fetch on `resources.head`, `resources.changed`, `control.resync_required`, reconnect and a bounded periodic fallback.
3. Send the active `have_commit` and all managed `have` entries.
4. Download missing objects with bounded parallelism and retry by digest.
5. Verify artifact size and SHA-256 before safe extraction; reject traversal, symlinks, case collisions and limit violations.
6. Verify every `FileManifestEntry`, the Bundle V2 tree digest, resource semantics, minimum EvoFlux version, dependencies and Plugin trust policy.
7. Construct the complete target tree in a new staging generation by reusing verified unchanged objects, applying changed entries and applying only returned managed tombstones.
8. Recompute the outer tree and commit. They must equal the server response.
9. Validate cross-resource invariants, including Agent team/mode rules, against the complete staged generation.
10. Atomically switch one active-generation pointer. Persist the commit and managed registry only after the switch succeeds.
11. Report inventory after activation. `downloaded` or `staged` is not `applied`.

Never edit `skill-settings.json` or another user-owned runtime overlay. Never delete a local/project/user resource merely because it is absent from the Conductor tree. A tombstone authorizes removal only when the same `resource_id` exists in the durable Conductor-managed registry.

On any failure, leave the active generation and active commit unchanged. Retain a bounded last-known-good generation for rollback. Do not acknowledge inventory or advance the commit after download alone.

## Concurrency and failure semantics

- Conductor builds the response from one stable member-visible head. If the head keeps changing during planning it returns `409`; retry with jitter.
- `401` and `403` suspend delivery. `429`, `409`, `503`, network failures and `5xx` are retryable with bounded exponential backoff.
- Fetch and object download are idempotent. Repeating an identical request returns the same desired commit even if its diagnostic sequence changes later.
- Different members can receive different commit IDs because access rules and release-channel eligibility are resolved before hashing.
- Clients should coalesce repeated SSE invalidations while one fetch/checkout is running, then fetch once more before declaring idle.

## Compatibility and rollout

`GET /api/v1/subscribe/resources`, `GET /api/v1/resources/changes` and hydrated version JSON remain compatibility endpoints. New EvoFlux clients must use smart fetch for Agent, Skill and Plugin delivery. Global MCP configuration is a separate subsystem and must not be installed as a Plugin alias.

Recommended rollout:

1. ship the EvoFlux adapter behind a feature flag and build the managed registry;
2. shadow-fetch and verify without activating;
3. compare computed commits and inventory against Conductor;
4. enable atomic Agent/Skill checkout;
5. enable Plugin staging plus explicit trust/install lifecycle;
6. remove legacy full-snapshot mutation after fleet convergence.

Future performance additions may return short-lived signed CDN URLs, byte ranges, packfiles, Bloom filters or binary deltas. They must preserve the digest, complete-tree and atomic-checkout rules above.
