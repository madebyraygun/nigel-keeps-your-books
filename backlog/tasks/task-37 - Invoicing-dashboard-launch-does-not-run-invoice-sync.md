---
id: TASK-37
title: 'Invoicing: dashboard launch does not run invoice sync'
status: To Do
assignee: []
created_date: '2026-08-06 19:14'
updated_date: '2026-08-07 21:53'
labels:
  - enhancement
  - invoicing
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The best-effort launch sync hook runs for CLI invocations but not for bare `nigel`, which is the dashboard and the most common entry point. Users who live in the dashboard never get automatic payment reconciliation.

`nigel serve` is in the same position: it is excluded from the per-command hook because its database may still be locked at startup (no stdin to prompt on) and server startup must not block on a network poll. A cooldown-based background sync would cover both entry points.

The PR review ledger notes this pairs with adding a cooldown, so launching the dashboard repeatedly does not hit Stripe on every start. The existing 24-hour update-check cooldown in settings.json is the precedent to follow.

Carried over from the review ledger of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Launching the dashboard runs the same best-effort invoice sync as CLI invocations
- [ ] #2 Sync is rate-limited by a persisted cooldown so repeated dashboard launches do not repeatedly call Stripe
- [ ] #3 Sync failure never blocks or delays dashboard startup, matching the existing best-effort behavior
- [ ] #4 The cooldown is configurable or disableable through settings, consistent with update_check
- [ ] #5 nigel serve reconciles Stripe payments under the same cooldown once its database is unlocked, without delaying server startup
<!-- AC:END -->
