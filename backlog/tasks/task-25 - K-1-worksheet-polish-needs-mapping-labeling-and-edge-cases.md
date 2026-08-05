---
id: TASK-25
title: 'K-1 worksheet polish: needs-mapping labeling and edge cases'
status: To Do
assignee: []
created_date: '2026-08-05 21:18'
updated_date: '2026-08-05 21:51'
labels:
  - reports
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-ups from the K-1 mapping final review (task-23): (1) Needs-mapping note says 'no form_line' but the bucket also holds unrecognized non-NULL values — reword to 'no recognized form_line' and render the Line column so typos self-diagnose; (2) Deductions-by-Line lists meals at gross while the headline Total Deductions is deductible-based — label the column '(gross)' or align; (3) 'nigel categories update N --form-line ""' stores Some("") (surfaces as unmapped) while the TUI coerces cleared input to NULL (income fallback) — trim empty to NULL in the CLI data layer; (4) COGS uses abs() so a net-credit period overstates COGS — consider signed handling.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Needs-mapping section wording and columns make unrecognized form_line values self-diagnosing
- [ ] #2 Meals gross-vs-deductible presentation is labeled or aligned across tables
- [ ] #3 Empty-string form_line from the CLI behaves like the TUI (NULL)
- [ ] #4 A golden-file test renders the complete K-1 worksheet text from a books-shaped fixture DB (mixed mapped/auto-mapped/unmapped/excluded categories, partial payments, meals) and asserts the full output verbatim
<!-- AC:END -->
