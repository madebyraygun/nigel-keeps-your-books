---
id: TASK-31.10
title: 'SPA: unlock and settings screens'
status: Done
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-07 12:39'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.4
  - TASK-31.9
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.10-unlock-settings.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Unlock screen for encrypted databases gates the app before any data loads: masked input, attempts-remaining feedback, backoff — parity with the splash-screen flow. Settings screen covers what settings_manager.rs does today: edit business name (metadata company_name), toggle the auto-update check, show the active data directory, and manage the database password (set/change/remove with confirmation, calling new endpoints that wrap the existing password data layer). Include switching data directories (load) with a full app-wide refresh.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An encrypted database shows the unlock screen before any data is fetched, with wrong-password feedback matching the TUI behavior
- [x] #2 Settings screen edits company name, toggles update check, and shows the data directory
- [x] #3 Password set, change, and remove flows work with confirmation
- [x] #4 Switching data directory reloads the whole app cleanly
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented per the approved plan, with the coordinator's seven rulings. Landed as three commits on nigel-31-spa: server API, @nigel/ui components, SPA gate/settings/ScreenContext.

- AppState surgery went in as approved and stayed small: `state.db_path` had only 7 non-test call sites, so swapping `Arc<PathBuf>` for `Arc<RwLock<PathBuf>>` plus a `db_path()` accessor touched almost nothing. `AppState::new` kept its signature, so no test constructor changed.

- 31.7 had landed by the time I started, and it does open connections outside `with_conn` — `routes/imports.rs` has its own `blocking()` helper used by upload, preview and confirm. Per the interface note in my own plan, that helper now takes `&AppState` and holds the `db_gate` read guard. Also picked up `state.data_dir()` as an accessor since imports.rs had its own local `data_dir(&state)` helper doing the same thing.

- Design change forced by review: `post_data_dir` originally committed (settings.json, rebind, password clear, budget reset) and *then* probed encryption and migrated. Both of those are fallible, so a target this build cannot open left the running server stranded on it — and settings.json sent it back there on the next start. Reordered so every fallible step runs first and the commit is the last thing in the block. Regression test `a_target_that_cannot_be_opened_leaves_the_switch_un_made` uses a directory named `nigel.db`; verified it fails against the old ordering (500 from /api/status afterwards) before restoring the fix.

- Second review finding, also fixed: the three password routes used `ApiJson`, whose rejection echoes the deserializer's message — which quotes the offending value. They now decode by hand with a fixed message, the same defense `/api/unlock` already documented. Test asserts a malformed body is never echoed back.

- Frontend bug caught while writing the test rather than after: the update-check toggle reverted by re-rendering the previous value, which lit dirty-checks away — the user had moved the DOM property out from under it, so the switch would have kept showing a state nobody saved. It now resets the control directly, and the test asserts the property rather than the attribute.

- The backoff countdown is the in-flight variant approved as ruling 1: on submit, if the last failure carried a retryAfterMs, the card counts down twice that (the documented ladder doubles, capped at 30s) while the request is out. No client-side cooldown, so the penalty is not charged twice.

- Test hygiene prerequisite worth knowing about: `settings::save_settings` writes `~/.config/nigel/settings.json` through a private `config_dir()`. Any settings test run as-is would have rewritten the developer's real settings and repointed their data directory. Added a `#[cfg(test)]` override in settings.rs plus a `TempConfig` RAII guard in testutil, rather than mutating `$HOME` (process-global, unsafe in newer editions, and `dirs::home_dir` does not consult the environment on every platform).

- ScreenContext landed as the unified ruling asked, deliberately minimal: `{ client, params, navigate }`. All ten stub screens take and ignore it; settings and unlock take `.client` from it. `vitest` now runs `screens/**` under jsdom, and the app's test-setup gained the ElementInternals `validity` shim the ui package already had — without it every Web Awesome form control threw an unhandled rejection on first update.

- wa-input and wa-switch both survive jsdom (spiked before building on them), so no fallback to native inputs was needed.

- Verification, all from the worktree. Web: `npm run lint`, `npm run typecheck`, `npm test` (401 tests: 79 theme + 198 ui + 124 app), `npm run build` — all clean. Cargo, the 8-command matrix with --test-threads=1: fmt clean; clippy --all-targets clean; both reduced-feature clippy runs report exactly the two known task-34 needless_return lints (dashboard.rs:852, report/mod.rs:160) and nothing new; tests 478+25 (default), 334+26 (no-default), 472+25 (no-default+serve); cargo build --release ok.

- Manual smoke, release binary, isolated HOME, real encrypted database, `nigel serve --port 5799 --no-open`: status while locked reports encrypted/locked with companyName null; all four settings routes probed answer 423 while locked; two wrong passwords report attemptsRemaining 2 then 1, the third attempt with the right one returns {"locked":false}; status then shows the company name; app settings read, toggled, and the new value confirmed in settings.json; business name saved with surrounding whitespace trimmed and echoed by status; password changed, then a wrong currentPassword rejected as invalid_password; password removed, status shows encrypted false and a data route still answers 200; data-dir switch answered with the new directory's status and /api/accounts then returned the *second* database's rows (empty) with settings.json rewritten; a switch to a directory with no nigel.db returned 400 with the same message the CLI prints; zero occurrences of either password in the server log; SIGINT printed "Server stopped." and exited cleanly.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds the unlock gate and the settings screen to the web UI, plus the seven server endpoints they need — the last of which no API task owned, so this task carries both halves.

## Server

New `src/server/routes/settings.rs`:

| Route | Method | Body |
|---|---|---|
| `/api/settings/app` | GET / PUT | `updateCheck` (the only web-editable field) |
| `/api/settings/company-name` | PUT | `name` — trimmed, empty clears it, as the TUI does |
| `/api/settings/data-dir` | POST | `path` — answers with the new database's status |
| `/api/settings/password/{set,change,remove}` | POST | wraps `cli/password.rs`'s `&Path` functions |

Two changes to `AppState` make the data-directory switch honest rather than cosmetic:

- **`db_path` moved behind an `RwLock`** so a switch rebinds the *running* server. Rewriting settings.json alone would leave it serving the old books under the new directory's name — a page reload would show stale data with nothing to indicate it. Only 7 non-test call sites; `AppState::new` kept its signature.
- **`db_gate`, a `tokio::sync::RwLock`**, is held for reading wherever a connection is opened and for writing by encrypt, decrypt and the switch. `encrypt_database`/`decrypt_database` finish by renaming the database file and deleting the `-wal`/`-shm` sidecars, which a connection held elsewhere would not survive. 31.7's import routes open their own connections outside `with_conn`, so their `blocking()` helper takes the read guard too.

The switch performs every fallible step — the encryption probe, and migrating an unencrypted target — before committing anything, so a target this build cannot open leaves settings.json, the bound path, the password global and the unlock budget untouched. It then clears the password (an encrypted target must come up locked, not inherit the previous key) and resets the attempt budget, which belongs to a database rather than a process.

**Every settings route sits behind the locked guard**, including the two that never open the database. Nothing on the unlock screen reads app settings, and `password/change`/`password/remove` take the current password in the body — an exemption would make them an unthrottled password oracle reachable without ever passing the gate. A wrong `currentPassword` draws down the same `UnlockGate` budget as a failed unlock, so guessing costs the same either way. Forgotten-password recovery stays `nigel load` plus a restart, which `docs/api.md` now states outright.

## Web

- **The gate replaces the app shell** while the database is locked instead of disabling it — the acceptance criterion made structural. With no shell there is no screen element, so nothing exists that could fetch data before the password arrives. A test asserts the entire call log while locked is ["getStatus"], becoming ["getStatus", "unlock:...", "getStatus"] after unlocking.
- `AppStore` gains a derived `boot` phase (starting / locked / failed / ready). Derived, not stored, so a 423 from any later call returns the app to the gate with no bespoke path.
- Settings covers all four sections `settings_manager.rs` has plus the data-directory switch, which reloads through an injectable seam. After every password change the screen re-reads status rather than trusting a local flag — the same reason the TUI re-probes `is_encrypted` when its password sub-screen closes.
- Three new `@nigel/ui` primitives with previews and axe suites: `wc-unlock-card`, `wc-panel`, `wc-password-form`. The password lives in an input and one event and nowhere else; the confirmation field never leaves the component.
- The backoff countdown runs during the in-flight request. The server serves the delay before answering, so a client-side cooldown would charge the same penalty twice.

## ScreenContext

`ScreenDef.render(ctx)` now receives `{ client, params, navigate }` — the seam 31.11-31.17 build on. Screens take their client from there rather than importing a singleton, which is what lets a test drive a whole screen with `FakeApiClient` and what will let a Tauri client take the same place. Stub screens accept and ignore it. A screen holding state is a custom element in `screens/`; `nigel-settings-screen` is the first, and `web/README.md` records the convention.

## Two bugs caught in review, both fixed with regression tests

1. `post_data_dir` used to commit and then do its fallible work, stranding the server on a database it could not open — and settings.json sent it back there on the next start. Reordered; the new test was verified to fail against the old ordering before the fix was restored.
2. The three password routes decoded through `ApiJson`, whose rejection echoes the deserializer's message, which quotes the offending value. They now decode by hand with a fixed message, the defense `/api/unlock` already documented.

A third, found while writing its test: the update-check toggle reverted by re-rendering the old value, which lit dirty-checks away because the user had moved the DOM property out from under it. It now resets the control directly.

## Tests

18 server tests (round-trips, locked posture across all seven routes, rebinding proved by reading the new database, a switch onto an encrypted target coming up locked, a failed switch changing nothing, no password in any response). 44 new web tests across the three components, the store's boot phases, the shell's gate, and the settings screen.

`settings.rs` gained a `#[cfg(test)]` config-dir override and testutil a `TempConfig` guard: without one, any test reaching `save_settings` rewrites the developer's real `~/.config/nigel/settings.json` and repoints their data directory.

## Verification

Web: lint, typecheck, 401 tests, build — all clean. Cargo (8-command matrix, `--test-threads=1`): fmt clean; `clippy --all-targets` clean; both reduced-feature clippy runs report exactly the two known task-34 `needless_return` lints and nothing new; tests 478+25 / 334+26 / 472+25; `--release` builds. Manual smoke against a real encrypted database in an isolated HOME covered every flow, including all four probed settings routes answering 423 while locked, the data-dir switch reading the second database afterwards, and zero occurrences of either password in the server log.

## Risks and follow-ups

The `db_gate` read guard is a discipline, not a type-level guarantee: a future handler that opens a connection without `with_conn` and without taking the guard would slip through. The requirement is documented on `with_conn`, which is where an author looks, and `routes::imports` is the worked example.
<!-- SECTION:FINAL_SUMMARY:END -->
