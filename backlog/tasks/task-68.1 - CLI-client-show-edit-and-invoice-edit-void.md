---
id: TASK-68.1
title: 'CLI: client show/edit and invoice edit/void'
status: In Progress
assignee:
  - '@opus-team'
created_date: '2026-08-08 00:27'
updated_date: '2026-08-08 00:50'
labels:
  - invoicing
  - cli
dependencies: []
parent_task_id: TASK-68
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The data-layer operations everything else builds on. client show <id> and client edit (name/email/address); invoice edit for draft-only fields (issue, due, currency, line items, plus the notes/terms fields TASK-38 asks for) — published invoices refuse edits the way void refuses send/pay; invoice void <number> with confirmation, making the guarded status reachable. Unknown ids answer with NotFound, not a raw FOREIGN KEY error (TASK-65 pattern).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 client show and client edit exist; email/address changes take effect on the next send
- [ ] #2 invoice edit updates draft invoices only and refuses published/void ones by status
- [ ] #3 invoice void cancels an invoice with confirmation; voided invoices refuse send and pay
- [ ] #4 notes and terms can be set at new and edit, and render on the invoice (absorbs TASK-38)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Spec: docs/superpowers/specs/2026-08-08-task-68-1-invoicing-edit-void-design.md. Plan: docs/superpowers/plans/2026-08-08-task-68-1-invoicing-edit-void.md (12 TDD tasks).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan approved by orchestrator. Rulings on open questions:
1. `voided_at` + migration v5 CONFIRMED — "status is derived, never set by hand" is a stated design constraint; void joins published_at's pattern. Backfill existing void rows.
2. `--clear-due` is IN (Option<Option> plumbing already exists; no-due-date is a documented state). Other --clear-* flags deferred.
3. Void of a partially paid invoice stays refused.
4. Stripe link deactivation on void: separate subtask (to be filed); v1 warning accepted.
5. client edit does not republish sent invoices — republish machinery belongs to TASK-64; a future `invoice republish` rides it.
6. Re-typing ensure_not_void/find_invoice (500 → 409/404) folded in here — 68.6 needs the statuses and this is the natural home.
7. create_invoice date/currency validation kept — small non-additive fix closing a real hole.
Scope boundary vs 68.3: 68.1 delivers notes/terms CLI flags + persistence ONLY. HTML rendering of {{NOTES}}/{{TERMS}} belongs to 68.3's vocabulary expansion (lands later; PDF already shows them in the interim).
<!-- SECTION:NOTES:END -->
