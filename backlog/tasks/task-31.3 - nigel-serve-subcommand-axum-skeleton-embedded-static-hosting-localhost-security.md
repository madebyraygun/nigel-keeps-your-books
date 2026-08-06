---
id: TASK-31.3
title: >-
  nigel serve subcommand: axum skeleton, embedded static hosting, localhost
  security
status: To Do
assignee: []
created_date: '2026-08-06 16:25'
labels:
  - web
  - backend
dependencies:
  - TASK-31.1
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add axum + tokio and a serve subcommand: bind 127.0.0.1 with a --port flag, serve embedded SPA assets (rust-embed) at /, mount an /api router, and run the same pre-flight as the dashboard (init check, migrations). Localhost is not a trust boundary by itself: generate a per-session auth token, open the browser with a tokenized URL that sets a cookie, validate the Host header to block DNS-rebinding, and reject cross-origin requests. Graceful shutdown on Ctrl-C.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel serve starts on 127.0.0.1 with --port and --no-open flags working, and prints or opens a tokenized URL
- [ ] #2 Requests without a valid session are rejected with 401; non-localhost Host headers are rejected with 403
- [ ] #3 Static SPA assets are served from the binary with no filesystem dependency, and /api routes are mounted
- [ ] #4 Ctrl-C shuts the server down cleanly
- [ ] #5 Builds pass with and without the pdf/gusto features
<!-- AC:END -->
