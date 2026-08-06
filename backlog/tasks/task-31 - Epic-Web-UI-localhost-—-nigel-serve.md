---
id: TASK-31
title: 'Epic: Web UI (localhost) — nigel serve'
status: To Do
assignee: []
created_date: '2026-08-06 16:24'
updated_date: '2026-08-06 18:22'
labels:
  - epic
  - web
dependencies: []
documentation:
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Serve the full Nigel experience in a browser from the existing single binary. A new 'nigel serve' subcommand runs an axum HTTP server bound to 127.0.0.1, exposing a JSON API that wraps the existing data layer (reports.rs, reviewer.rs, cli/accounts.rs, cli/categories.rs, importer.rs, categorizer.rs, reconciler.rs — all already take &Connection and return plain structs), plus an embedded SPA reusing components and patterns from boxcraft-app. All SPA-to-server communication goes through one thin api client module (fetch backend) so the same frontend can later run inside Tauri (invoke backend) and against a remote shared server (multiuser). The TUI dashboard remains the default experience; serve is additive. Feasibility notes: data layer is already connection-per-call with WAL + busy_timeout, snapshots use the SQLite online backup API, and the SQLCipher unlock flow (is_encrypted/validate_password/set_db_password) maps directly to a web unlock screen.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Running nigel serve opens a browser UI covering dashboard, register browse with editing, review, import, all reports, accounts/categories/rules managers, reconcile, undo, settings, and encrypted-DB unlock
- [ ] #2 Server binds 127.0.0.1 only and rejects requests without a valid session token or with a non-localhost Host header
- [ ] #3 Ships as a single binary with the SPA embedded; TUI and CLI behavior are unchanged
- [ ] #4 README and CLAUDE.md document serve mode
<!-- AC:END -->
