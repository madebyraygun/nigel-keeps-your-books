---
id: TASK-31
title: 'Epic: Web UI (localhost) — nigel serve'
status: Done
assignee: []
created_date: '2026-08-06 16:24'
updated_date: '2026-08-07 17:20'
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
- [x] #1 Running nigel serve opens a browser UI covering dashboard, register browse with editing, review, import, all reports, accounts/categories/rules managers, reconcile, undo, settings, and encrypted-DB unlock
- [x] #2 Server binds 127.0.0.1 only and rejects requests without a valid session token or with a non-localhost Host header
- [x] #3 Ships as a single binary with the SPA embedded; TUI and CLI behavior are unchanged
- [x] #4 README and CLAUDE.md document serve mode
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Epic complete: nigel serve delivers the full Nigel experience in a browser from the single existing binary.

Delivered across 17 subtasks: lib+bin crate split; camelCase serde across the data layer; an axum server (default-on serve feature) with session-token auth, Host/Origin DNS-rebinding guards, and graceful shutdown; encrypted-DB unlock flow with backoff and a fail-closed locked guard; 21+ JSON API endpoint groups covering every report, list, mutation, the import pipeline (upload/preview/confirm), and PDF/text export downloads byte-identical to CLI exports; and a three-package Lit SPA (@nigel/theme tokens derived from the TUI palette, @nigel/ui component library with preview harness and axe coverage on every state, @nigel/app) embedded via rust-embed, covering dashboard, register with inline editing, review, import, all eight reports with print stylesheet and exports, accounts/categories/rules managers, reconcile, undo, settings, and the unlock gate.

Verification: 520 cargo tests + 1391 web tests green across the full feature matrix; live walkthrough of all endpoints (200s), security posture (401 no-cookie, 403 bad Host/Origin), and the embedded bundle containing every screen. Docs: docs/api.md (full endpoint inventory), CLAUDE.md, README.md, web/README.md.

Follow-ups filed on main: tasks 34, 41, 42, 43, 47, 48, 49. Manual punch list: print-preview check per web/README.md and a real-browser pass over the preview harness (npm run preview).
<!-- SECTION:FINAL_SUMMARY:END -->
