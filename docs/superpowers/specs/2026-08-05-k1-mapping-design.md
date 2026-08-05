# K-1 Worksheet Mapping — Design (task-23)

**Problem:** `nigel report k1` reports Gross Receipts $0.00 and a large ordinary
business loss on real books. `k1_prep` (`src/reports.rs`) counts income only from
categories whose `form_line` is the literal string `Gross receipts` or
`Other income` — but the seeded chart of accounts stores those values in
`tax_line` and leaves `form_line` NULL for every income category, for Cost of
Goods Sold, and for Transfer. Any category without a `form_line` is silently
excluded from the worksheet. Two secondary defects: COGS has no 1120-S line 2
handling at all, and the headline Total Deductions sums meals at 100% while the
Other Deductions sub-table correctly applies the 50% limit.

**Goal:** the worksheet must be correct for *any* chart of accounts, not just the
seeded one. Nothing with period activity may ever silently disappear from the
worksheet.

## Mapping model

One resolution function decides where a category's activity lands on the
worksheet:

1. `form_line = 'excluded'` (reserved value) → deliberately outside the
   return: skipped from totals *and* from Needs-mapping.
2. Any other non-NULL `form_line` → use it (explicit mapping always wins).
3. `form_line` NULL, `category_type = 'income'` → **gross receipts**, marked
   `(auto)` in the worksheet output.
4. `form_line` NULL, `category_type = 'expense'` → **Unmapped**: excluded from
   all totals, listed in the Needs-mapping section.

Rationale: an unmapped income category is almost always gross receipts on a
cash-basis 1120-S, and the inference is visible; a blind expense default could
misfile COGS (line 2) or non-deductible items onto line 19, and overstating
deductions is the worse tax error — so expenses surface instead.

`form_line` vocabulary gains three values used by the resolution and the
worksheet: `1120S-1a` (gross receipts), `1120S-2` (COGS), `1120S-5` (other
income) — alongside the existing `1120S-N`, `K-N`, and new `excluded`. The dead
literal matches on `"Gross receipts"`/`"Other income"` in `form_line` are
removed. `tax_line` remains the Schedule C mapping used by `report tax` and
never drives K-1 logic.

## Worksheet changes (`k1_prep` + text/PDF renderers)

- The category query drops its `form_line IS NOT NULL` filter and selects
  `category_type` so the resolution function can run per category.
- Income Summary becomes: Gross Receipts → COGS (line 2) → Gross Profit →
  Other Income (line 5) → Total Deductions → Ordinary Business Income.
- Headline Total Deductions uses the 50%-limited meals figure, consistent with
  the Other Deductions sub-table.
- New **Needs mapping** section: every category with nonzero period activity
  that resolved to Unmapped, with its total. Sits with the existing
  uncategorized-transaction count as the validation block. Empty → section
  omitted.
- Auto-mapped income rows render with an `(auto)` marker so inferred mappings
  are visible.

## Migration + seed

- New migration (version 3 on `main` — lands before the invoicing branch, whose
  own migration renumbers to 4 when it merges; see Coordination). Backfills
  `form_line` only where it is NULL **and** both `name` and `tax_line` match a
  stock seeded category:
  - `Client Services`, `Hosting & Maintenance`, `Reimbursements`
    (tax_line `Gross receipts`) → `1120S-1a`
  - `Other Income` (tax_line `Other income`) → `1120S-5`
  - `Cost of Goods Sold` (tax_line `Schedule C Part III / 1120-S Line 2`) →
    `1120S-2`
  - `Transfer` (tax_line `Not deductible`) → `excluded`
  - `Uncategorized` stays NULL on purpose: activity there belongs in
    Needs-mapping, not hidden.
  - Custom categories are never touched — they hit the fallback/validation
    paths.
- `DEFAULT_CATEGORIES` in `src/db.rs` gets the same `form_line` values so fresh
  databases start fully mapped.

## Testing

- Resolution function: explicit mapping wins; income fallback; expense
  unmapped; `excluded` skipped everywhere.
- `k1_prep` on a fully custom chart of accounts: income lands in gross receipts
  with `(auto)`, unmapped expense surfaces with its total, excluded category
  appears nowhere.
- COGS math: gross profit and ordinary business income include line 2.
- Meals: headline total equals the sum of per-line deductible amounts (50%
  meals), matching the sub-table.
- Migration: stock-named categories get backfilled; a custom category with the
  same tax_line but different name does not; already-set `form_line` is never
  overwritten.
- Regression shape of the found bug: seeded chart, income + COGS + meals →
  positive ordinary business income, not a loss.

## Out of scope

- Category-manager UI changes (`form_line` is already editable free text
  there). Docs gain the reserved `excluded` value and the fallback behavior.
- `report tax` (Schedule C) behavior is unchanged.
- No renaming/consolidation of `tax_line`/`form_line` columns.

## Release

This fix ships as **v1.0.1**: bump `version` in `Cargo.toml` (and `Cargo.lock`)
as part of the branch.

## Coordination

The open invoicing PR (#172) also adds a migration `version: 3`. This fix lands
first; the invoicing branch renumbers its migration to 4 (its plan already
instructs using the next free integer) before merge.
