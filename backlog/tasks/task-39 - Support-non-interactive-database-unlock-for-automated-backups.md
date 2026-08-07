---
id: TASK-39
title: Support non-interactive database unlock for automated backups
status: Done
assignee: []
created_date: '2026-08-06 18:31'
updated_date: '2026-08-06 22:10'
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
- [x] #4 nigel backup completes successfully with no controlling terminal (verified under launchd, not just a shell)
- [x] #5 The password is never echoed to stdout, stderr, or logs on any error path
- [x] #6 All linting checks pass
- [x] #7 Update test coverage
- [x] #8 Create or update documentation, making sure to remove any out of date information
- [x] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Verification

AC#4 confirmed under a real launchd run on 2026-08-06, not just a no-tty shell. `launchctl kickstart -k gui/$UID/com.dalton.dr-backup` with the password sourced from the login keychain (`security find-generic-password -s nigel-db -w`, ACL granted to `/usr/bin/security` via `-T` at creation):

```
2026-08-06 13:20:58 — SNAPSHOT: ledger saved to /Users/dalton/Documents/nigel/main/backups/nigel-2026-08-06.db (268K)
```

The snapshot is encrypted (header is not the SQLite magic) and matches the live database byte count.

## Review fixes

A review pass found the new unit tests failed under a default `cargo test`: the fixture set the global `DB_PASSWORD` while cargo ran tests as parallel threads, so concurrent tests inherited it. CI passes `--test-threads=1` and stayed green, but CONTRIBUTING.md and CLAUDE.md both document bare `cargo test`. Fixtures now use `open_connection()` with an explicit password.

Also fixed: a non-UTF-8 value silently fell through to the prompt via `std::env::var().ok()` (now `var_os` with an explicit error); three distinct failures shared one message that named the wrong cause (empty value, and wrong-password-vs-damaged-file, are now distinguished and name the database path); and the leak test passed vacuously on a killed child.

Verified across `cargo test` (default parallel, 5x for the race), `--test-threads=1`, and `--no-default-features`; clippy and fmt clean.
<!-- SECTION:NOTES:END -->
