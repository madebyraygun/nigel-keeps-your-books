---
id: TASK-68.1
title: 'CLI: client show/edit and invoice edit/void'
status: To Do
assignee: []
created_date: '2026-08-08 00:27'
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
