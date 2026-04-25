---
id: TASK-9
title: Journal entry layer (lightweight double-entry)
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - enhancement
  - architecture
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/81'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Introduce a `journal_entries` table that wraps transactions into balanced debit/credit pairs. This supports the invoicing module's AR tracking without requiring users to understand double-entry bookkeeping.

## Proposed approach
- Promote the chart of accounts: categories become a type of account (revenue, expense), bank accounts become asset accounts, add liability accounts (AR, AP)
- Each transaction generates two journal lines (e.g., debit expense, credit bank)
- Existing import/review/categorize workflow unchanged — journal entries generated automatically
- Enables: trial balance, proper balance sheet, AR/AP tracking
- Does NOT require: accrual-basis support, manual journal entries, full GL complexity

## Key constraint
The user-facing experience should remain simple. A freelancer importing a bank CSV should never see the word "debit" unless they go looking for it.

---
*Migrated from [GitHub issue #81](https://github.com/madebyraygun/nigel-keeps-your-books/issues/81)*
<!-- SECTION:DESCRIPTION:END -->
