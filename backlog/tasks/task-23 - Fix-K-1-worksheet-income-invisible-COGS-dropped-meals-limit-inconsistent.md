---
id: TASK-23
title: 'Fix K-1 worksheet: income invisible, COGS dropped, meals limit inconsistent'
status: To Do
assignee: []
created_date: '2026-08-05 19:14'
labels:
  - reports
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The K-1 prep worksheet (nigel report k1) reports Gross Receipts $0.00 and a large ordinary business loss on real books because k1_prep (src/reports.rs) only counts categories whose form_line is the literal 'Gross receipts'/'Other income' — but the seeded chart of accounts (src/db.rs DEFAULT_CATEGORIES) stores those values in tax_line and leaves form_line NULL for every income category and for Cost of Goods Sold. Verified against live 2025 books: worksheet showed -$352,710.31 loss; books support ~+$2,544 ordinary business income. The fix must be generalizable to any chart of accounts, not just the seeded/Raygun one: unmapped categories must never silently zero out of the worksheet.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 K-1 worksheet gross receipts reflect all income-classified activity for the period on any chart of accounts, whether or not form_line is set
- [ ] #2 Cost of Goods Sold activity appears on the worksheet (1120-S line 2) and reduces ordinary business income
- [ ] #3 Headline Total Deductions applies the 50% meals limit consistently with the Other Deductions sub-table
- [ ] #4 Categories the worksheet cannot map to a form line are surfaced with their totals in a visible validation section instead of being silently excluded
- [ ] #5 Existing seeded databases produce correct worksheets without manual category re-mapping
<!-- AC:END -->
