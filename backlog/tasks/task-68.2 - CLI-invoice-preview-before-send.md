---
id: TASK-68.2
title: 'CLI: invoice preview before send'
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
Today the first render happens inside send, after the Stripe link is created — there is no way to see what the client will receive. invoice preview <number> renders the same HTML (and PDF with the pdf feature) to local files in the data dir or a --output path and prints where they landed, without touching Stripe, R2, or Mailgun. Works for drafts; the Pay button renders as a placeholder when no payment link exists yet.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 invoice preview writes the rendered HTML (and PDF when the feature is on) locally and prints the paths
- [ ] #2 Preview makes no network calls and works with no invoicing config set
- [ ] #3 Previewed output matches what send would publish, modulo the payment link placeholder
<!-- AC:END -->
