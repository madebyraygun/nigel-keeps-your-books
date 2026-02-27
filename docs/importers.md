# Adding Importers to Nigel

All importer code lives in `src/importer.rs`. Nigel uses enum dispatch — each bank format is a variant of `ImporterKind`, with match arms for detection and parsing. No trait objects, no plugin registry.

## Architecture

```
CSV/XLSX file
  → ImporterKind::detect() inspects headers/structure
  → ImporterKind::parse() extracts Vec<ParsedRow>
  → import_file() deduplicates and inserts into SQLite
```

Key types:

- **`ImporterKind`** — enum with one variant per bank format
- **`ParsedRow`** — intermediate representation: `{ date: String, description: String, amount: f64 }`
- **`ImportResult`** — returned by `import_file()`: counts of imported/skipped rows

## Step-by-Step: Adding an Importer

This walks through adding a fictional "Chase Checking" importer. The CSV looks like:

```csv
Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #
DEBIT,01/15/2025,AMAZON PURCHASE,-42.50,ACH_DEBIT,1957.50,
CREDIT,01/16/2025,DIRECT DEPOSIT,3200.00,ACH_CREDIT,5157.50,
```

### 1. Add a variant to `ImporterKind`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImporterKind {
    BofaChecking,
    BofaCreditCard,
    BofaLineOfCredit,
    #[cfg(feature = "gusto")]
    GustoPayroll,
    ChaseChecking,  // ← new
}
```

### 2. Add match arms to all methods

Add the new variant to each `match` in the `impl ImporterKind` block:

```rust
// key() — snake_case identifier, used by --format CLI flag
Self::ChaseChecking => "chase_checking",

// name() — human-readable display name
Self::ChaseChecking => "Chase Checking",

// account_types() — which account types this format applies to
Self::ChaseChecking => &["checking"],

// detect() — call your detection function
Self::ChaseChecking => detect_chase_checking(file_path),

// parse() — call your parse function
Self::ChaseChecking => parse_chase_checking(file_path),
```

If your importer doesn't need post-import processing, the `has_post_import()` and `post_import()` match arms already handle the `_ => false` / `_ => Ok(())` fallback.

### 3. Add to `ALL_IMPORTERS`

```rust
const ALL_IMPORTERS: &[ImporterKind] = &[
    ImporterKind::BofaChecking,
    ImporterKind::BofaCreditCard,
    ImporterKind::BofaLineOfCredit,
    #[cfg(feature = "gusto")]
    ImporterKind::GustoPayroll,
    ImporterKind::ChaseChecking,  // ← new
];
```

### 4. Write the detect function

Inspect the file's headers or structure to identify the format. Return `true` if the file matches, `false` otherwise.

```rust
fn detect_chase_checking(file_path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(file_path) else {
        return false;
    };
    // Chase checking CSVs have this specific header row
    content.starts_with("Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #")
}
```

Tips:
- Check for unique header signatures — column names, column count, or file preamble
- Use `csv::ReaderBuilder` for more robust header inspection (see `detect_bofa_checking`)
- Return `false` on any I/O error

### 5. Write the parse function

Extract rows into `Vec<ParsedRow>`, following the ParsedRow contract (see below).

```rust
fn parse_chase_checking(file_path: &Path) -> Result<Vec<ParsedRow>> {
    let content = std::fs::read_to_string(file_path)?;
    let mut rows = Vec::new();
    let mut found_header = false;

    for line in content.lines() {
        let fields: Vec<&str> = parse_csv_line(line);
        if !found_header {
            if fields.first().map_or(false, |f| f.trim() == "Details") {
                found_header = true;
            }
            continue;
        }
        if fields.len() < 4 || fields[0].trim().is_empty() {
            continue;
        }
        let Some(date) = parse_date_mdy(fields[1]) else {
            continue;
        };
        let description = fields[2].trim().to_string();
        let amount = parse_amount(fields[3]); // already negative for debits
        rows.push(ParsedRow {
            date,
            description,
            amount,
        });
    }
    Ok(rows)
}
```

### 6. Post-import hook (optional)

If your importer needs to auto-categorize transactions after import (like Gusto does for payroll), add logic to `has_post_import()` and `post_import()`:

```rust
fn has_post_import(&self) -> bool {
    match self {
        Self::ChaseChecking => true,  // only if needed
        // ...
    }
}
```

Most bank importers don't need this.

### 7. Add a test

```rust
#[test]
fn test_chase_checking_parse() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chase.csv");
    let content = "Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #\n\
                   DEBIT,01/15/2025,AMAZON PURCHASE,-42.50,ACH_DEBIT,1957.50,\n\
                   CREDIT,01/16/2025,DIRECT DEPOSIT,3200.00,ACH_CREDIT,5157.50,\n";
    std::fs::write(&path, content).unwrap();

    let rows = ImporterKind::ChaseChecking.parse(&path).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].date, "2025-01-15");
    assert_eq!(rows[0].description, "AMAZON PURCHASE");
    assert_eq!(rows[0].amount, -42.5);
    assert_eq!(rows[1].amount, 3200.0);
}

#[test]
fn test_chase_checking_detect() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chase.csv");
    std::fs::write(&path, "Details,Posting Date,Description,Amount,Type,Balance,Check or Slip #\n").unwrap();
    assert!(ImporterKind::ChaseChecking.detect(&path));

    let other = dir.path().join("other.csv");
    std::fs::write(&other, "Date,Description,Amount\n").unwrap();
    assert!(!ImporterKind::ChaseChecking.detect(&other));
}
```

## Shared Helpers

These are available in `src/importer.rs` for use in any parser:

| Function | Purpose |
|----------|---------|
| `parse_amount(raw)` | Strips commas, quotes, whitespace → `f64` (returns 0.0 on failure) |
| `parse_date_mdy(raw)` | Converts `MM/DD/YYYY` → `YYYY-MM-DD` string |
| `excel_serial_to_date(serial)` | Converts Excel serial number → `YYYY-MM-DD` string |
| `parse_csv_line(line)` | Splits a CSV line by comma (simple, no quoted-field handling) |

## ParsedRow Contract

Every parser must produce `Vec<ParsedRow>` where:

- **`date`** — `YYYY-MM-DD` format (use `parse_date_mdy` for MM/DD/YYYY sources, `excel_serial_to_date` for XLSX)
- **`description`** — as-is from the source, trimmed
- **`amount`** — negative = expense/debit, positive = income/credit. If the source uses separate debit/credit columns or a type indicator, normalize to this convention in the parser.

## Feature Gating

To make an importer optional (like Gusto), gate it behind a Cargo feature:

1. Add the feature to `Cargo.toml`:
   ```toml
   [features]
   default = ["gusto"]
   gusto = ["dep:calamine"]
   ```

2. Gate enum variant, match arms, `ALL_IMPORTERS` entry, and functions with `#[cfg(feature = "...")]`:
   ```rust
   #[cfg(feature = "gusto")]
   GustoPayroll,
   ```

3. Test both configurations:
   ```bash
   cargo test                        # with feature
   cargo test --no-default-features  # without feature
   ```

Feature gating is only needed for importers that pull in heavy or optional dependencies. Standard CSV importers don't need gating.

## Account Type Matching

When `--format` is not specified, `get_for_file()` narrows candidates by `account_types()` then runs `detect()`. If your importer shares an account type with an existing one (e.g., two checking importers), make sure `detect()` returns `true` only for files that actually match your format. The first importer whose `detect()` returns `true` wins.
