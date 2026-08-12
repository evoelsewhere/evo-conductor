# EvoFlux fleet simulator

`tools/fleet-simulator.ts` creates a deterministic EvoFlux-like fleet against
one Conductor through public HTTP APIs only. Its default workload is 1,000
active members and one installation per member.

The simulator covers the full control/data path:

1. complete a fresh local setup or log in as the configured admin;
2. create or reuse published Agent, Skill and Plugin fixtures;
3. create/reuse members, activate them, and issue scoped connection secrets;
4. idempotently register installations and heartbeat them;
5. negotiate a Git-style smart fetch plan with the client's current commit and object `have` set;
6. download only missing Agent, Skill and Plugin artifacts, verify SHA-256, confirm the new commit is up to date and report current inventory;
7. emit privacy-safe request/model/tool telemetry attributed to immutable Agent,
   Skill and Plugin versions;
8. replay batches to prove duplicate handling and submit sampled invalid batches
   to prove rejection paths;
9. submit the legacy resource-usage contract with accepted, duplicate and
   rejected events; and
10. query analytics and write a machine-readable invariant/performance report.

No prompts, completions, source code or local paths are sent. Names, IDs,
models, token counts, timings and outcomes are deterministic synthetic data.

## Local run

Start an isolated Conductor:

```bash
CONDUCTOR_DATABASE_URL='sqlite:data/fleet.db?mode=rwc' \
CONDUCTOR_HOST=127.0.0.1 \
CONDUCTOR_PORT=4700 \
cargo run -p conductor-server --bin evo-conductor
```

In another terminal, run the 1,000-member flow:

```bash
FLEET_ADMIN_PASSWORD='LocalFleetOnly!2026' \
bun run tools/fleet-simulator.ts \
  --base-url http://127.0.0.1:4700 \
  --members 1000 \
  --requests-per-member 3 \
  --concurrency 24 \
  --summary-file .fleet-simulator/summary-1000.json
```

The default admin email is `fleet-admin@fleet.invalid`. On an already configured
Conductor, pass its real local admin email/password using `FLEET_ADMIN_EMAIL`
and `FLEET_ADMIN_PASSWORD`. The password is not written to state or reports.
Local automation may instead supply an existing administrator session through
`FLEET_ADMIN_TOKEN`; the token is used only in memory and is never included in
the summary.

Create a Vietnamese 120-member demo with one month of chart data:

```bash
FLEET_ADMIN_EMAIL='admin@example.com' \
FLEET_ADMIN_PASSWORD='your-local-admin-password' \
bun run tools/fleet-simulator.ts \
  --base-url http://127.0.0.1:4700 \
  --members 120 \
  --requests-per-member 12 \
  --concurrency 12 \
  --seed demo-vn-120-v1 \
  --member-prefix demo-vn \
  --email-domain evokit.demo \
  --member-name-style vietnamese \
  --history-days 30 \
  --summary-file .fleet-simulator/summary-demo-vn-120-v1.json
```

Use a new seed for another independent dataset. Reusing the same seed and state
file resumes safely and exercises duplicate-event handling.

Run a quick smoke workload with:

```bash
bun run tools/fleet-simulator.ts --members 10 --concurrency 4
```

Show every option:

```bash
bun run tools/fleet-simulator.ts --help
```

## Safety and resuming

The tool refuses any hostname except `localhost`, `127.0.0.1`, `0.0.0.0` or
`::1` unless `--allow-non-local` is explicitly supplied. Only use that flag for
an environment where load testing and synthetic member creation are authorized.

Raw one-time connection secrets and installation IDs are stored in
`.fleet-simulator/` with mode `0600`; that directory is gitignored. The stable
seed, member email scheme, registration keys, installation keys and event IDs
make reruns resumable and idempotent. A rerun reuses state, registration calls
and event IDs, so accepted events become server-counted duplicates rather than
new usage. If a stored secret is revoked, the simulator issues a replacement.

To start a genuinely new synthetic fleet, choose a new `--seed` or an explicit
new `--state-file`. Do not delete state during a partially completed run unless
you accept that replacement secrets may be issued.

## Output and interpretation

Progress is written to stderr. Stdout is one JSON summary containing:

- accepted/duplicate/rejected inventory and telemetry counters;
- aggregate Conductor analytics totals;
- pass/fail end-to-end invariants;
- member and telemetry throughput; and
- per-endpoint request counts, retry/failure counts and p50/p95/p99/max latency.

Exit code `0` means all invariants passed, `2` means the flow completed but one
or more invariants failed, and `1` means the run aborted.

## Focused tests

```bash
bun test tools/fleet-simulator.test.ts
```

The tests cover deterministic identifiers, workload bounds, URL normalization,
remote-target refusal and latency aggregation. The simulator itself is the
public-HTTP end-to-end test; run it against a disposable local database before
using large counts in CI.

## Reference 1,000-member run

The fresh local SQLite run on 2026-08-12 used 24 concurrent member flows and
three request triplets per member. It completed in **463.56 seconds** with all
9 end-to-end invariants passing and no retry, unexpected error or HTTP failure:

| Signal | Result |
|---|---:|
| Fresh members / installations | 1,000 / 1,000 |
| Legacy version payloads / artifacts verified | 3,000 / 1,000 |
| Telemetry accepted / duplicate replay | 9,000 / 9,000 |
| Expected invalid telemetry rejected | 20 / 20 |
| Member flows / telemetry events per second | 2.16 / 19.41 |
| Change feed p50 / p95 / p99 | 91 / 190 / 299 ms |
| Telemetry batch p50 / p95 / p99 | 296 / 655 / 1,148 ms |
| Inventory p50 / p95 / p99 | 55 / 153 / 274 ms |

The report is written to `.fleet-simulator/summary-1000-v2.json` and stays
gitignored because it includes local state paths and run-specific identifiers.

This run exposed two real concurrency defects before passing: Argon2 work was
blocking async runtime workers, and SQLite deferred telemetry transactions could
surface `SQLITE_BUSY` during concurrent writes. Password hashing/verification now
runs on the blocking pool; file-backed SQLite uses WAL plus a per-connection
30-second busy timeout; telemetry duplicate detection is one atomic insert with
conflict handling. A dedicated 24-writer regression test covers that boundary.

After ingestion, the complete portfolio analytics endpoint returned 3,210
requests / 9,630 resource uses in **835 ms**, and the Skill-scoped view returned
3,210 requests / 3,210 uses in **866 ms** on the same development machine.
Independent report panels execute concurrently behind the bounded database pool.
