---
id: TASK-10
title: Remove redundant pre-filter in parse_bofa_card_format
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - P2
  - cleanup
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/112'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Context

After PR #108 changed `parse_amount` to return `Option<f64>`, the character-by-character amount validation in `parse_bofa_card_format` (lines 417-419 of `src/importer.rs`) is largely redundant — `parse_amount` now returns `None` for invalid strings.

The pre-filter is not harmful (defense-in-depth), but it's worth cleaning up for clarity so future maintainers don't wonder why both checks exist.

## Proposal

Remove the manual character validation and rely on `parse_amount` returning `None`. Keep the `continue` on `None`.

Follow-up from PR #108 review.

---
*Migrated from [GitHub issue #112](https://github.com/madebyraygun/nigel-keeps-your-books/issues/112)*
<!-- SECTION:DESCRIPTION:END -->
