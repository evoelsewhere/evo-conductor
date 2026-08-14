# TSK-011-03 — Implement EvoFlux connection service

| | |
|---|---|
| ID | TSK-011-03 |
| Created | 2026-08-10 |
| Updated | 2026-08-14 |
| Status | Implemented — merged; packaged OS/restart smoke remains |
| Layer | EvoFlux |
| Requirement | [REQ-011](../../requirement/12-REQ-011-client-registration.md) |
| Design | [DES-011 sections 3, 5 and 8](../../design/12-DES-011-client-registration.md#8-evoflux-changes) |
| Covers | AC-1, AC-7, AC-8, AC-9, AC-10, AC-12 |
| Depends on | TSK-011-02 |
| Estimate | 2.5d |
| Branch | `feat/REQ-011-client-registration` |

## 1. Goal

Replace temporary resource-subscription enrolment with a robust EvoFlux service that registers once,
persists no secret outside the OS credential store, and maintains a cancellable heartbeat worker.

## 2. Files in scope

| File | Action |
|---|---|
| `../../../../evoflux/app/conductor/models.py` | Modify typed register/heartbeat and connection-state models. |
| `../../../../evoflux/app/conductor/client.py` | Modify HTTP client, redaction and error classification. |
| `../../../../evoflux/app/conductor/service.py` | Modify lifecycle, state, retry, heartbeat and disconnect. |
| `../../../../evoflux/app/core/runtime_settings.py` | Modify non-secret connection configuration/state only. |
| `../../../../evoflux/app/api/routes/settings.py` | Modify status API; never return token value. |
| `../../../../evoflux/tests/conductor/test_client_service.py` | Add service/retry/credential/restart tests. |
| `../../../../evoflux/tests/api/test_settings_routes.py` | Add redaction and disconnect tests. |

## 3. Implementation steps

1. Add credential-store interface backed by the OS secure store and a test fake. Remove any production
   fallback that writes raw token into JSON/configuration.
2. Persist one random installation key and only non-secret server state: installation ID, branding,
   interval and connection state.
3. Implement Connect: validate URL/token shape, call register, then write credential/state only when all
   steps succeed. Report credential-store failures clearly.
4. Implement default 60-second heartbeat, accepting server interval changes and surviving restart without
   blocking FastAPI/EvoFlux startup.
5. Retry transient network/`5xx` failure with capped exponential backoff. Stop on 401/403; clear stale
   server installation ID on 404 and require fresh registration. Redact tokens everywhere.
6. Implement Disconnect as worker cancellation, credential deletion, state clearing and no further HTTP.

## 4. Required tests

| Type | Tool | Must cover |
|---|---|---|
| Unit/integration | `pytest` | Connect, one persisted key, server interval, restart, heartbeat, capped retry, auth failure, stale ID and disconnect. |
| Credential regression | `pytest` | Token absent from settings/status/error/logs; credential-store failure leaves no partial connection. |
| Static | `ruff check`, `ruff format --check`, `ty check` | Every touched app/test file. |

## 5. Commands and reports

```bash
cd ../evoflux
uv run ruff check app/ tests/ && uv run ruff format --check app/ tests/
uv run ty check app/conductor app/api/routes/settings.py
uv run pytest --no-cov -q tests/conductor/test_client_service.py tests/api/test_settings_routes.py
```

## 6. Definition of done

- [x] Token is in OS credential storage only after successful registration.
- [x] Failed/revoked connection cannot block startup or retry forever.
- [ ] Restart preserves schedule without duplicate registration. State/interval persistence is covered; packaged restart smoke remains.
- [x] Disconnect cancels worker before returning success.

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-1 | V1 registration validates and persists safe bootstrap state | `test_registration_uses_v1_contract_without_persisting_token`, `test_service_persists_safe_registration_state_and_disconnects` | Pass |
| AC-7 | Keyring adapter and credential-store failure leave no partial connection | `test_credential_store_failure_leaves_no_partial_connection` | Pass in tests; packaged OS smoke pending |
| AC-8 | Heartbeat uses stored token and persists server interval | `test_heartbeat_uses_stored_token`, `test_heartbeat_persists_server_interval` | Pass |
| AC-9 | Rejected registration/authorization failure is terminal | `test_rejected_registration_is_terminal_and_never_saves_token`, `test_authorization_failure_stops_heartbeat_retry` | Pass |
| AC-10 | Disconnect deletes credential and clears connection state | `test_service_persists_safe_registration_state_and_disconnects` | Pass |
| AC-12 | Default/server interval persists | `test_heartbeat_persists_server_interval` | Partial; packaged restart smoke pending |

### Command output

```text
uv run ruff check app/ tests/                    PASS
uv run pytest --no-cov -q tests/conductor       PASS (25 tests)
focused registration/reconcile/runtime/telemetry suite PASS (37 tests; verified 2026-08-14)
uv run ruff format --check app/ tests/           BASELINE FAIL (31 pre-existing files)
uv run ty check app/                             BASELINE FAIL (25 pre-existing diagnostics)
```

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-10 | Draft planning | Created before design approval at user request |
| 2026-08-10 | In Review | Connection service implemented by `4995fac3`; EvoFlux PR #4 was open |
| 2026-08-14 | Implemented | Current EvoFlux source retains the service/credential tests; packaged OS and restart smoke remain |
