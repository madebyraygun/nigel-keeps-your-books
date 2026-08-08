---
id: TASK-68.6
title: 'Web UI: invoicing endpoints and screens at full parity'
status: In Progress
assignee:
  - '@opus-team'
created_date: '2026-08-08 00:28'
updated_date: '2026-08-08 06:11'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Spec: docs/superpowers/specs/2026-08-08-task-68-6-web-invoicing-design.md. Plan: docs/superpowers/plans/2026-08-08-task-68-6-web-invoicing.md (5 stages, each its own PR).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan approved by orchestrator. Rulings on open questions:
1. Blocking send CONFIRMED — the invoice row is the job record; bounded reqwest timeouts + SendStep trace + wire-level {"confirm":true} are the price, paid in the plan.
2. Nav stays FLAT — the registry is flat today; grouping is a shell redesign that belongs to its own task if 14 screens ever feels crowded.
3. Aging lives in #/reports (it is a report; matches 68.5 making it a ReportKind), with a link from the invoices screen.
4. Invoice detail is a FULL VIEW (query param), not a drawer — consistent with how reports/register handle depth; detail carries line items, payments, actions.
5. Preview iframe COLLAPSED by default — one click to open; keeps detail fast.
6. "from Raygun" subject: already ruled into 68.3 ({{COMPANY}}); 68.6 inherits, no separate fix.
7. delete_client stays in 68.6 stage 3 as planned — it is the only consumer (68.4's TUI has no client delete).
8. /api/status omits the `invoicing` block while LOCKED — do not advertise configured integrations pre-unlock.
502/UpstreamFailed and token skip_serializing are accepted as specced.

Stage 1 merged (PR #189): Serialize derives with token skip_serializing, payment_amount+void guards in the data layer, wire-shaped list_invoices, formatters extracted and routed through fmt::money (the one intentional CLI output change). Stage-2 notes: ClientSummary needs serde(flatten) at the route wrapper; publicUrl computed at the route; OPEN_STATUSES/PAYMENT_METHODS consts are the legal-set source; clientName is Option. Orphaned-invoice detail deferred to stage 3 with delete_client.
<!-- SECTION:NOTES:END -->
