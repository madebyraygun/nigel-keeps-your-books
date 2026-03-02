# Replace f64 with rust_decimal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all f64 monetary values with `rust_decimal::Decimal` for exact arithmetic, eliminating floating-point drift in reconciliation and reporting.

**Architecture:** Bottom-up refactor — add crate + helpers first, then update types layer-by-layer: models → schema → parsing → formatting → data queries → TUI → output. Since this is a type-change refactor, compilation will be broken between Tasks 2-8. Checkpoint at Task 9 where `cargo check` must pass, then fix tests in Task 10.

**Tech Stack:** rust_decimal (with `serde` feature for future use), rusqlite TEXT storage, existing ratatui/printpdf stack unchanged.

---

### Task 1: Add rust_decimal and create decimal helpers

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/fmt.rs`
- Modify: `src/tui.rs`

**Step 1: Add rust_decimal dependency**

In `Cargo.toml`, add to `[dependencies]`:

```toml
rust_decimal = { version = "1", features = ["serde-with-str"] }
```

**Step 2: Update `fmt::money()` to accept Decimal**

In `src/fmt.rs`, change the `money` function:

```rust
use rust_decimal::Decimal;

/// Format a Decimal as a dollar amount with thousands separators: $1,234.56
pub fn money(val: Decimal) -> String {
    let negative = val < Decimal::ZERO;
    let abs = val.abs();
    let rounded = abs.round_dp(2);
    let cents = format!("{:.2}", rounded);
    let parts: Vec<&str> = cents.split('.').collect();
    let int_part = parts[0];
    let dec_part = parts[1];

    let mut with_commas = String::new();
    for (i, c) in int_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            with_commas.push(',');
        }
        with_commas.push(c);
    }
    let with_commas: String = with_commas.chars().rev().collect();

    if negative {
        format!("-${with_commas}.{dec_part}")
    } else {
        format!("${with_commas}.{dec_part}")
    }
}
```

Update the tests in `fmt.rs` to use `Decimal`:

```rust
use rust_decimal_macros::dec;

#[test]
fn test_money_formatting() {
    assert_eq!(money(dec!(1234.56)), "$1,234.56");
    assert_eq!(money(dec!(-500.00)), "-$500.00");
    assert_eq!(money(Decimal::ZERO), "$0.00");
    assert_eq!(money(dec!(1000000.99)), "$1,000,000.99");
    assert_eq!(money(dec!(42.10)), "$42.10");
}
```

Note: Add `rust_decimal_macros = "1"` to `[dev-dependencies]` for test convenience.

**Step 3: Update `tui::money_span()` to accept Decimal**

In `src/tui.rs`:

```rust
use rust_decimal::Decimal;

pub fn money_span(amount: Decimal) -> Span<'static> {
    let style = if amount < Decimal::ZERO {
        AMOUNT_NEG_STYLE
    } else {
        AMOUNT_POS_STYLE
    };
    Span::styled(money(amount.abs()), style)
}
```

**Step 4: Run `cargo check` to verify fmt.rs and tui.rs compile (other files will have errors — that's expected)**

Run: `cargo check 2>&1 | head -5`
Expected: Errors in other files referencing `money()` / `money_span()` with f64 — that's fine.

**Step 5: Commit**

```bash
git add Cargo.toml src/fmt.rs src/tui.rs
git commit -m "feat: add rust_decimal, update money formatting to Decimal"
```

---

### Task 2: Update core types and database schema

**Files:**
- Modify: `src/models.rs`
- Modify: `src/db.rs`

**Step 1: Update models.rs struct fields**

Change `Transaction.amount` and `ParsedRow.amount` from `f64` to `Decimal`:

```rust
use rust_decimal::Decimal;

pub struct Transaction {
    // ... other fields unchanged ...
    pub amount: Decimal,
    // ...
}

pub struct ParsedRow {
    pub date: String,
    pub description: String,
    pub amount: Decimal,
}
```

**Step 2: Update db.rs schema — change REAL to TEXT**

In `src/db.rs`, update the schema string:

```sql
-- In transactions table:
    amount TEXT NOT NULL,

-- In reconciliations table:
    statement_balance TEXT,
    calculated_balance TEXT,
```

**Step 3: Commit**

```bash
git add src/models.rs src/db.rs
git commit -m "feat: change monetary types to Decimal, schema REAL to TEXT"
```

---

### Task 3: Update importer parsing layer

**Files:**
- Modify: `src/importer.rs`

**Step 1: Update `parse_amount()` to return `Decimal`**

```rust
use rust_decimal::Decimal;
use std::str::FromStr;

pub fn parse_amount(raw: &str) -> Decimal {
    let s = raw.replace(',', "").replace('"', "").replace('$', "");
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        return -inner.trim().parse::<Decimal>().unwrap_or(Decimal::ZERO);
    }
    s.parse().unwrap_or(Decimal::ZERO)
}
```

**Step 2: Update BofA parsers**

In `parse_bofa_checking()`, `parse_bofa_card_format()`: the `amount` variable from `parse_amount()` is already Decimal now. Update sign logic to use Decimal methods:

- Replace `-parse_amount(x).abs()` → `-parse_amount(x).abs()` (same syntax, Decimal supports `Neg` and `abs()`)
- Replace `-parse_amount(x)` → `-parse_amount(x)` (same — Decimal implements `Neg`)

These should work as-is since Decimal implements the same arithmetic traits.

**Step 3: Update Gusto payroll parser**

In `parse_gusto_payroll()`:
- The `wages_by_date` and `taxes_by_date` maps change from `BTreeMap<String, f64>` to `BTreeMap<String, Decimal>`
- Float values from calamine (`Data::Float(f)`, `Data::Int(i)`) must be converted via string to avoid precision loss:

```rust
let gross = match &row[7] {
    Data::Float(f) => Decimal::from_str(&f.to_string()).unwrap_or(Decimal::ZERO),
    Data::Int(i) => Decimal::from(*i),
    _ => continue,
};
```

- In the result construction, `-total.abs()` works the same with Decimal.

**Step 4: Update `is_duplicate_row()`**

The SQL parameter for `row.amount` is now Decimal. Since SQLite stores it as TEXT, use `row.amount.to_string()` in the params:

```rust
fn is_duplicate_row(conn: &Connection, account_id: i64, row: &ParsedRow) -> bool {
    let mut stmt = conn
        .prepare_cached(
            "SELECT 1 FROM transactions WHERE account_id = ?1 AND date = ?2 AND amount = ?3 AND description = ?4",
        )
        .unwrap();
    stmt.exists(rusqlite::params![account_id, row.date, row.amount.to_string(), row.description])
        .unwrap_or(false)
}
```

**Step 5: Update transaction INSERT in `import_file()`**

Change the amount parameter to `.to_string()`:

```rust
rusqlite::params![account_id, row.date, row.description, row.amount.to_string()],
```

Similarly update the Gusto `auto_categorize_payroll()` WHERE clause to use `.to_string()`.

**Step 6: Update `excel_serial_to_date()` — no change needed (not monetary)**

This function takes f64 for Excel date serials, which is correct — dates are not money.

**Step 7: Commit**

```bash
git add src/importer.rs
git commit -m "feat: update importer parse_amount and all parsers to Decimal"
```

---

### Task 4: Update reports.rs data layer

**Files:**
- Modify: `src/reports.rs`

**Step 1: Update all report struct fields from f64 to Decimal**

Add `use rust_decimal::Decimal;` and change every monetary field:

- `PnlItem.total: Decimal`
- `PnlReport`: `total_income`, `total_expenses`, `net` → `Decimal`
- `ExpenseItem`: `total`, `pct` → `Decimal`
- `VendorItem.total: Decimal`
- `ExpenseBreakdown.total: Decimal`
- `TaxItem.total: Decimal`
- `CashflowMonth`: `inflows`, `outflows`, `net`, `running_balance` → `Decimal`
- `RegisterRow.amount: Decimal`
- `RegisterReport.total: Decimal`
- `FlaggedTransaction.amount: Decimal`
- `AccountBalance.balance: Decimal`
- `BalanceReport`: `total`, `ytd_net_income` → `Decimal`
- `K1LineItem.total: Decimal`
- `K1OtherDeduction`: `total`, `deductible` → `Decimal`
- `K1Validation`: `officer_comp`, `distributions` → `Decimal`, `comp_dist_ratio` → `Option<Decimal>`
- `K1PrepReport`: `gross_receipts`, `other_income`, `total_deductions`, `ordinary_business_income`, `other_deductions_total` → `Decimal`

**Step 2: Update query functions to read Decimal from SQL**

For SQL queries that read `amount` directly (no aggregation), parse from the TEXT column:

```rust
// Helper at top of file:
use rust_decimal::Decimal;
use std::str::FromStr;

fn read_decimal(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Decimal> {
    let s: String = row.get(idx)?;
    Decimal::from_str(&s).map_err(|e| rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Text,
        Box::new(e),
    ))
}
```

For SQL SUM() queries, the result is a REAL in SQLite. Read as f64 and convert via string:

```rust
fn read_decimal_sum(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Decimal> {
    let f: f64 = row.get(idx)?;
    Decimal::from_str(&format!("{:.2}", f)).map_err(|e| rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Real,
        Box::new(e),
    ))
}
```

**Step 3: Update each query function**

Apply `read_decimal()` or `read_decimal_sum()` for every `row.get()` call that reads a monetary value. Update Rust-side arithmetic:

- `income.iter().map(|i| i.total).sum()` works because Decimal implements `Sum`
- `running += inflows + outflows` works because Decimal implements `Add` + `AddAssign`
- `t / total * 100` for percentage — use `Decimal::from(100)` or `dec!(100)`
- Comparisons like `total != 0.0` become `total != Decimal::ZERO`
- `0.0f64` initializers become `Decimal::ZERO`

**Step 4: Update K1 logic**

In `get_k1_prep()`:
- `*wages_by_date.entry(check_date).or_default() += gross;` — Decimal implements Default (returns ZERO) and AddAssign
- `total.abs()` works on Decimal
- `abs_total * 0.5` → use `abs_total * Decimal::new(5, 1)` or `abs_total / Decimal::from(2)`
- `officer_comp / distributions` for ratio — Decimal division works

**Step 5: Commit**

```bash
git add src/reports.rs
git commit -m "feat: update all report structs and queries to Decimal"
```

---

### Task 5: Update reconciler and reviewer

**Files:**
- Modify: `src/reconciler.rs`
- Modify: `src/reviewer.rs`

**Step 1: Update reconciler.rs**

Change `ReconcileResult` fields and the `reconcile()` function signature:

```rust
use rust_decimal::Decimal;
use std::str::FromStr;

pub struct ReconcileResult {
    pub is_reconciled: bool,
    pub statement_balance: Decimal,
    pub calculated_balance: Decimal,
    pub discrepancy: Decimal,
}

pub fn reconcile(
    conn: &Connection,
    account_name: &str,
    month: &str,
    statement_balance: Decimal,
) -> Result<ReconcileResult> {
```

- Read `calculated` from SQL SUM as Decimal (via f64 → string → Decimal, since SUM operates on TEXT-stored values)
- `discrepancy < 0.01` → `discrepancy < Decimal::new(1, 2)`
- Rounding: `(discrepancy * 100).round() / 100` → `discrepancy.round_dp(2)`
- INSERT params: use `.to_string()` for Decimal values

**Step 2: Update reviewer.rs**

Change `FlaggedTxn.amount` from `f64` to `Decimal`:

```rust
pub struct FlaggedTxn {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: Decimal,
    pub account_name: String,
}
```

Update `get_flagged_transactions()` and `get_transaction_by_id()` to read amount via `read_decimal()` helper.

No changes needed to `apply_review()`, `undo_review()`, `update_transaction_category()`, `update_transaction_vendor()`, `toggle_transaction_flag()` — these don't touch amounts.

**Step 3: Commit**

```bash
git add src/reconciler.rs src/reviewer.rs
git commit -m "feat: update reconciler and reviewer to Decimal"
```

---

### Task 6: Update browser and dashboard

**Files:**
- Modify: `src/browser.rs`
- Modify: `src/cli/dashboard.rs`

**Step 1: Update browser.rs**

- `RegisterBrowser.total` → `Decimal`
- `RegisterBrowser::new()` takes `total: Decimal`
- Status line: `money(self.total)` already works since `money()` now takes Decimal
- `tui::money_span(row_data.amount)` already works since money_span takes Decimal
- Test helper `make_rows()`: change amount literals to Decimal:
  ```rust
  amount: if i % 2 == 0 { dec!(100) } else { dec!(-50) },
  ```
- `RegisterBrowser::new(rows, 0.0, ...)` → `RegisterBrowser::new(rows, Decimal::ZERO, ...)`

**Step 2: Update dashboard.rs**

In `DashboardData` struct:
- `total_income: Decimal`, `total_expenses: Decimal`, `net: Decimal`
- `balances: Vec<(String, Decimal)>`
- `top_expenses: Vec<(String, Decimal)>`

Update `load_data()`:
- Where it reads from report data, values are already Decimal after Task 4
- Bar chart: `m.outflows.abs().max(Decimal::ZERO)` — but BarChart needs `u64`. Convert: `outflows.abs().round_dp(0).to_string().parse::<u64>().unwrap_or(0)` or use `rust_decimal::prelude::ToPrimitive` → `outflows.abs().to_u64().unwrap_or(0)`

Update `y_axis_ticks()` and `format_k()` functions — these take f64 for chart rendering. Since BarChart is a ratatui widget that requires integer bar values, convert Decimal → u64 at the chart boundary only.

**Step 3: Commit**

```bash
git add src/browser.rs src/cli/dashboard.rs
git commit -m "feat: update browser and dashboard to Decimal"
```

---

### Task 7: Update CLI report views, text formatters, and reconcile screen

**Files:**
- Modify: `src/cli/report/view.rs`
- Modify: `src/cli/report/text.rs`
- Modify: `src/cli/reconcile_manager.rs`

**Step 1: Update view.rs**

- `money_cell(amount: Decimal)` — parameter type change, body unchanged
- All `money_cell()` calls already pass struct fields which are now Decimal
- `money_span()` calls already take Decimal
- `money(item.total.abs())` — `.abs()` works on Decimal
- `data.net >= 0.0` → `data.net >= Decimal::ZERO`
- `money(data.validation.officer_comp)` — already Decimal

**Step 2: Update text.rs**

- All `money()` calls receive Decimal values from report structs — no signature changes needed
- Comparison `pnl.net >= 0.0` → `pnl.net >= Decimal::ZERO`
- `m.net >= 0.0` → `m.net >= Decimal::ZERO`
- `a.balance >= 0.0` → `a.balance >= Decimal::ZERO`
- `r.amount < 0.0` → `r.amount < Decimal::ZERO`
- `item.deductible < item.total` — Decimal comparison works
- `ratio < 1.0` → `ratio < Decimal::ONE`

**Step 3: Update reconcile_manager.rs**

- Local `ReconcileResult` struct: change f64 fields to Decimal
- `balance: f64 = self.balance.replace(',', "").parse()` → `balance: Decimal = self.balance.replace(',', "").parse()`
- `money(result.calculated_balance)` — already takes Decimal
- The `reconciler::reconcile()` call now takes `Decimal` — matches

**Step 4: Commit**

```bash
git add src/cli/report/view.rs src/cli/report/text.rs src/cli/reconcile_manager.rs
git commit -m "feat: update report views, text formatters, reconcile screen to Decimal"
```

---

### Task 8: Update PDF export

**Files:**
- Modify: `src/pdf.rs`

**Step 1: Update all render functions**

- All `money()` calls receive Decimal values from report structs — they already work
- `report.net >= 0.0` → `report.net >= Decimal::ZERO`
- `report.ordinary_business_income >= 0.0` → `report.ordinary_business_income >= Decimal::ZERO`
- `item.total.abs()` → works on Decimal
- `item.deductible < item.total` → Decimal comparison works
- Add `use rust_decimal::Decimal;` at top

No functional logic changes — just type compatibility.

**Step 2: Commit**

```bash
git add src/pdf.rs
git commit -m "feat: update PDF export to Decimal"
```

---

### Task 9: Update demo data and snake game

**Files:**
- Modify: `src/cli/demo.rs`
- Modify: `src/cli/snake.rs`

**Step 1: Update demo.rs**

- `DemoTxn.amount: Decimal`
- `RecurringTxn.amount: Decimal`
- `RotatingTxn.amount: Decimal`
- `INCOME_BASES`, `MEAL_AMOUNTS` constants: change f64 tuples to Decimal tuples
  - Use `Decimal::new(value_cents, 2)` for const contexts, e.g.: `Decimal::new(-5499, 2)` for `-54.99`
  - Alternatively, since these are `const` arrays and `Decimal::new()` is `const fn`, this works in const context
- Variation arithmetic: `1.0 + ((idx % 7) as f64 - 3.0) * 0.01` needs Decimal arithmetic:
  ```rust
  let vary = Decimal::ONE + (Decimal::from((idx % 7) as i64) - Decimal::from(3)) * Decimal::new(1, 2);
  ```
- Rounding: `(base1 * vary * 100.0).round() / 100.0` → `(base1 * vary).round_dp(2)`
- Interest: `1.50 + (idx % 5) as f64 * 0.25` → `Decimal::new(150, 2) + Decimal::from((idx % 5) as i64) * Decimal::new(25, 2)`
- INSERT params: use `.to_string()` for amount values

**Step 2: Update snake.rs**

- `food_value: Decimal`, `score: Decimal`
- `random_food_value()`: generate random cents as before, construct with `Decimal::new(cents, 2)`
- `self.score += self.food_value` — Decimal implements AddAssign
- Score display in UI: `money(self.score)` — already takes Decimal
- The `phase: f64` field is NOT monetary — it controls animation and stays f64

**Step 3: Commit**

```bash
git add src/cli/demo.rs src/cli/snake.rs
git commit -m "feat: update demo data generation and snake game to Decimal"
```

---

### Task 10: Fix compilation and tests

**Files:**
- Potentially all files from Tasks 1-9
- Modify: `Cargo.toml` (add rust_decimal_macros to dev-deps)

**Step 1: Run `cargo check` and fix all remaining compilation errors**

Run: `cargo check 2>&1`

Common fixes needed:
- Any remaining `0.0` or `0.0f64` → `Decimal::ZERO`
- Any `as f64` casts on monetary values → remove or convert properly
- Any `f64` function parameters that should be `Decimal`
- Missing `use rust_decimal::Decimal;` imports
- Type mismatches in test code (literal floats in test assertions)

**Step 2: Add `rust_decimal_macros` to dev-dependencies**

In `Cargo.toml`:
```toml
[dev-dependencies]
tempfile = "3"
rust_decimal_macros = "1"
```

**Step 3: Fix all test files**

Update test assertions across all modules:
- `assert_eq!(report.total_income, 1000.0)` → `assert_eq!(report.total_income, dec!(1000))`
- `assert_eq!(result.discrepancy, 0.0)` → `assert_eq!(result.discrepancy, Decimal::ZERO)`
- Test helper functions that construct transactions with f64 amounts → use Decimal
- SQL INSERT statements in tests: use `.to_string()` for Decimal params or use string literals for amount values

**Step 4: Run `cargo test` and fix failures**

Run: `cargo test 2>&1`

Focus areas:
- `reports::tests` — all seeded amounts need Decimal or string conversion
- `reconciler::tests` — balance comparisons
- `importer::tests` — parse_amount assertions
- `browser::tests` — make_rows helper
- `demo::tests` — transaction generation tests

**Step 5: Run `cargo test --no-default-features` to verify without gusto/pdf**

Run: `cargo test --no-default-features 2>&1`

**Step 6: Run `cargo build --release` for clean release build**

Run: `cargo build --release 2>&1`

**Step 7: Commit**

```bash
git add -A
git commit -m "fix: resolve all compilation errors and test failures for Decimal migration"
```

---

### Task 11: Update documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md`

**Step 1: Update CLAUDE.md**

In the **Key Design Constraints** section, replace:
```
- Cash amounts are plain `f64` — negative = expense, positive = income
```
with:
```
- Cash amounts use `rust_decimal::Decimal` stored as TEXT in SQLite — negative = expense, positive = income; no f64 in financial code paths
```

In the **Architecture** section, add under Database:
```
Monetary values stored as TEXT (rust_decimal::Decimal serialization) for exact arithmetic.
```

**Step 2: Update README.md**

Add a note in the Features or Technical section about exact decimal arithmetic.

**Step 3: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: update documentation for Decimal money representation"
```

---

## Key Patterns Reference

### Reading Decimal from SQLite

For direct column reads (amount stored as TEXT):
```rust
fn read_decimal(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Decimal> {
    let s: String = row.get(idx)?;
    Decimal::from_str(&s).map_err(|e| rusqlite::Error::FromSqlConversionFailure(
        idx, rusqlite::types::Type::Text, Box::new(e),
    ))
}
```

For SQL SUM() results (SQLite returns REAL from aggregation on TEXT):
```rust
fn read_decimal_sum(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Decimal> {
    let f: f64 = row.get(idx)?;
    Decimal::from_str(&format!("{:.2}", f)).map_err(|e| rusqlite::Error::FromSqlConversionFailure(
        idx, rusqlite::types::Type::Real, Box::new(e),
    ))
}
```

### Writing Decimal to SQLite

Always convert to string: `rusqlite::params![amount.to_string()]`

### Common Decimal conversions

| f64 pattern | Decimal equivalent |
|---|---|
| `0.0` / `0.0f64` | `Decimal::ZERO` |
| `1.0` | `Decimal::ONE` |
| `100.0` | `Decimal::from(100)` or `Decimal::ONE_HUNDRED` |
| `-54.99` literal | `Decimal::new(-5499, 2)` |
| `val >= 0.0` | `val >= Decimal::ZERO` |
| `val.abs()` | `val.abs()` (same) |
| `val * 0.5` | `val / Decimal::from(2)` or `val * Decimal::new(5, 1)` |
| `(val * 100.0).round() / 100.0` | `val.round_dp(2)` |
| `format!("{:.2}", val)` | `format!("{:.2}", val)` (Decimal implements Display) |
| `str.parse::<f64>()` | `str.parse::<Decimal>()` |
| In tests: `1234.56` | `dec!(1234.56)` (from rust_decimal_macros) |

### Files NOT changed (no monetary f64)

- `src/settings.rs` — no monetary data
- `src/error.rs` — no monetary data
- `src/effects.rs` — f64 used for animation/particles, not money
- `src/categorizer.rs` — works on rules/patterns, no amounts
- `src/cli/onboarding.rs` — no monetary data
- `src/cli/account_manager.rs` — no monetary data
- `src/cli/category_manager.rs` — no monetary data
- `src/cli/rules_manager.rs` — no monetary data
- `src/cli/import_manager.rs` — delegates to importer, no direct f64
- `src/cli/load_manager.rs` — no monetary data
- `src/cli/splash.rs` — no monetary data
