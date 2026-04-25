---
id: TASK-11
title: Stream detect_bofa_checking to match detect_bofa_credit_card pattern
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - P2
  - cleanup
  - importers
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/117'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Context

PR #108 changed `detect_bofa_credit_card` to use `BufReader` streaming instead of reading the entire file. However, `detect_bofa_checking` still reads the full file through `create_csv_reader`. For consistency, it should also use a streaming approach.

Follow-up from PR #108 review.

---
*Migrated from [GitHub issue #117](https://github.com/madebyraygun/nigel-keeps-your-books/issues/117)*
<!-- SECTION:DESCRIPTION:END -->
