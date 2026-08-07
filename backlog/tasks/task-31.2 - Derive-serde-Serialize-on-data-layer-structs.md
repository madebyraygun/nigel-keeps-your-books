---
id: TASK-31.2
title: Derive serde Serialize on data-layer structs
status: Done
assignee:
  - '@agent-31.2'
created_date: '2026-08-06 16:25'
updated_date: '2026-08-06 19:09'
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
- [x] #1 All structs returned by reports.rs, reviewer.rs, the accounts/categories/rules data layers, and importer.rs derive Serialize
- [x] #2 JSON field casing is consistent across the API (single documented choice, e.g. serde rename_all)
- [x] #3 A serialization smoke test exists for at least one report struct
- [x] #4 cargo test passes with and without default features
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Scope

Add `serde::Serialize` (plus `Deserialize` on the one input struct) with the
epic-wide `#[serde(rename_all = "camelCase")]` to every data-layer struct the
HTTP API will return. Rust field names are untouched; no DTO layer; no
Cargo.toml change (serde 1 with `derive` and serde_json are already normal
dependencies, so unit tests can use `serde_json::to_value`).

Every struct below gets `#[derive(Serialize)]` merged into its existing derive
list (most currently have none) and `#[serde(rename_all = "camelCase")]`
directly beneath it. The attribute is applied uniformly even where every field
is single-word (e.g. `PnlItem`), so the convention is visible and stays correct
when fields are added.

## Exact inventory

### src/reports.rs — Serialize only
- `PnlItem`, `PnlReport` (`totalIncome`, `totalExpenses`, `net`)
- `ExpenseItem`, `VendorItem`, `ExpenseBreakdown` (`topVendors`)
- `TaxItem` (`taxLine`, `categoryType`), `TaxSummary` (`lineItems`)
- `CashflowMonth` (`runningBalance`), `CashflowReport`
- `RegisterRow` (`categoryId`, `accountName`, `isFlagged`), `RegisterReport`
- `FlaggedTransaction` (`accountName`)
- `AccountBalance` (`accountType`), `BalanceReport` (`ytdNetIncome`)
- `K1LineItem` (`formLine`, `categoryName`), `K1OtherDeduction`,
  `K1Validation` (`uncategorizedCount`, `officerComp`, `distributions`,
  `compDistRatio`), `K1PrepReport` (`grossReceipts`, `cogs`, `grossProfit`,
  `otherIncome`, `totalDeductions`, `ordinaryBusinessIncome`,
  `deductionLines`, `scheduleKItems`, `otherDeductions`,
  `otherDeductionsTotal`, `autoMapped`, `unmapped`, `validation`)
- `DateGranularity` — `#[derive(Serialize)]` + `rename_all = "camelCase"`.
  On a unit-variant enum `rename_all` renames the *variants*, so it serializes
  as the bare strings `"monthAndYear"`, `"yearOnly"`, `"none"` — exactly the
  wrapper vocabulary 31.5 specifies (`{ granularity, report }`). No custom impl
  needed. Also adding `Debug` to its derive list (it has none today, which is
  why the existing `report_kind_slugs_and_granularity` test uses `assert!(a ==
  b)` instead of `assert_eq!`); this is a one-word nicety — say the word and I
  drop it.

### src/reviewer.rs
- `FlaggedTxn` (`accountName`) — Serialize; keeps its existing `Debug`.
  Also the payload of `get_transaction_by_id`, which 31.6 returns.
- `CategoryChoice` (`categoryType`) — Serialize.

### src/importer.rs
- `ImportResult` (`duplicateFile`) — Serialize.
- `GenericCsvConfig` (`dateCol`, `descCol`, `amountCol`, `dateFormat`) —
  **Serialize + Deserialize**; it is a request input (31.7) and also the shape
  `/api/csv-profiles` returns.
- `ImporterKind` gets nothing (spec): it has a `key()` slug and a
  `#[cfg(feature = "gusto")]` variant; a derive there would be a casing trap.

### src/categorizer.rs
- `CategorizeResult` (`stillFlagged`) — Serialize.

### src/reconciler.rs
- `ReconcileResult` (`isReconciled`, `statementBalance`, `calculatedBalance`,
  `discrepancy`) — Serialize.

### src/models.rs — Serialize on all six
- `Account` (`accountType`, `lastFour`), `Category` (`categoryType`,
  `parentId`, `taxLine`, `formLine`, `isActive`), `Transaction` (`accountId`,
  `categoryId`, `isFlagged`, `flagReason`, `importId`), `Rule` (`categoryId`,
  `matchType`, `hitCount`, `isActive`), `ImportRecord` (`accountId`,
  `recordCount`, `dateRangeStart`, `dateRangeEnd`, `checksum`), `ParsedRow`
  (appears in `ImportResult.sample`). Existing `#[allow(dead_code)]` and
  `Debug, Clone` derives stay.

### src/cli/categories.rs, src/cli/undo.rs
- `CategoryRow` (`categoryType`, `taxLine`, `formLine`) — Serialize.
- `LastImport` (`importId`, `accountName`, `importDate`,
  `transactionCount`) — Serialize.
- `cli/accounts.rs` needs no change: `list_accounts` returns `models::Account`.

### Untouched, deliberately
- `settings.rs::Settings` — already serde, on-disk `settings.json` snake_case
  casing must not change (31.10 wraps it in its own response struct).
- `reports.rs::ReportKind`, `K1Mapping`, `cli/update.rs::UpdateInfo`, clap
  structs in `cli/mod.rs`, and all TUI state structs.

## Feature-gate check (AC #4)

Surveyed every `#[cfg(feature = ...)]` in the crate: gusto gates only functions
and one `ImporterKind` variant; pdf gates only `cli/export.rs` fns and
`src/pdf.rs`. No struct in the inventory is feature-gated, so the derives are
identical in the default and `--no-default-features` builds.

## Tests (AC #3)

New cases in the existing `#[cfg(test)] mod tests` blocks — all pure
construction + `serde_json::to_value`, no DB needed:

1. `reports.rs::pnl_report_serializes_camel_case` — build a populated
   `PnlReport` (both vecs non-empty) and assert the top-level keys are exactly
   `income`, `expenses`, `totalIncome`, `totalExpenses`, `net`, that no
   snake_case key survives, and that a nested `PnlItem` has `name`/`total`.
2. `reports.rs::k1_prep_report_serializes_camel_case` — the Option-heavy,
   deeply nested case: `K1PrepReport` with non-empty `deduction_lines`,
   `schedule_k_items`, `other_deductions`, `unmapped`, `auto_mapped`, and a
   `K1Validation` with `comp_dist_ratio: Some(2.0)`; assert `grossReceipts`,
   `deductionLines[0].formLine`, `otherDeductionsTotal`,
   `validation.compDistRatio`, plus one `None` case serializing as JSON null.
3. `reports.rs::date_granularity_serializes_camel_case` — asserts the three
   variants emit `"monthAndYear"` / `"yearOnly"` / `"none"`, pinning the
   vocabulary 31.5 depends on.
4. `importer.rs::generic_csv_config_round_trips_camel_case` — deserialize
   `{"dateCol":0,"descCol":1,"amountCol":3,"dateFormat":"%m/%d/%Y"}` and
   re-serialize, asserting the value round-trips (covers the Deserialize side).

## Verification

- `cargo fmt --check`
- `cargo clippy -- -D warnings` (CI form)
- `cargo clippy --no-default-features -- -D warnings`
- `cargo test -- --test-threads=1` (CI form)
- `cargo test --no-default-features -- --test-threads=1` (CI form, AC #4)

## Docs

No CLI/module/settings surface changes, so CLAUDE.md and README.md need no
edit; the JSON casing convention is already documented in the epic spec and
`docs/api.md` is created by 31.5.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented the approved plan: Serialize + rename_all="camelCase" on 18 report structs and DateGranularity (reports.rs), FlaggedTxn/CategoryChoice (reviewer.rs), CategorizeResult, ReconcileResult, all six models.rs structs, CategoryRow, LastImport, ImportResult, and Serialize+Deserialize on GenericCsvConfig.
- Per coordinator decision: added Debug to DateGranularity and switched the existing granularity assertion to assert_eq!. Derived nothing on ReportKind (its K1 slug is "k1-prep", which a camelCase derive would render "k1").
- Added 4 serialization tests: pnl_report_serializes_camel_case, k1_prep_report_serializes_camel_case, date_granularity_serializes_camel_case (reports.rs), generic_csv_config_round_trips_camel_case (importer.rs).
- Pre-existing finding: cargo clippy --no-default-features reports 2 needless_return warnings (cli/dashboard.rs:852, cli/report/mod.rs:160) in the no-pdf paths. Verified against a clean HEAD — identical 2 warnings, unrelated to this task and not a regression. CI only runs the default-feature clippy, which is clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Derives `serde::Serialize` (plus `Deserialize` on the one request-input struct) across the data layer so axum handlers in 31.3+ can return domain types as JSON directly, with no parallel DTO layer.

**Casing decision (AC #2):** every API-visible struct carries `#[serde(rename_all = "camelCase")]` — the epic-wide choice from `2026-08-06-epic-31-architecture.md`. Rust field names are untouched; the attribute is applied uniformly (even on all-single-word structs like `PnlItem`) so the convention stays correct as fields are added. 33 derive sites in total.

**Changes:**
- `reports.rs` — Serialize on all 18 report structs (`PnlItem`/`PnlReport`, `ExpenseItem`/`VendorItem`/`ExpenseBreakdown`, `TaxItem`/`TaxSummary`, `CashflowMonth`/`CashflowReport`, `RegisterRow`/`RegisterReport`, `FlaggedTransaction`, `AccountBalance`/`BalanceReport`, `K1LineItem`/`K1OtherDeduction`/`K1Validation`/`K1PrepReport`) plus `DateGranularity`, which serializes as `"monthAndYear"` / `"yearOnly"` / `"none"` — the exact vocabulary 31.5's `{ granularity, report }` wrapper needs. Added `Debug` to `DateGranularity` and tightened the existing granularity assertion to `assert_eq!`.
- `reviewer.rs` — `FlaggedTxn` (also the payload of `get_transaction_by_id`, which 31.6 returns), `CategoryChoice`.
- `importer.rs` — `ImportResult`; `GenericCsvConfig` gains **Serialize + Deserialize** as a request input for 31.7 and the `/api/csv-profiles` shape.
- `models.rs` — all six structs (`Account`, `Category`, `Transaction`, `Rule`, `ImportRecord`, `ParsedRow`).
- `categorizer.rs` `CategorizeResult`, `reconciler.rs` `ReconcileResult`, `cli/categories.rs` `CategoryRow`, `cli/undo.rs` `LastImport`.

**Deliberately not derived:** `ReportKind` — its `K1` variant's slug is `"k1-prep"`, which a `rename_all` derive would render `"k1"`, silently diverging from `as_str()`. If a later task needs it serialized it must use explicit per-variant `#[serde(rename)]`. Also untouched: `settings.rs::Settings` (on-disk `settings.json` snake_case is an existing file format; 31.10 wraps it), `K1Mapping`, `ImporterKind`, clap structs, TUI state.

**Scope note on AC #1:** the "rules data layer" portion is satisfied vacuously — there is no public rule-row struct today. The only `RuleRow` is private in `cli/rules_manager.rs`; the public `rules::list_rules -> Vec<RuleRow>` is created by 31.5, as are `ImportListItem` (31.5) and `RuleTestResult` (31.6). Per the epic contract those API tasks derive on the structs they introduce.

**Tests (AC #3):** four new serialization tests, all pure construction + `serde_json::to_value`, no DB needed — `pnl_report_serializes_camel_case` (asserts the exact top-level key set and that no snake_case key survives), `k1_prep_report_serializes_camel_case` (the nested/Option-heavy case: nested vecs, `validation.compDistRatio`, and a `None` serializing to JSON null), `date_granularity_serializes_camel_case`, and `generic_csv_config_round_trips_camel_case` (covers the Deserialize side).

**Verification:** `cargo fmt --check` clean; `cargo clippy -- -D warnings` (CI form) clean; `cargo test -- --test-threads=1` → 307 lib + 23 integration passed; `cargo test --no-default-features -- --test-threads=1` → 300 lib + 23 integration passed (AC #4). No struct in the inventory is feature-gated, so the derives are identical in both builds.

**Pre-existing issue found, not fixed here:** `cargo clippy --no-default-features -D warnings` reports 2 `needless_return` warnings in the no-pdf fallbacks (`cli/dashboard.rs:852`, `cli/report/mod.rs:160`). Verified against a clean HEAD: identical 2 warnings, unrelated to this task. CI only runs the default-feature clippy, which is why they were never caught. Worth a follow-up task.

**Risk:** low — additive derives only, no behavior or signature changes. Note for 31.9's `types.ts`: `serde_json` maps NaN/Infinity to JSON `null`, so divide-derived `f64` fields (`ExpenseItem.pct`, `K1Validation.comp_dist_ratio`) should be typed nullable-tolerant.
<!-- SECTION:FINAL_SUMMARY:END -->
