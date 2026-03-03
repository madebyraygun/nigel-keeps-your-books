# Import Enhancements Design

**Date:** 2026-03-02
**Issues:** #74, #97, #80

## Overview

Three related import enhancements delivered as a single PR:

1. **#97** — Track and report malformed/skipped rows during import
2. **#74** — Add `--dry-run` flag to preview imports without writing to DB
3. **#80** — Generic CSV importer with configurable column mapping and persistent profiles

## #97 — Malformed Row Tracking

### Problem
CSV read errors in BofA parsers silently `continue` without incrementing the `malformed` counter. The display code already handles malformed counts — the gap is in the parsers.

### Changes
- Add `malformed += 1` before `continue` on CSV read error lines in `parse_bofa_checking`, `parse_bofa_card_format`, and the header-scanning loop
- Gusto parser: track malformed rows instead of hardcoded 0
- Date/empty/header skips remain uncounted (they are expected structural skips, not data errors)

## #74 — Dry-Run Flag

### CLI Interface
```
nigel import statement.csv --account "BofA Checking" --dry-run
```

### Changes
- Add `--dry-run` bool flag to `Import` in `cli/mod.rs`
- Add `dry_run: bool` parameter to `import_file()` in `importer.rs`
- When `dry_run=true`:
  - Parse file and detect format (normal path)
  - Run row-level duplicate detection (normal path)
  - Skip: snapshot creation, INSERT into `imports` table, INSERT into `transactions` table
  - Still return full `ImportResult` with accurate counts
- Add `sample: Vec<ParsedRow>` field to `ImportResult` — first 5 parsed rows (always populated, displayed only in dry-run output)
- CLI output format:
  ```
  Dry run — no changes made
  Format detected: BofA Checking
  245 would be imported, 3 duplicates, 2 malformed

  Sample transactions:
    2025-03-01  PAYROLL DEPOSIT           +4,500.00
    2025-03-02  AMAZON.COM                  -127.43
    2025-03-03  WHOLE FOODS                  -89.12
    2025-03-04  TRANSFER TO SAVINGS       -1,000.00
    2025-03-05  CLIENT PAYMENT            +2,750.00
  ```
- TUI import screen: not applicable (dry-run is a CLI preview tool)

## #80 — Generic CSV Importer

### CLI Interface
```bash
# First import — specify columns
nigel import statement.csv --account "Chase" \
  --format generic --date-col 0 --desc-col 1 --amount-col 3 --date-format "%m/%d/%Y"

# Save as reusable profile
nigel import statement.csv --account "Chase" \
  --format generic --date-col 0 --desc-col 1 --amount-col 3 --save-profile chase

# Subsequent imports — use saved profile
nigel import statement.csv --account "Chase" --format chase
```

### New ImporterKind Variant
- `ImporterKind::GenericCsv` — never auto-detected, must be explicit
- Configured via `GenericCsvConfig` struct:
  ```rust
  pub struct GenericCsvConfig {
      pub date_col: usize,
      pub desc_col: usize,
      pub amount_col: usize,
      pub date_format: String,  // default "%m/%d/%Y"
  }
  ```

### Database Schema (Migration)
New `csv_profiles` table:
```sql
CREATE TABLE csv_profiles (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    date_col INTEGER NOT NULL,
    desc_col INTEGER NOT NULL,
    amount_col INTEGER NOT NULL,
    date_format TEXT NOT NULL DEFAULT '%m/%d/%Y',
    created_at TEXT DEFAULT (datetime('now'))
);
```

### Resolution Logic
In `import_file()`, when `format_key` is provided:
1. Check built-in importers (`bofa_checking`, etc.)
2. If no match, query `csv_profiles` table by name
3. If found, use `GenericCsv` with loaded config
4. If not found, return `UnknownFormat` error

### Parser Behavior
- Skip first row (assumed header)
- For each subsequent row: extract date/desc/amount by column index
- Parse date using configured format string (via `NaiveDate::parse_from_str`)
- Parse amount using existing `parse_amount()` helper
- Track malformed count for unparseable rows
- No auto-detection — `GenericCsv` is only used when explicitly requested

### Save Profile Flow
When `--save-profile <name>` is provided:
1. Validate column flags are present (error if missing)
2. Insert/replace into `csv_profiles` table
3. Proceed with normal import using those columns
