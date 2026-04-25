---
id: TASK-1
title: Invoicing add-on
status: To Do
assignee: []
created_date: '2026-04-25 18:05'
labels: []
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/24'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview
Add the ability to create, send, and track invoices. This is the natural trigger for introducing journal entry pairs (lightweight double-entry) since invoicing creates the "billed but not yet paid" state that single-entry can't represent.

**Scope:**
- Create invoices with line items, due dates, client info
- Track invoice status (draft → sent → paid → overdue)
- Match incoming bank transactions to open invoices
- AR aging report
- PDF invoice generation (extends existing PDF export)

**Architectural note:** This should ship alongside a journal entry layer. Each invoice creates a journal entry (debit AR, credit revenue). When payment is matched, a second entry clears AR (debit cash, credit AR). Existing bank-import workflows continue unchanged — the journal entries are generated behind the scenes. See discussion in [link to double-entry design doc when created].

---
*Migrated from [GitHub issue #24](https://github.com/madebyraygun/nigel-keeps-your-books/issues/24)*
<!-- SECTION:DESCRIPTION:END -->
