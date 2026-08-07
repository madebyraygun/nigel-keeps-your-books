---
id: TASK-38
title: 'Invoicing: missing CLI flags for notes/terms and a client show command'
status: To Do
assignee: []
created_date: '2026-08-06 19:14'
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
The `invoices` table has `notes` and `terms` columns and the HTML/PDF templates render them, but there is no CLI flag to populate either, so they are unreachable from the command line. There is also no `client show` to inspect a single client, unlike `invoice show`.

Carried over from the review ledger of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `invoice new` accepts --notes and --terms and persists both
- [ ] #2 Values set via those flags appear in the rendered HTML and PDF invoice
- [ ] #3 `client show <id>` displays a single client's details, mirroring the shape of `invoice show`
- [ ] #4 CLAUDE.md and docs/invoicing.md list the new flags and command
<!-- AC:END -->
