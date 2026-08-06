---
id: TASK-9
title: Journal entry layer (lightweight double-entry)
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
updated_date: '2026-08-05 23:57'
labels:
  - enhancement
  - architecture
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/81'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Introduce a `journal_entries` table that wraps transactions into balanced debit/credit pairs, giving Nigel a real general ledger underneath its single-entry surface.

## Motivation

This task was originally justified by the invoicing module needing AR tracking. That is no longer the driver: PR #172 ships invoicing with its own `invoices` / `invoice_payments` tables, derived status, and aging buckets, all without a journal layer. Re-scope accordingly.

What a journal layer still buys:

- **Trial balance** (TASK-27) as a direct query over journal lines instead of a derivation from single-entry data that has to be separately proven to tie
- **A real balance sheet**, with equity as an actual account rather than a computed plug
- **Structurally correct equity treatment** — owner distributions and contributions stop living as expense categories, where they currently misreport as deductions
- **Reconciliation between invoice payments and bank transactions** (see below)

## Open design question introduced by PR #172

`invoice_payments` has no `transaction_id`. An invoice payment and the bank deposit representing the same money are two unlinked records. On cash basis this is tolerable — the bank transaction is the source of truth for the P&L — but AR and the P&L can disagree with nothing tying them together.

If this layer generates journal entries from transactions while invoices sit in a parallel table, the result is either double-counted revenue or a permanent unreconciled gap. **Deciding how invoice payments map to bank transactions is a prerequisite for this design**, not an implementation detail.

## Proposed approach

- Promote the chart of accounts: categories become a type of account (revenue, expense), bank accounts become asset accounts, add liability and equity accounts
- Each transaction generates two journal lines (e.g. debit expense, credit bank)
- Existing import/review/categorize workflow unchanged — journal entries generated automatically
- Does NOT require: accrual-basis support, manual journal entries, full GL complexity

## Key constraint

The user-facing experience should remain simple. A freelancer importing a bank CSV should never see the word "debit" unless they go looking for it.

## Sequencing

Depends on PR #172 merging first — that PR rewrites `models.rs`, `migrations.rs`, `cli/mod.rs`, and `main.rs`, all of which this task touches heavily. TASK-27 (trial balance) should land after this.

---
*Migrated from [GitHub issue #81](https://github.com/madebyraygun/nigel-keeps-your-books/issues/81)*
<!-- SECTION:DESCRIPTION:END -->
