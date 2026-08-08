---
id: TASK-68.9
title: 'Invoicing: deactivate the Stripe payment link when an invoice is voided'
status: To Do
assignee: []
created_date: '2026-08-08 01:02'
labels:
  - invoicing
dependencies: []
parent_task_id: TASK-68
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Deferred out of 68.1 by design review: void does not touch Stripe or R2, and sync_all skips void invoices, so a client paying a voided invoice through a still-live payment link goes unrecorded — 68.1 ships a warning on void. This task adds a PaymentGateway method to deactivate a payment link (Stripe supports active=false), calls it from void best-effort (a Stripe failure must not block the void; print the link so the operator can kill it by hand), and decides whether the published R2 page should be replaced with a "voided" page or removed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Voiding an invoice with a payment link deactivates the link in Stripe, best-effort
- [ ] #2 A Stripe failure leaves the invoice voided and prints the link for manual cleanup
- [ ] #3 The fate of the published R2 page on void is decided and implemented or explicitly documented
<!-- AC:END -->
