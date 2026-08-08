---
id: TASK-38
title: 'Invoicing: missing CLI flags for notes/terms and a client show command'
status: Done
assignee: []
created_date: '2026-08-06 19:14'
updated_date: '2026-08-08 01:58'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Absorbed by TASK-68.1 (notes/terms flags, client show) under the invoicing epic (TASK-68).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Delivered by TASK-68.1 / PR #183: invoice new/edit gained --notes/--terms (PDF renders them; HTML page follows in 68.3), and client show exists.
<!-- SECTION:FINAL_SUMMARY:END -->
