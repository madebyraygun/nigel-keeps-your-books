---
id: TASK-31.2
title: Derive serde Serialize on data-layer structs
status: To Do
assignee: []
created_date: '2026-08-06 16:25'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - backend
dependencies: []
references:
  - src/reports.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.2-serde.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Report and domain structs (PnlReport, ExpenseBreakdown, TaxSummary, CashflowReport, RegisterReport, BalanceReport, K1PrepReport, FlaggedTxn, ImportResult, Account, CategoryRow, rule rows, etc.) need Serialize — and Deserialize where they are inputs — so axum handlers can return them as JSON without a parallel DTO layer. serde is already a dependency; this is mechanical but touches many structs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All structs returned by reports.rs, reviewer.rs, the accounts/categories/rules data layers, and importer.rs derive Serialize
- [ ] #2 JSON field casing is consistent across the API (single documented choice, e.g. serde rename_all)
- [ ] #3 A serialization smoke test exists for at least one report struct
- [ ] #4 cargo test passes with and without default features
<!-- AC:END -->
