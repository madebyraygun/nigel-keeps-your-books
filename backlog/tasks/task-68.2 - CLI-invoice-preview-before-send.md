---
id: TASK-68.2
title: 'CLI: invoice preview before send'
status: In Progress
assignee:
  - '@opus-team'
created_date: '2026-08-08 00:27'
updated_date: '2026-08-08 00:48'
labels:
  - invoicing
  - cli
dependencies: []
parent_task_id: TASK-68
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Today the first render happens inside send, after the Stripe link is created — there is no way to see what the client will receive. invoice preview <number> renders the same HTML (and PDF with the pdf feature) to local files in the data dir or a --output path and prints where they landed, without touching Stripe, R2, or Mailgun. Works for drafts; the Pay button renders as a placeholder when no payment link exists yet.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 invoice preview writes the rendered HTML (and PDF when the feature is on) locally and prints the paths
- [ ] #2 Preview makes no network calls and works with no invoicing config set
- [ ] #3 Previewed output matches what send would publish, modulo the payment link placeholder
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Spec: docs/superpowers/specs/2026-08-08-task-68-2-invoice-preview-design.md. Plan: docs/superpowers/plans/2026-08-08-task-68-2-invoice-preview.md (5 TDD tasks).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan approved by orchestrator. Rulings on open questions:
1. `--output-dir` accepted over the task's literal `--output` — matches report grammar; preview is always two artifacts.
2. `--open` deferred — would promote the `open` dep out of the serve feature; documented escape hatch instead.
3. Void invoices: warn-and-render confirmed (preview is read-only; refusal is for world-changing commands). Pay button Omitted even when a URL is stored.
4. `render_invoice` stays `pub` — codebase convention for data/render layer functions, and 68.6's preview endpoint consumes it.
5. `(from_email not configured)` placeholder wording accepted — it names the setting to fix.
Key structural points for implementers: new src/invoicing/render.rs seam owns both cfg halves of PDF rendering; PayButton::{Link,Placeholder,Omitted} replaces Option<&str>; InvoiceCommands::Preview joins the sync_invoice_payments skip list in main.rs.
<!-- SECTION:NOTES:END -->
