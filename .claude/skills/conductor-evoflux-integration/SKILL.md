---
name: conductor-evoflux-integration
description: Use whenever Conductor work touches what EvoFlux downloads, uploads, authenticates with, or writes to disk — resource and document distribution, the sync client, inventory and telemetry reporting, connection tokens, MCP delivery, or a model gateway. Contains the verified EvoFlux-side consumption points with file and line references, the landing paths, and the traps that silently destroy user files. Trigger on "sync to EvoFlux", "client endpoint", "telemetry ingest", "publish an agent", "tích hợp với EvoFlux".
---

# Conductor to EvoFlux integration contract

Conductor distributes configuration and receives controlled telemetry. EvoFlux runs the agents. Getting
this boundary wrong does not produce a compile error — it silently overwrites a user's work or leaks
their source code, so every claim in this file is cited to a real file and line.

**Current state:** searching the entire `evoflux` repository for the string `conductor` across Python,
TypeScript, Rust and Markdown returns **zero matches**. The integration exists only on the server side.
Everything below describes what EvoFlux already does, which is what Conductor must fit into.

Repository paths below are relative to the workspace root that contains both `evo-conductor/` and
`evoflux/`.

## 1. Where things land on a member's machine

EvoFlux resolves its directories in `evoflux/app/core/config.py`. `EVOFLUX_CONFIG_DIR` is the root
(`config.py:227-228`); the rest derive from it unless overridden.

| What Conductor distributes | Lands at | EvoFlux consumer |
|---|---|---|
| `agent` | `{CONFIG_DIR}/agents/*.md` (`config.py:178`) | `app/agent/loader.py` |
| `skill` | `{CONFIG_DIR}/skills/{name}/SKILL.md` (`config.py:183`) | `app/agent/tools/builtin/skill.py:50-71` |
| `command` | `{workspace}/.evoflux/commands/` | `app/services/commands.py:79-80` |
| `mcp` | `{CONFIG_DIR}/mcp.json` (`app/agent/mcp/config.py:47,132-134`) | `app/agent/mcp/manager.py:275` |
| `document` / context | a Conductor-owned directory registered as an extra workspace root | `app/agent/hooks/workspace_instructions.py` |

### Agent file format

An agent is a Markdown file: YAML frontmatter carries configuration, **the body is the system prompt**
(`app/agent/loader.py`). Frontmatter fields include `name`, `role: lead|member`, `description`, `model`,
`thinking_level`, `tools`, `skills`, `fallback_model`. Exactly one file per directory may be
`role: lead`.

This is why `ManagedResource.payload` needs no schema change to carry an agent — it is free-form JSON
stored as `TEXT`, and the Markdown goes in as-is.

### Skill layout and precedence

A skill is a directory containing `SKILL.md` plus optional supporting files
(`skill.py:4-6`). `_iter_skill_roots()` (`skill.py:50-71`) searches project roots before the global
`SKILLS_DIR`, and **earlier entries win on a name collision**. A Conductor-distributed skill written to
the global directory can therefore be shadowed by a project-local one — that is correct behaviour, not a
bug, and the console should not claim otherwise.

`SKILL.md` bodies may contain `{EVOFLUX_CONFIG_DIR}`, `{SKILLS_DIR}` and `{AGENTS_DIR}` tokens which
EvoFlux expands at load time (`skill.py:82-99`). Do not expand them server-side.

## 2. Documents and context files — the trap

EvoFlux already consumes project instruction files; this does not need building.
`WorkspaceInstructionsHook` (`app/agent/hooks/workspace_instructions.py:30-70`) appends the `AGENTS.md`
of every workspace root to the system prompt of **every model call**, loads nested directories on demand,
and blocks a mutating tool call once so the model is forced to read newly applicable rules before
editing.

There are three candidate destinations and **only one is correct**.

**Never write `AGENTS.override.md`.** The name suggests augmentation. The loader returns either the
override file or the standard file, never both:

```python
def _instruction_at(directory: Path) -> Path | None:      # workspace_instructions.py:194-199
    override = directory / "AGENTS.override.md"
    if override.is_file():
        return override
    standard = directory / "AGENTS.md"
    return standard if standard.is_file() else None
```

Writing it silently discards the project's own instructions. Any sync implementation must carry a test
asserting this file is never created (REQ-012 AC-4).

**Never overwrite `AGENTS.md`.** It normally lives inside the repository and is tracked by git.

**Do write into a Conductor-owned directory outside the repository and register it as an extra workspace
root.** The hook merges roots as `[workspace, *extra]`
(`workspace_instructions.py:44-48`), so Conductor content is injected alongside project content, touching
neither git nor the project's own instructions.

Two consequences to state in any design:

- `extra_workspace_paths` is also a **sandbox root** (`app/agent/sandbox.py:120-123`), so agents can read
  that directory. Acceptable for admin-published content, but document it.
- Content is truncated at **128 KB** (`workspace_instructions.py:25`) and at **64 KB** for a
  per-repository `AGENTS.md` (`app/agent/hooks/multi_repo_context.py:16`). Truncation happens silently on
  the member's machine, so Conductor must reject oversized payloads at publish time (REQ-007 AC-7, AC-8).

## 3. MCP is remote code execution by configuration

An MCP server definition contains an executable command:

```python
class StdioServerConfig(BaseModel):        # app/agent/mcp/config.py:63-71
    command: Annotated[str, Field(min_length=1)]
    args: list[str] = Field(default_factory=list)
    env: dict[str, str] = Field(default_factory=dict)
```

`MCPManager` loads `mcp.json` and starts one connection task per enabled server
(`app/agent/mcp/manager.py:275`). Distributing MCP configuration therefore starts a process on every
machine in the project.

A bad prompt degrades answers. A bad MCP definition runs unknown code. These must not share a trust
level even though they share the `resources` table. REQ-010 requires: Admin-only publication, an explicit
warning in the console, and **no automatic activation on the client** — the user confirms, sees the exact
command, args and environment, and is re-prompted when the definition's checksum changes.

Note also that MCP config values support `$VAR` and `${VAR}` expansion from the process environment or
`{CONFIG_DIR}/.env` (`config.py:98-110`). A published MCP definition can therefore reference a local
secret by name without Conductor ever holding it — prefer that over shipping credentials.

## 4. Authentication from the client side

EvoFlux authenticates with a connection token, not a JWT:

```
Authorization: Bearer evc_<prefix>_<secret>
```

The token is generated by `conductor-auth::generate_connection_token`, stored as a SHA-256 hash, and
shown exactly once. Scopes: `subscribe_resources`, `report_telemetry`, `sync_inventory`, and
`read_documents` once REQ-006 adds it.

Client-side storage: the token goes in the **operating system credential store** (REQ-006 AC-9). Do not
put it in `{CONFIG_DIR}/.env` — that file exists and is readable by the MCP secret-expansion path above,
which would expose the project token to any published MCP definition.

Server-side validation order, in every token-authenticated handler:

1. header starts with `evc_`
2. `hash_token` matches a stored `token_hash`
3. not expired, not revoked
4. carries the required scope
5. **owner is still `active`** — missing today, added by REQ-005

## 5. Client endpoint family

Per `docs/requirements.md` section 14:

```
POST /api/v1/client/register     identity, branding, role, policy, privacy config
POST /api/v1/client/heartbeat    liveness; must be safe to call repeatedly
PUT  /api/v1/client/inventory    idempotent per installation
POST /api/v1/telemetry/batch     idempotent per event id
GET  /api/v1/resources
GET  /api/v1/resources/changes?cursor=...
GET  /api/v1/documents
```

**An installation is a first-class entity, not a user.** One member may run EvoFlux on two machines. The
existing `member_inventory` table is keyed by `user_id` alone and cannot express that; REQ-013 replaces
it with `client_installations`.

## 6. Telemetry — what may cross the wire, and what may not

| Collect | Never collect |
|---|---|
| Token counts, tool calls, sessions, durations | Prompt or response content |
| Provider and model identifier | Source code, file content, terminal output |
| Tool name and category, MCP server and tool name | Tool arguments containing project data |
| Resource slug and version | Environment variables, API keys, credentials |
| Mode, EvoFlux version, error category | Absolute local file paths |

`TelemetrySnapshot` (`conductor-domain/src/telemetry.rs`) currently carries counters only, which is the
correct starting point — preserve that as the schema grows. REQ-014 AC-9 requires a schema test asserting
no field can carry content.

Two environment facts shape the design and are easy to miss:

- EvoFlux is local-first and will regularly be **offline**. Events must queue locally and replay. Without
  client-generated event ids and server-side de-duplication, a replay double-counts everything in the
  queue.
- The **client clock is not trustworthy**. Store both `client_reported_at` and a server-assigned
  `server_received_at`, and aggregate on server time.

Workspace identifiers must be normalized, never absolute paths (REQ-013 AC-9).

## 7. Model routing, if a gateway is ever built

EvoFlux allows `base_url` configuration on most providers
(`app/agent/providers/factory.py:169-258`), and `ChatCompletionsOnlyProvider` handles OpenAI-compatible
providers generically. Pointing traffic at an internal gateway therefore requires **no change to the
EvoFlux core**.

This is recorded but **deferred** — `docs/requirements.md` section 10 keeps LLM provider credentials
local to EvoFlux. See REQ-023 for the analysis and the conditions that would reopen it. Do not build
toward it without that requirement being accepted.

The consequence to state honestly elsewhere: client-reported usage can be disabled by the client, so it
is not suitable for chargeback, and a model policy distributed to clients is advisory rather than
enforced (REQ-022 AC-5).

## 8. Rules that override convenience

- Never write inside the user's repository working tree.
- Never overwrite a file the user has edited locally; report the conflict instead (REQ-012 AC-6).
- EvoFlux must work fully when Conductor is unreachable, using the last synchronized content
  (REQ-012 AC-10). The integration is additive; it must never become a startup dependency.
- Deletion is limited to paths previously written by the sync client and recorded in local state, never a
  blanket clean of the managed directory (REQ-012 AC-13).
- A permanent error stops retrying and tells the user; it does not loop (REQ-011 AC-9).
