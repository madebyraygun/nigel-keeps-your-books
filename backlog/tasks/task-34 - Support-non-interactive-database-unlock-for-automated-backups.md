---
id: TASK-34
title: Support non-interactive database unlock for automated backups
status: In Progress
assignee: []
created_date: '2026-08-06 18:31'
updated_date: '2026-08-06 18:38'
labels:
  - enhancement
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

The `backup` command cannot run unattended, so encrypted databases have no automated backup path. `nigel backup` already uses SQLite's online-backup API and produces a consistent, encryption-preserving snapshot — the only blocker is that unlocking the database requires a human at a terminal.

## Detail

`prompt_password_if_needed()` (`src/db.rs:419`) calls `rpassword::prompt_password`, which reads from `/dev/tty` rather than stdin. In a context with no controlling terminal (launchd, cron, CI) this fails with `Device not configured (os error 6)`. Because it reads the TTY and not stdin, piping the password in (`echo "$PW" | nigel backup`) does not work either.

There is currently no non-interactive unlock path: searching the source for `env::var`, `keyring`, and `keychain` returns no matches.

**Proposed approach:**

Check a `NIGEL_DB_PASSWORD` environment variable before falling back to the interactive prompt. If the variable is set but wrong, fail immediately with a clear error rather than falling through to a prompt that cannot be answered — a silent fallback would hang an automated job until it times out.

**Relevant code:**
- `src/db.rs` — `prompt_password_if_needed()`, `set_db_password()`, `get_connection()`
- `src/cli/backup.rs` — `run()`; already correct, no change expected
- `src/main.rs` — dispatch calls `prompt_password_if_needed()` before command execution

**Security considerations:**

The password lands in the process environment, readable via `ps -E` by the same user. This is a real tradeoff versus having no backups at all, but the docs should steer users to a secret store (macOS Keychain via `security find-generic-password -w`) rather than a literal in a script or plist. Worth confirming the password is not echoed in any error path or debug output.

## Consumer

The `~/Scripts/backups/k2so/dr-backup.sh` rclone job, which currently file-copies a live `nigel.db` (plus `-wal`/`-shm`) to R2. That copy is not guaranteed to be restorable: it captures whatever the writer was mid-way through. Once this lands, that job can take a real snapshot before syncing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 NIGEL_DB_PASSWORD environment variable unlocks an encrypted database without a TTY
- [x] #2 A wrong password supplied via the environment fails immediately with a clear error and does not fall back to the interactive prompt
- [x] #3 An unset environment variable preserves the existing interactive prompt behaviour, including the 3-attempt retry loop
- [ ] #4 nigel backup completes successfully with no controlling terminal (verified under launchd, not just a shell)
- [x] #5 The password is never echoed to stdout, stderr, or logs on any error path
- [x] #6 All linting checks pass
- [x] #7 Update test coverage
- [x] #8 Create or update documentation, making sure to remove any out of date information
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## What landed

`src/db.rs`
- Added `PASSWORD_ENV_VAR` (`NIGEL_DB_PASSWORD`) and a private `env_password()` helper, consulted by `prompt_password_if_needed()` before the interactive prompt.
- `env_password()` takes the variable's value as a parameter rather than reading the process environment, because cargo runs tests as parallel threads sharing one environment.
- A wrong password from the environment is a hard error. Falling through to the prompt would hang an automated caller until it timed out, which is a worse failure than an explicit one.
- Unencrypted databases short-circuit before the environment is read, so a stale variable cannot lock a user out.

`tests/cli_dispatch.rs` — three integration tests driving the real binary with no controlling terminal: env unlock (asserting the snapshot is encrypted AND reopens with the original password), fail-fast on a wrong password, and no password echoed to stdout/stderr. All carry timeouts, so a regression that reintroduced the prompt fails the suite rather than hanging it.

`README.md` — new "Automated backups" section covering the keychain-sourced invocation and the `ps -E` visibility tradeoff.

`src/cli/backup.rs` unchanged — it already used SQLite's online-backup API correctly.

## Verification

331 tests pass (308 unit + 23 integration, plus 3 new integration and 6 new unit). `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` both clean.

AC#4 is deliberately unchecked: the no-TTY path is verified (the test shell reports `not a tty`, which is what produced the original `Device not configured (os error 6)`), but a true launchd run needs the binary installed and the keychain entry created, neither of which is done yet. Keychain reads can behave differently under launchd if the keychain is locked or the ACL excludes `/usr/bin/security`, so that step needs a real run to confirm.

## Consumer changes

`~/Scripts/backups/k2so/dr-backup.sh` gained a `snapshot_nigel()` step ahead of the rclone sync, sourcing the password from the login keychain (`nigel-db`) and pruning snapshots older than 30 days. It uses an absolute `$HOME/.cargo/bin/nigel` because launchd's PATH omits `~/.cargo/bin`. A snapshot failure logs and continues rather than aborting the home-directory backup.
<!-- SECTION:NOTES:END -->
