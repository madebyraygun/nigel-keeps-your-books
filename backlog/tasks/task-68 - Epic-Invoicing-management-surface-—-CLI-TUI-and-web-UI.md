---
id: TASK-68
title: 'Epic: Invoicing management surface — CLI, TUI, and web UI'
status: In Progress
assignee:
  - '@claude-orchestrator'
created_date: '2026-08-08 00:27'
updated_date: '2026-08-08 00:41'
labels:
  - epic
  - invoicing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
PR #172 shipped invoicing as a create-and-send pipeline: client add/list, invoice new/list/show/send/sync/pay/aging/import. What's missing is the management layer around it — nothing can be edited or cancelled after creation (the void status exists in the data model with send/pay guards, but no command sets it, so a mistaken draft is permanent), there is no way to see an invoice before it goes to a client, the HTML/PDF templates are compiled into the binary, the TUI has no invoicing screens, and the web UI has no invoicing endpoints or screens.

This epic completes the surface in three layers: CLI first (data-layer operations every other front end reuses), then TUI screens following the manager-screen pattern, then web UI parity following the task-31 API/SPA conventions.

Folds in TASK-38 (notes/terms flags, client show) and TASK-62 (web UI invoicing), which predate it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Clients and draft invoices can be edited, invoices can be voided, and a client or invoice is inspectable from CLI, TUI, and web
- [ ] #2 An invoice can be previewed (HTML and PDF) before anything is published or emailed
- [ ] #3 Invoice HTML styling is customizable without rebuilding the binary
- [ ] #4 TUI has screens for client management, invoice management, and the aging report
- [ ] #5 Web UI reaches feature parity with the CLI invoicing surface
<!-- AC:END -->
