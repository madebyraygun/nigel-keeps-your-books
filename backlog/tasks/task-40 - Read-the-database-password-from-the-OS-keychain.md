---
id: TASK-40
title: Read the database password from the OS keychain
status: To Do
assignee: []
created_date: '2026-08-06 20:30'
labels:
  - enhancement
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Nigel has no native access to an OS secret store. `NIGEL_DB_PASSWORD` lets a scheduled job unlock an encrypted database, but the password must be fetched by a wrapper and handed over through the environment, and the TUI cannot use it at all.

## Detail

Two separate gaps.

**The TUI ignores `NIGEL_DB_PASSWORD` entirely.** Running `nigel` with no subcommand goes to `cli::dashboard::run()` (`src/main.rs:38`), which never reaches `dispatch()` and so never calls `prompt_password_if_needed()`. It unlocks through `splash::run_with_password()` (`src/cli/dashboard.rs:951`) instead, an inline password field that sets the global password directly at `src/cli/splash.rs:208`. The environment variable has no effect on that path. Anyone who exports it for CLI use will still be prompted by the dashboard, which is surprising.

**Even for the CLI, the environment is a workaround, not the destination.** The password has to be materialised into the process environment, where it is readable via `ps -E` by any process running as the same user, and every caller must wrap the invocation in a secret lookup. That is acceptable for an unattended job with no better option; it is poor ergonomics for interactive use.

**Proposed approach:**

Add optional keychain support via the `keyring` crate, which abstracts macOS Keychain, Windows Credential Manager, and Linux Secret Service. Resolution order becomes: `NIGEL_DB_PASSWORD` (unchanged, still authoritative for automation) → keychain entry → interactive prompt. Both the CLI and the TUI would consult the same resolver, closing the split above.

Worth considering as part of the design:
- A `nigel password store` / `forget` subcommand so users never touch `security` directly.
- An opt-in prompt after `nigel password set` offering to save to the keychain.
- On macOS, a keychain read can raise a GUI authorization dialog. Under launchd that blocks invisibly, so the environment variable must remain the documented path for scheduled jobs and the keychain lookup needs a non-interactive failure mode.
- Whether a stored password weakens the threat model the encryption exists for: an attacker with the unlocked login keychain gets the ledger. This should be opt-in, and the docs should say plainly what it trades away.

**Relevant code:**
- `src/db.rs` — `prompt_password_if_needed()`, `env_password()`, `PASSWORD_ENV_VAR`
- `src/main.rs:38` — the dashboard branch that bypasses `dispatch()`
- `src/cli/dashboard.rs:943-955` — TUI pre-flight
- `src/cli/splash.rs:208` — inline password entry
- `src/cli/password.rs` — where store/forget subcommands would live

## Context

Follows the `NIGEL_DB_PASSWORD` work. The consuming backup job currently reads the keychain itself and passes the result through the environment; native support would let it call `nigel backup` directly.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The TUI unlocks an encrypted database without an interactive prompt when a password is available from the keychain
- [ ] #2 CLI and TUI share one password-resolution path, so behaviour cannot drift between them
- [ ] #3 NIGEL_DB_PASSWORD continues to take precedence and remains the documented mechanism for scheduled jobs
- [ ] #4 A keychain lookup that would block on a GUI authorization dialog fails non-interactively instead of hanging
- [ ] #5 Keychain storage is opt-in, and the documentation states what it trades away
- [ ] #6 All linting checks pass
- [ ] #7 Update test coverage
- [ ] #8 Create or update documentation, making sure to remove any out of date information
- [ ] #9 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
