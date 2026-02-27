---
# nigel-c96t
title: Double-entry accounting plugin
status: draft
type: feature
created_at: 2026-02-27T01:30:19Z
updated_at: 2026-02-27T01:30:19Z
blocked_by:
    - nigel-to27
---

Add a plugin that extends Nigel's single-entry cash-basis system with double-entry bookkeeping support.

## Motivation

Single-entry works for simple consultancy books, but double-entry provides:
- Proper debit/credit balancing for every transaction
- Balance sheet accounts (assets, liabilities, equity)
- Accrual-basis accounting support
- Journal entries with enforced balance constraints
- Trial balance and general ledger reports

## Scope

- [ ] Define a chart of accounts schema with account types (asset, liability, equity, revenue, expense)
- [ ] Implement journal entry model (each entry has balanced debit/credit legs)
- [ ] Add ledger storage (new tables or extended schema via plugin hooks)
- [ ] Auto-generate double-entry legs from existing single-entry transactions
- [ ] Add reports: trial balance, general ledger, balance sheet
- [ ] Ensure backward compatibility — single-entry workflow remains the default

## Design Considerations

- Should hook into the plugin system (nigel-to27) once available
- Existing single-entry transactions should be viewable through a double-entry lens (auto-generated contra entries)
- Must not break existing cash-basis reports or workflows
- Consider whether this replaces or supplements the current accounting model
