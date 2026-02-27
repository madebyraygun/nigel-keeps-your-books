# Issues

## Open

---

## Closed

### #4 — Formatter problem with BofA Bank account importer

Custom `parse_csv_line` split on all commas including those inside quoted fields, causing `"2,000.00"` to be parsed as `2.00`. Replaced custom parser with the `csv` crate for all BofA parsers.

### #5 — `parse_csv_line` returns fields with surrounding quote characters

Resolved by #6 — switching to the `csv` crate handles quote stripping automatically.

### #6 — Detection and parsing use different CSV strategies

All BofA parsers (checking, credit card, LOC) now use the `csv` crate with `flexible(true)` for both detection and parsing, ensuring consistent RFC 4180 handling.

### #7 — `parse_amount` silently returns $0 for parenthesized negatives

`parse_amount` now handles parenthesized negatives `(500.00)` → `-500.0` and strips `$` currency symbols.

### #8 — Credit card / LOC parsers use magic column indices without header validation

Credit card and LOC parsers now read column positions from the header row (`Posting Date`, `Payee`, `Amount`, `Type`) instead of using hard-coded indices.

### #9 — `parse_date_mdy` does not validate calendar ranges

Now uses `chrono::NaiveDate::from_ymd_opt` to validate month/day/year, rejecting invalid dates like `13/45/2025`.

### #10 — `imports.record_count` records inserted count, not total parsed rows

Changed to use `parsed_rows.len()` so `record_count` reflects the total rows in the file, not just the de-duplicated count.

---

### #3 — Multi-database support, backup, and cleanup

Added `nigel load <path>` to switch between existing data directories, `nigel backup [--output <path>]` for safe database backups via SQLite backup API, and `nigel status` to show the active database with summary statistics. Removed redundant `imports/` archive folder since duplicate detection uses checksums, not file copies.

---

### #1 — Bring back K-1 prep report

Added K-1 preparation worksheet for 1120-S filings. Includes `reports::get_k1_prep()` data layer, `nigel report k1` terminal display, `nigel export k1` PDF export, and inclusion in `nigel export all`. Categories with `form_line` values are grouped into income summary, deduction lines, Schedule K items, and Line 19 other deductions (with 50% meals limitation). Validation warnings for uncategorized transactions and officer comp vs. distributions.

---

### #2 — Claude skill: pre-import CSV reviewer with rule suggestions

Created `.claude/skills/csv-rule-reviewer/SKILL.md` — a Claude skill that analyzes a bank CSV before import, compares transaction descriptions against existing rules, clusters unmatched descriptions by vendor, and suggests ready-to-run `nigel rules add` commands.
