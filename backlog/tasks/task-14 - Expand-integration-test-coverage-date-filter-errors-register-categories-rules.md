---
id: TASK-14
title: >-
  Expand integration test coverage: date filter errors, register, categories,
  rules
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - enhancement
  - P2
  - reliability
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/122'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Context

PR #110 added 15 integration tests covering the main CLI dispatch paths. The following additional test cases were identified during review as worthwhile follow-ups:

## Missing Coverage

- **`--from` without `--to` (and vice versa)**: CLAUDE.md documents this as a hard error. A regression test would lock this in.
- **`nigel report register --year N` (text mode)**: One of the main report types, currently only tested via the interactive browser path.
- **`nigel categories list`**: Categories have unit tests but no integration test exercising the CLI command.
- **`nigel rules update` / `nigel rules delete`**: Rule mutation paths are untested at the integration level.

Follow-up from PR #110 review.

---
*Migrated from [GitHub issue #122](https://github.com/madebyraygun/nigel-keeps-your-books/issues/122)*
<!-- SECTION:DESCRIPTION:END -->
