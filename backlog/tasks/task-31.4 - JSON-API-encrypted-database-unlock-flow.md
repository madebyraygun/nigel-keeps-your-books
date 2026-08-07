---
id: TASK-31.4
title: 'JSON API: encrypted database unlock flow'
status: Done
assignee:
  - '@agent-31.4'
created_date: '2026-08-06 16:25'
updated_date: '2026-08-06 20:33'
labels:
  - web
  - backend
dependencies:
  - TASK-31.3
references:
  - src/db.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.4-unlock.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Web equivalent of the splash-screen password gate. GET /api/status reports initialized/encrypted/locked state plus company name; POST /api/unlock validates the password (db::validate_password) and sets it for the server process (set_db_password). The server starts locked for encrypted databases and every data endpoint returns a distinct locked status until unlocked. Mirror the TUI three-attempt behavior with delay/backoff on failures.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 GET /api/status reports initialized, encrypted, and locked states
- [x] #2 POST /api/unlock with a valid password unlocks the process; invalid attempts get attempts-remaining feedback and backoff
- [x] #3 Data endpoints refuse with a distinct locked status until unlock; unencrypted databases skip the flow entirely
- [x] #4 The password is never logged or persisted to disk
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## 1. Files

- NEW `src/server/secret.rs` — `Secret(String)` newtype: `#[serde(transparent)] Deserialize`, hand-written `Debug` printing `<redacted>`, `Drop` calling `zeroize()` (matches splash.rs / password_manager.rs precedent), `fn expose(&self) -> &str`. `pub(crate)` so 31.10 (password management over HTTP) reuses it.
- NEW `src/server/routes/status.rs` — `GET /api/status`, `POST /api/unlock`, and the `locked_guard` middleware. One domain: server lock state.
- EDIT `src/server/state.rs` — add `UnlockGate` (failure counter + backoff) and `AppState.unlock`, plus `AppState::is_locked()`.
- EDIT `src/server/error.rs` — add `ApiErrorCode::InvalidPassword` -> 401 / `"invalid_password"` (the spec asks for this code; 31.3 did not ship it).
- EDIT `src/server/routes/mod.rs` — `api_router(state: &AppState)`, mount status/unlock ungated, mount `data_router()` behind the guard.
- EDIT `src/server/mod.rs` — `build_router` passes `&state` to `api_router`; new router tests.
- EDIT `docs/api.md`, `CLAUDE.md`, `README.md`.

## 2. GET /api/status

`{"initialized": bool, "encrypted": bool, "locked": bool, "companyName": string|null, "version": string, "dataDir": string}` — `#[serde(rename_all = "camelCase")]`, `companyName` is `Option<String>` serialized as null (NOT skipped, the SPA wants a stable shape).

- `initialized` = `state.db_path.exists()`. (Discrepancy noted below: main.rs already refuses to start serve on a missing DB, so false is only reachable if the file disappears at runtime. Field kept because the spec and SPA contract name it.)
- `encrypted` = `db::is_encrypted(&state.db_path)?` (false for a missing file, by that function's contract).
- `locked` = `encrypted && db::get_db_password().is_none()`.
- `companyName` = null when `!initialized || locked` (reading metadata needs the key); otherwise `db::get_metadata(&conn, "company_name")` inside `spawn_blocking` with a fresh `db::get_connection`. A connection failure is a 500.
- `version` = `env!("CARGO_PKG_VERSION")`.
- `dataDir` = `state.db_path.parent()` display string — derived from the path the server actually opened, NOT `settings::get_data_dir()`, so it cannot disagree with it after a `nigel load` in another process.

Mounted OUTSIDE the locked guard, INSIDE the session guard (it is under /api, so the cookie is still required). Works in every state including uninitialized.

## 3. POST /api/unlock

Request `{"password": "..."}` -> `UnlockRequest { password: Secret }`.

Body extraction: take `Result<Json<UnlockRequest>, JsonRejection>` and map every rejection to a FIXED message — `ApiError::bad_request("Expected a JSON body with a \"password\" string.")`. Deliberately not `rejection.body_text()`: serde type-mismatch messages can echo the supplied value, and the envelope contract also forbids axum's default plain-text rejection body.

Order of checks:
1. `db::is_encrypted(&db_path)?` false -> `400 bad_request` "This database is not encrypted — no password is needed." (no attempt counted).
2. Already unlocked (`db::get_db_password().is_some()`) -> idempotent `200 {"locked": false}` without re-validating. Rationale: the process is already unlocked; re-validating would let a wrong password produce a confusing failure for a state that is not actually locked.
3. Otherwise one `spawn_blocking` job does all the blocking work:
   - `db::validate_password(&db_path, pw)?`
   - `Ok(false)` -> failure path (below).
   - `Err(e)` -> I/O / other DB error, NOT a wrong password.
   - `Ok(true)` -> `db::set_db_password(Some(pw.to_string()))`, then `db::get_connection(&db_path)` + `db::init_db(&conn)` — this runs the migrations 31.3 deferred for encrypted databases. If either fails, `db::set_db_password(None)` first (never leave the process half-unlocked) and return the error.
4. Success -> `state.unlock.reset()`, respond `200 {"locked": false}`.

Error mapping:
- wrong password -> `401` code `invalid_password`, `details: {"attemptsRemaining": n, "retryAfterMs": ms}`.
- `validate_password`/`init_db` `Err` -> `500 internal` with a FIXED message ("Couldn't open the database to check the password.") and the underlying error text DISCARDED. This is not error-swallowing (the request still fails loudly with 500); it is required because `rusqlite::Error::SqlInputError`'s Display is `"{msg} in {sql} at offset {offset}"` and `open_connection` sets the key via `pragma_update(None, "key", pw)`, which rusqlite renders as literal SQL — so the password can appear inside a DB error string. Verified in rusqlite-0.31.0 `src/error.rs:65-70` and `src/pragma.rs:263-270`. One short code comment explains this (a security constraint, not an edit justification).
- `JoinError` from `spawn_blocking` -> `500 internal`, fixed message.

## 4. Backoff

In `state.rs`, no persistence, process-wide:

```
const MAX_FREE_ATTEMPTS: u32 = 3;
const BACKOFF_BASE: Duration = 1s;
const BACKOFF_CAP: Duration = 30s;

fn backoff_delay(failures: u32) -> Duration   // failures = count AFTER this failure
    failures <= 2 -> 0
    failures >= 3 -> min(BACKOFF_BASE * 2^(failures - 3), BACKOFF_CAP)
    // 0, 0, 1s, 2s, 4s, 8s, 16s, 30s, 30s, ... (saturating shift, no overflow)

struct UnlockGate { failures: std::sync::Mutex<u32> }
    record_failure(&self) -> (attempts_remaining: u32, delay: Duration)
        // attempts_remaining = MAX_FREE_ATTEMPTS.saturating_sub(failures)
    reset(&self)
```

Handler: call `record_failure()`, let the `MutexGuard` drop INSIDE that method (never hold a std mutex across an await — clippy `await_holding_lock`), then `tokio::time::sleep(delay).await` before building the response. tokio's `time` feature is already enabled. Never `std::thread::sleep` — that would block a runtime worker.

`retryAfterMs` = the delay applied before THIS response (0, 0, 1000, 2000, 4000, ... capped 30000). Documented in docs/api.md as "how long the server held this response back"; the client may retry immediately because the penalty has already been served. (Open decision — alternative reading is "the delay the next failure will incur"; see below.)

`attemptsRemaining` saturates at 0 and there is NO hard lockout — the spec is explicit that the desk user can always restart the process; past 3 failures every attempt just gets slower.

## 5. Locked guard and the mount point for 31.5-31.8

`AppState::is_locked(&self) -> Result<bool, ApiError>` = `db::is_encrypted(&self.db_path)` (mapped to 500 on I/O error) `&& db::get_db_password().is_none()`, short-circuiting on the password check first so the common post-unlock path does no file I/O at all. Deliberately a live probe rather than a value cached at startup: 31.10 can change the encryption state at runtime, and a stale cache there would either lock out a good session or expose a locked DB. The probe is a 16-byte read of an already-hot file; it only runs while the process is locked or the DB is plaintext.

Guard is axum middleware, applied with `route_layer` so it covers only matched data routes and not the `/api` JSON 404 fallback:

```
pub fn api_router(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/ping", get(ping))
        .merge(status::routes())                       // /status, /unlock — never gated
        .merge(data_router().route_layer(
            middleware::from_fn_with_state(state.clone(), status::locked_guard)))
        .fallback(api_not_found)
}

/// Every route that reads or writes the database. 31.5+ add their routes here
/// and inherit the locked guard.
fn data_router() -> Router<AppState> { Router::new() /* + a #[cfg(test)] probe route */ }
```

`api_router` therefore gains a `&AppState` parameter (state is needed for `from_fn_with_state`, mirroring how `mod.rs` already applies `auth::session_guard`); `build_router` is updated. `data_router()` is empty in this task apart from a `#[cfg(test)]` `/_guarded_probe` route, so the guard tests exercise the REAL `api_router` wiring that 31.5 will inherit rather than a parallel test-only assembly.

Guard behavior: locked -> `ApiError::locked()` (423, code `locked`, already built by 31.3); otherwise `next.run(req)`.

## 6. Where unlock state lives

The password itself stays exactly where it is: the process-global `db::DB_PASSWORD` mutex, set via `db::set_db_password`. `AppState` gains only the failure counter (`unlock: Arc<UnlockGate>`) and never holds, copies, or Debug-prints a password. Module docs on `routes/status.rs` state the consequence: unlock is process-wide, so `nigel serve` assumes a single database per process — the same assumption every CLI subcommand already makes.

## 7. Keeping the password out of logs and Debug

- `Secret` has a hand-written `Debug` -> `Secret(<redacted>)`; `UnlockRequest` derives `Debug` and inherits the redaction.
- `Secret` zeroizes on drop.
- `AppState`/`UnlockGate` `Debug` contain no password material.
- No `eprintln!`/`println!` on the unlock path at all (migration progress lines from `run_migrations` carry no secrets).
- 500s on the unlock path carry fixed strings, never a `NigelError`/rusqlite Display (see section 3).
- Accepted limitation, documented in docs/api.md: axum's buffered request body holds the plaintext password until it is dropped, and the copy handed to `set_db_password` lives for the process lifetime — identical to the CLI/TUI path.

## 8. Tests

New unit tests (`state.rs`, `secret.rs`, `routes/status.rs`) and router tests (`server/mod.rs`), all in-crate, driven through `tower::ServiceExt::oneshot` like 31.3's. Shared helpers in the `mod.rs` test module: build a `TempDir`, `db::open_connection(path, None)` + `db::init_db`, optional `cli::password::encrypt_database(&path, "hunter2")`, then an app + session cookie.

Every test that touches the DB password global calls `db::set_db_password(None)` on entry and exit, with the standard `--test-threads=1` comment (see `db.rs:477`, `cli::password::tests`); CI already runs all three test steps with `-- --test-threads=1`.

1. `backoff_delay`: 0, 0, 1s, 2s, 4s, 8s, 16s, 30s, 30s at failures 1..=9, and no overflow at u32::MAX.
2. `UnlockGate::record_failure` returns attemptsRemaining 2, 1, 0, 0 and the matching delays; `reset` restores the sequence.
3. `Secret`/`UnlockRequest` `Debug` output contains "redacted" and does NOT contain the password.
4. status, unencrypted tempdir DB with `company_name` metadata set: initialized true, encrypted false, locked false, companyName echoed, version = `CARGO_PKG_VERSION`, dataDir = tempdir.
5. status, missing DB file: initialized false, encrypted false, locked false, companyName null.
6. status, encrypted DB, not unlocked: initialized true, encrypted true, locked true, companyName null (proves it does not need the key).
7. Guarded probe route on an encrypted locked DB -> 423, code `locked`; `/api/ping`, `/api/status`, `/api/unlock` still reachable in the same state.
8. Guarded probe route on an unencrypted DB -> 200 (no gate at all).
9. Unlock wrong password x3 on an encrypted DB: 401 each, code `invalid_password`, details.attemptsRemaining 2 -> 1 -> 0, details.retryAfterMs 0 -> 0 -> 1000; the third response's measured wall clock is >= ~900ms (the only slow test, ~1s) proving the sleep is real; the DB stays locked.
10. Unlock correct password: 200 `{"locked": false}`, status flips to locked false, and the previously 423 probe route now returns 200.
11. Counter reset: two failures, one success, `db::set_db_password(None)` to simulate a re-lock, one more failure -> attemptsRemaining is 2 again.
12. Unlock on an unencrypted DB -> 400 `bad_request`, no attempt recorded.
13. Unlock with a malformed/incomplete JSON body -> 400 `bad_request` in the standard envelope, message does not echo the body.
14. Migrations run on unlock: build a DB, force `metadata.schema_version` to "1", encrypt it, assert the version is still 1 before unlock, POST the correct password, reopen with `open_connection(path, Some(pw))` and assert `migrations::get_schema_version == LATEST_VERSION`.
15. Unlock is idempotent while unlocked: second POST with the same password -> 200.
16. `error.rs`: extend the existing code/status table test with `(InvalidPassword, 401, "invalid_password")`.

## 9. Docs

- `docs/api.md`: `invalid_password` row in the code table; a "Locked state" subsection (server starts locked for an encrypted DB; `/api/ping`, `/api/status`, `/api/unlock` work while locked, everything else is 423; unlock runs the deferred migrations); `GET /api/status` and `POST /api/unlock` endpoint entries with request/response/failure JSON, the backoff table, and the `retryAfterMs` semantics; the note that the password is runtime-only.
- `CLAUDE.md`: extend the server bullet with `routes/status.rs`, the locked guard, and the `src/server/secret.rs` entry in Project Structure; add design constraints for the locked flow (guard + backoff + migrations-on-unlock + process-wide unlock).
- `README.md`: one line under the serve/Features material — an encrypted database prompts for its password in the browser.

## 10. Build order (TDD)

1. `error.rs` `InvalidPassword` + table test.
2. `secret.rs` + its tests.
3. `state.rs` `UnlockGate`/`backoff_delay`/`is_locked` + unit tests.
4. `routes/status.rs` status handler + status tests.
5. `routes/status.rs` unlock handler + unlock/backoff tests.
6. `locked_guard` + `data_router` wiring + guard tests.
7. Migration-on-unlock test.
8. Docs, then the full verification sweep.

## 11. Verification

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo clippy --no-default-features --all-targets -- -D warnings` (expect ONLY the two pre-existing task-34 `needless_return` lints at dashboard.rs:852 and report/mod.rs:160)
4. `cargo clippy --no-default-features --features serve --all-targets -- -D warnings` (same two, nothing new)
5. `cargo test -- --test-threads=1`
6. `cargo test --no-default-features -- --test-threads=1`
7. `cargo test --no-default-features --features serve -- --test-threads=1`
8. `cargo build --release`

Plus a manual smoke against a real encrypted database in a temp data dir (`NIGEL` settings pointed at it), `nigel serve --port 0 --no-open`: `/api/status` (encrypted true / locked true / companyName null), a wrong-password unlock (401 invalid_password, attemptsRemaining), three wrong passwords (visible ~1s pause on the third), a data-route 423, the correct password (200), `/api/status` again (locked false, companyName present), and Ctrl-C for a clean exit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented per the approved plan. One design change forced by axum: the plan mounted the locked guard on data_router() with route_layer, but axum 0.8 panics at startup with "Adding a route_layer before any routes is a no-op" because data_router() has no routes yet outside cfg(test). Caught by the manual smoke, not the tests (the cfg(test) probe route hid it). The guard is now layered over the whole /api router and exempts /ping, /status, /unlock by path (UNGATED_PATHS). This is fail-closed: a route added anywhere under /api is guarded unless explicitly named, so 31.5+ cannot expose a locked database by mounting in the wrong place. data_router() survives as the documented merge point for per-domain routers.
- Consequence documented in docs/api.md and CLAUDE.md: while locked, an unknown /api path answers 423 rather than 404 (the guard wraps the fallback). Covered by a test.
- retryAfterMs is the delay already applied to that response, per the approved ruling; unlock is idempotent while unlocked; dataDir comes from db_path.parent(); no password trimming.
- Migration-on-unlock test builds a genuine pre-v2 database (raw db::SCHEMA + schema_version=1). An earlier version forced schema_version back to 1 on a fully migrated DB, which made migration v2 re-run CREATE TABLE csv_profiles and fail — the test was wrong, not the code.

- Verification (all 8): cargo fmt --check clean; cargo clippy --all-targets -D warnings clean; cargo test 351+25 pass; cargo test --no-default-features 300+26 pass; cargo test --no-default-features --features serve 344+25 pass; cargo build --release ok. clippy --no-default-features (with and without serve) reports exactly the two pre-existing task-34 needless_return lints at dashboard.rs:852 and report/mod.rs:160, nothing new. All test runs use --test-threads=1 (the DB password global).
- Manual smoke, release binary, isolated HOME with an encrypted database (nigel init --data-dir, then nigel password set through a pty): /auth 302 + cookie; status while locked -> {"initialized":true,"encrypted":true,"locked":true,"companyName":null,...}; /api/accounts (unknown, guarded) -> 423 locked; /api/ping -> 200 while locked; three wrong passwords -> 401 invalid_password with attemptsRemaining 2, 1, 0 and retryAfterMs 0, 0, 1000, the third taking 1.10s wall clock; malformed body -> 400 bad_request with no echo; correct password -> 200 {"locked":false}; status -> locked false; /api/accounts -> 404 (gate open, route genuinely absent); second unlock -> 200 (idempotent); no cookie -> 401; server log contains zero occurrences of the password; SIGINT printed "Server stopped." and exited cleanly.
- Smoke DB had no company_name set, so companyName was null in both states there; the populated case is covered by the unit tests (echoed when unlocked, null while locked with a name actually present).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds the web unlock flow for encrypted databases: `GET /api/status` reports server and database state in every condition, and `POST /api/unlock` trades a password for a working session, running the schema migrations that `nigel serve` has to defer on a database it cannot open. Until then every other `/api` route answers 423 `locked`.

## Changes

- **`src/server/routes/status.rs`** (new): `GET /api/status` -> `{initialized, encrypted, locked, companyName, version, dataDir}`, with `companyName` null while locked or uninitialized because reading it needs the key, and `dataDir` taken from the database the server actually opened rather than settings.json. `POST /api/unlock` validates with `db::validate_password`, adopts the key with `db::set_db_password`, then runs `init_db`; if migration fails it re-locks the process rather than leaving it half-unlocked. Unlocking while already unlocked is an idempotent 200 — it is not a password checker — and unlocking an unencrypted database is a 400.
- **Locked guard**: layered over the whole `/api` router, exempting `/ping`, `/status`, and `/unlock` by name. Fail-closed by design: a route added anywhere under `/api` is guarded unless explicitly named, so later tasks cannot expose a locked database by mounting in the wrong place. `data_router()` remains the documented merge point for per-domain routers. The guard probes `is_encrypted` per request instead of caching it at startup, so runtime password changes cannot desynchronise it.
- **Backoff** (`state.rs`): an in-memory `UnlockGate` counts failures. `attemptsRemaining` runs 3 down to 0 with no hard lockout — whoever is at the keyboard can restart the process — and from the third failure the server holds the response back 1s, 2s, 4s… capped at 30s via `tokio::time::sleep`, never a thread sleep. `retryAfterMs` reports the delay already applied to that response, so the client may retry at once. Success resets the counter; nothing is persisted.
- **`src/server/secret.rs`** (new): `Secret` wraps password strings with a redacted `Debug` and zeroize-on-drop, matching what the TUI already does for its input buffers. Errors from opening the database on the unlock path are replaced with a fixed message, because rusqlite renders `PRAGMA key = '<password>'` as literal SQL and prints it back inside `SqlInputError`. Malformed request bodies get a fixed 400 rather than serde's text, which quotes the offending value.
- **`src/server/error.rs`**: new `invalid_password` code (401) carrying `{attemptsRemaining, retryAfterMs}` in `details`.

## User impact

`nigel serve` now works against an encrypted database: the browser asks for the password instead of the terminal. The password is held in memory for the life of the process and never written to disk or logged. Unencrypted databases are unaffected and never see the gate.

## Tests

14 new tests. Router tests drive the real app against tempdir databases in each state — unencrypted, missing, encrypted-locked, encrypted-unlocked — covering the status shape in each, 423 on guarded and unknown paths while locked, the attempts countdown with a wall-clock assertion that the third failure really is delayed, counter reset after success, idempotent re-unlock, 400 on an unencrypted database and on a malformed body, and migrations running on unlock against a genuine pre-v2 database. Unit tests cover the backoff curve including overflow, the gate bookkeeping, `Secret` redaction, and the new error code.

Green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and all three feature combos (351+25, 300+26, 344+25), plus `cargo build --release`. Reduced-feature clippy still reports only the two pre-existing task-34 `needless_return` lints.

## Risks / follow-ups

While locked, an unknown `/api` path answers 423 before 404 — the guard wraps the fallback. Documented and tested. A route that should answer while locked must be added to `UNGATED_PATHS`.
<!-- SECTION:FINAL_SUMMARY:END -->
