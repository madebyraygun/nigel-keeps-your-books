---
id: TASK-68.6
title: 'Web UI: invoicing endpoints and screens at full parity'
status: To Do
assignee: []
created_date: '2026-08-08 00:28'
labels:
  - invoicing
  - web
dependencies: []
parent_task_id: TASK-68
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Supersedes TASK-62 with the full scope: Serialize derives on the invoicing structs (task-31.2 pattern), JSON endpoints behind the standard locked/session guards (clients CRUD, invoices list/detail/edit/void, preview, pay, aging; send with explicit confirmation UX since it touches Stripe/R2/Mailgun), and SPA screens following the manager/report patterns — figure parity with the CLI where both render the same data. sync stays pull-based; the web can expose a "sync now" action but no webhook endpoint.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Invoicing data structs derive Serialize following the task-31.2 pattern
- [ ] #2 JSON API covers clients, invoices, payments, preview, and aging behind the standard guards
- [ ] #3 Send requires explicit confirmation in the UI and reports each step's failure by cause
- [ ] #4 SPA screens cover client management, invoice management, and aging with CLI figure parity
<!-- AC:END -->
