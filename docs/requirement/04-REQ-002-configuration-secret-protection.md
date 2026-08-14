# REQ-002 — Configuration secret protection

| | |
|---|---|
| ID | REQ-002 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Draft |
| Priority | P0 |
| Build order | Step 4 of 23 |
| Spec section | [requirements.md section 15](../requirements.md) |
| Source | Code review 2026-08-09 |
| Depends on | REQ-001 if the column is renamed or re-typed |
| Blocks | none |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

The OIDC client secret is stored in plaintext in a column named `client_secret_enc`. The column name
implies encryption at rest. This is more dangerous than storing it plainly under an honest name, because
it misleads the development team into believing a protection exists.

The specification states that API keys and provider credentials must never be uploaded, and that a
dedicated encrypted secret-management system would be a prerequisite for Conductor holding credentials
at all. That posture is undermined if the one credential Conductor already holds is unprotected.

## 2. Requirement

Configuration secrets held by Conductor shall be protected at rest, or the schema shall name them
accurately. A state in which the name claims protection that does not exist shall not be permitted.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Passwords hashed with Argon2 ([password.rs](../../crates/conductor-auth/src/password.rs)) | Any encryption function; searching `crates/` for `encrypt`, `aes` or `cipher` returns zero matches | `setup.rs` writes `sso.client_secret` directly into the `_enc` column with no transformation ([setup.rs:71-85](../../crates/conductor-server/src/http/routes/setup.rs)) |
| Connection tokens hashed with SHA-256 ([secret_token.rs](../../crates/conductor-auth/src/secret_token.rs)) | Key management | `update_sso` writes the same way ([instance.rs:391-409](../../crates/conductor-storage/src/repos/instance.rs)) |
| API responses expose only a `client_secret_set` flag, never the value ([instance.rs:244](../../crates/conductor-storage/src/repos/instance.rs)) | | Column name states a property the value does not have |

The secret must be recoverable in cleartext for the OIDC token exchange
([`sso_runtime()`](../../crates/conductor-storage/src/repos/instance.rs)), so one-way hashing is not
applicable. Symmetric encryption is required.

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | The OIDC client secret is stored encrypted with a symmetric algorithm; reading the column directly does not yield the original value |
| AC-2 | The encryption key is not stored in the database; it is supplied through the environment or the host key store |
| AC-3 | A missing or invalid key at startup produces a clear error; the process does not silently fall back to unencrypted operation |
| AC-4 | The SSO sign-in flow continues to work unchanged after the change |
| AC-5 | The secret never appears in an API response or a log line; only the `client_secret_set` flag is exposed |
| AC-6 | Existing plaintext rows are migrated on first startup after the key is configured |
| AC-7 | If encryption is deferred by decision, the column is renamed to `client_secret` in the same change |

## 5. Out of scope

- Integration with an external secret store such as Vault or a cloud KMS. Reconsider at P2.
- Encrypting other columns. No other column currently holds a recoverable secret.
- Holding LLM provider credentials, which section 10 of the specification places outside scope until a
  dedicated secret-management system exists. See [REQ-023](23-REQ-023-ai-gateway.md).

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Losing the encryption key makes the SSO configuration unrecoverable | Medium | Document key backup; allow the secret to be re-entered through the console |
| 2 | The work is half-completed and the misleading column name survives | High | AC-7 makes the honest rename mandatory in the deferral case |
| 3 | Key handling differs between development and production | Medium | AC-3 forces the failure to be visible rather than silent |

## 7. Open questions

- Is the minimal option acceptable, namely renaming the column now and deferring encryption to P1?
  It is not recommended, but it is much cheaper and it removes the most dangerous property of the current
  state, which is the false impression of safety.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-14 | Rechecked the current SSO storage path; encryption/key-management gaps are unchanged | Codex |
