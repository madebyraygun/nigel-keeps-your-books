# Issue #32: Refactor — extract shared helpers from BofA importers

**Link:** https://github.com/madebyraygun/nigel-keeps-your-books/issues/32
**Branch:** `feat/32-refactor-bofa-importers`
**File:** `src/importer.rs` (only file modified)

## Analysis

The BofA Credit Card parser (lines 332-405) and Line of Credit parser (lines 411-476) are ~95% identical.
The only differences:
1. LOC has no `Type` column — always negates amounts
2. CC uses `Type` column (D/C) for sign logic

CSV reader setup (3 lines) is repeated in all 3 BofA parsers (checking, CC, LOC).

## Plan

### Step 1: Add `create_csv_reader` helper
- [x] Extract the 3-line CSV reader setup into `create_csv_reader(file_path: &Path) -> Result<csv::Reader<...>>`
- [x] Update `parse_bofa_checking` to call it
- [x] Update `parse_bofa_credit_card` to call it
- [x] Update `parse_bofa_line_of_credit` to call it
- [x] Run tests — all 93 pass

### Step 2: Unify CC and LOC into `parse_bofa_card_format`
- [x] Create `parse_bofa_card_format(file_path: &Path, has_type_column: bool) -> Result<Vec<ParsedRow>>`
  - `has_type_column: true` → CC behavior (D/C sign logic)
  - `has_type_column: false` → LOC behavior (always negate)
- [x] Reduce `parse_bofa_credit_card` to one-line wrapper: `parse_bofa_card_format(file_path, true)`
- [x] Reduce `parse_bofa_line_of_credit` to one-line wrapper: `parse_bofa_card_format(file_path, false)`
- [x] Run tests — all 93 pass

## NOT doing
- No trait/builder abstraction
- No `find_header_row` or `calculate_field_offset` helpers
- Gusto (XLSX-based) untouched
- `detect_*` functions untouched (they return `bool`, not `Result`)

## Notes
- Net impact: 37 insertions, 85 deletions (−48 lines)
- All 93 tests pass unchanged
