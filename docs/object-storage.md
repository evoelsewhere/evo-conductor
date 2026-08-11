# Project object storage

Conductor stores every authored file outside the relational database. This includes mutable resource drafts, immutable Agent/Skill/Plugin releases and uploaded project logos.

## Storage invariant

The `resources.payload` and `resource_versions.payload` columns contain metadata only:

```json
{
  "storage_schema_version": 1,
  "artifact": {
    "key": "sha256/8b/8b…",
    "sha256": "8b…",
    "size": 2741,
    "media_type": "application/vnd.evoflux.resource+zip"
  },
  "files": [
    {
      "path": "SKILL.md",
      "sha256": "2c…",
      "size": 985,
      "media_type": "text/markdown",
      "executable": false
    }
  ],
  "bundle_v2": {}
}
```

No `files[].content`, base64 file, archive or uploaded logo is persisted in SQL. Draft rows also have dedicated `draft_artifact_key`, `draft_content_sha256` and `draft_content_size` columns. Immutable versions use `artifact_key`, `content_sha256` and `content_size`.

Objects use canonical provider-independent keys:

```text
sha256/<first two hex characters>/<64-character SHA-256>
```

Provider prefixes are applied by the storage adapter and never embedded into database keys. This lets a project migrate between providers without rewriting every resource row.

## Backends

### Local filesystem

The default root is `CONDUCTOR_DATA_DIR/objects`. A Project Settings override may be absolute or relative to `CONDUCTOR_DATA_DIR`.

Local storage is suitable for a single Conductor process or a shared filesystem with the durability and consistency guarantees required by every replica. Back up both SQL and the object root.

### Amazon S3 and compatible services

Project Settings stores only bucket, region, endpoint, prefix and addressing mode. Credentials come from the AWS credential chain available to the Conductor process:

- IAM role or workload identity;
- shared AWS configuration;
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and optional session token.

Enable path-style requests for MinIO or another compatible endpoint when required.

### Azure Blob Storage

Project Settings stores only account, container, endpoint and prefix. Authentication uses the Azure credential chain available to the process, including managed/workload identity, account key or SAS environment configuration.

Credential values are never accepted by the Project Settings API and never stored in Conductor SQL.

## Backend change transaction

`PUT /api/settings/storage` is admin-only. A change performs these steps:

1. Build the candidate provider and write/read/delete a health-check object.
2. Pause normal object reads and writes.
3. Load every referenced draft, version and logo object from the active backend.
4. Verify its key against the exact SHA-256 bytes.
5. Copy it to the candidate backend and read it back.
6. Verify the copied digest.
7. Persist the new project setting.
8. Atomically switch the live adapter and resume resource operations.

If any read, write, verification or database update fails, the active backend remains unchanged. Content-addressed objects copied before a failure are safe unreferenced duplicates and may be garbage-collected later.

The response reports `objects_copied` and `bytes_copied`. A backend change with existing objects is rejected when `migrate_existing=false`.

## Legacy migration

At startup, Conductor scans pre-object-store rows for inline `files[].content` or data-URL logos. It writes them to the configured backend, verifies the object, then replaces SQL payloads with manifest-only metadata. Released resources are migrated to immutable ZIP artifacts for all bundle kinds.

Operators should take a database backup before the first upgrade and verify that the configured backend is writable. The migration is idempotent: already externalized rows are skipped and content-addressed writes are safe to repeat.

## Delivery behavior

- Draft APIs hydrate editable file content from the draft ZIP object.
- The version descriptor endpoint hydrates `payload.files` from the immutable object for compatibility.
- The artifact endpoint serves immutable ZIP bytes for Agent, Skill and Plugin.
- EvoFlux verifies `artifact_sha256`, extracts into a staging directory, verifies the file manifest/tree digest and then activates atomically.

The current authoring/import model accepts UTF-8 files. Binary file bytes and Unix executable bits require the binary bundle milestone documented in [resource-bundle-v2.md](resource-bundle-v2.md).
