---
id: TASK-64
title: 'Invoicing: republish HTML and PDF when payments change an invoice'
status: To Do
assignee: []
created_date: '2026-08-07 23:09'
labels:
  - invoicing
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The published R2 page and PDF are rendered once at send time and never touched again. After a payment lands (via invoice pay or Stripe sync), the public page still shows the original balance, so a client following their link sees an unpaid invoice they already settled.

When a payment is recorded against a published invoice, Nigel should re-render and re-upload i/{token}/index.html and invoice.pdf so the page reflects paid amount, balance, and status. Needs the R2 config at pay/sync time; should be best-effort like the launch sync (a failed republish must not fail the payment recording).

Found during pre-merge testing of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Recording a manual payment against a published invoice re-renders and re-uploads the HTML and PDF
- [ ] #2 Payments recorded by invoice sync trigger the same republish
- [ ] #3 A failed republish leaves the payment recorded and reports a notice rather than an error
<!-- AC:END -->
