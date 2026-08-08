---
id: TASK-68.4
title: 'TUI: client and invoice management screens'
status: Done
assignee:
  - '@opus-team'
created_date: '2026-08-08 00:28'
updated_date: '2026-08-08 04:52'
labels:
  - invoicing
  - tui
dependencies: []
parent_task_id: TASK-68
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Following the manager-screen pattern (account_manager.rs, category_manager.rs): a Clients screen (list, add, edit) and an Invoices screen (scrollable list with status/total/due, detail view with line items and payments, and actions — send with confirmation, record payment form, void with confirmation). Both reachable from the dashboard command chooser with shortcuts. Reuses the CLI data layer from 68.1; send/pay honor the same guards and error wording the CLI prints.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Clients screen lists, adds, and edits clients from the dashboard
- [x] #2 Invoices screen lists and shows invoices with line items, payments, and balance
- [x] #3 Send, record-payment, and void actions work from the invoice screen with confirmations
- [x] #4 Dashboard command chooser gains shortcuts for both screens
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Spec: docs/superpowers/specs/2026-08-08-task-68-4-tui-management-design.md (wireframes S1-S10). Plan: docs/superpowers/plans/2026-08-08-task-68-4-tui-management.md (18 TDD steps).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan approved by orchestrator EXCEPT three points held for user checkpoint (shortcut letter for Clients; Balance column vs wider Client column; whether TUI gets invoice creation or stays CLI-only for drafts).
Rulings on the mechanical questions:
1. Actions bind to the detail view only — confirmed (halves the state matrix; one extra keypress).
2. List shows STORED status (CLI parity) — a display-only overdue derivation would make TUI and CLI disagree; TASK-35 fixes staleness everywhere at once.
3. Client detail view via client_summary: OUT for v1 — edit form suffices; polish later.
4. Payment-amount error may drop the literal `--amount ` prefix — sanctioned adaptation, rest of wording identical.
5. Draw-then-block send with an honest "terminal unresponsive" frame — confirmed; a thread would need a second SQLCipher handle racing mark_published (&Connection is !Sync). Buffered-input drain after send stays.
6. Owns list_invoices (no N+1) + payments data-layer additions; steps 5/11/10-date blocked on 68.1 code, rest independent.

User checkpoint resolved: Clients shortcut = k; Balance column stays (80-col layout as specced); invoice creation stays out of 68.4 — filed as 68.7. 68.4 is now FULLY approved for implementation.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
PR #188 merged (20 commits). Client manager (k: list/add/edit) and invoice manager (n: list with Balance column, detail with line items/payments/pay link, send/pay/void actions on the detail) join the dashboard, following the manager-screen pattern. Send is draw-then-block with an honest Sending frame, buffered-input drain, template pre-flight in CLI order, and errors worded from the reloaded status. Guards are the CLI's own (ensure_voidable pre-flights before the confirm; published-void warning shared via PUBLISHED_VOID_WARNING const). Data layer gained list_invoices (single query) and payments. Review round fixed 5 findings incl. the dropped published-invoice warning and a genuinely wrong 80-col budget (five gaps, not four) now pinned by worst-case row tests. 809+72 / 626+73 green.
<!-- SECTION:FINAL_SUMMARY:END -->
