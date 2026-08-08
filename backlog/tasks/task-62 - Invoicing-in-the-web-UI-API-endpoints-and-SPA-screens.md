---
id: TASK-62
title: 'Invoicing in the web UI: API endpoints and SPA screens'
status: To Do
assignee: []
created_date: '2026-08-07 21:53'
updated_date: '2026-08-08 00:28'
labels:
  - invoicing
  - web
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
PR #172 ships invoicing as CLI-only; the web UI (nigel serve, PR #182) has no invoicing endpoints or screens, so invoices, clients, and A/R aging are invisible in the browser. The gap is deliberate for the merge, but the web UI should reach parity with the CLI surface: list/show invoices, client list, and the aging report at minimum, with send/pay as a follow-on decision (they touch external services and may warrant confirmation UX).

Prerequisite: the invoicing data-layer structs in src/invoicing/ and models.rs need serde Serialize derives, following the task-31.2 pattern the rest of the data layer received for the JSON API.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Invoicing data-layer structs derive Serialize following the existing task-31.2 pattern
- [ ] #2 JSON API exposes invoice list, invoice detail, client list, and A/R aging behind the standard locked/session guards
- [ ] #3 SPA screens render invoices, clients, and aging with figure parity against the CLI output
- [ ] #4 Send/pay actions are either implemented with explicit confirmation UX or explicitly deferred to a follow-up task
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Superseded by TASK-68.6, which carries the full web-parity scope of the invoicing epic (TASK-68).
<!-- SECTION:NOTES:END -->
