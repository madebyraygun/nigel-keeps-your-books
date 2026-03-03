Import Enhancements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `--dry-run` flag, malformed row tracking, and generic CSV importer with persistent profiles to the import system.

**Architecture:** Three features layered on the existing `import_file()` pipeline. #97 (malformed tracking) fixes gaps in existing parsers. #74 (dry-run) adds a boolean gate in `import_file()` that skips DB writes but preserves parsing + duplicate detection. #80 (generic CSV) adds a new importer variant resolved via DB-stored profiles. A schema migration creates the `csv_profiles` table.

**Tech Stack:** Rust, rusqlite, clap, csv, chrono

**Issues:** #74, #97, #80

---

### Task 1: Fix malformed row tracking gaps (#97)

**Files:**
- Modify: `src/importer.rs:307` (parse_bofa_checking CSV read error)
- Modify: `src/importer.rs:385` (parse_bofa_card_format CSV read error)
- Test: `src/importer.rs` (existing test module)

**Step 1: Write the failing test**

Add to `src/importer.rs` `mod tests`:

```rust
#[test]
fn test_bofa_checking_counts_csv_read_errors_as_malformed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.csv");
    // Valid header, one good row, then a line with invalid UTF-8 isn't
    // possible in a &str literal, so instead we test with a row that the
    // CSV parser returns Ok for but has bad data — this is already covered.
    // Instead, verify that a completely empty amount counts as malformed.
    let content = "\
Date,Description,Amount,Running Bal.
01/15/2025,GOOD PAYMENT,-100.00,900.00
01/16/2025,EMPTY AMOUNT,,900.00
01/17/2025,ANOTHER GOOD,50.00,950.00
";
    std::fs::write(&path, content).unwrap();
    let (rows, malformed) = ImporterKind::BofaChecking.parse(&path).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(malformed, 1, "empty amount should be counted as malformed");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_bofa_checking_counts_csv_read_errors_as_malformed -- --nocapture`

Expected: FAIL — empty string passes through to `parse_amount("")` which returns `None`, but the `continue` on line 325-327 already increments malformed. Actually, `parse_amount("")` on an empty trimmed string: `"".parse::<f64>()` returns `None`, so malformed IS incremented. This test should pass already. Let me adjust the test to verify the actual gap: CSV record read errors.

Actually, the real gap from the code review is that `let Ok(record) = result else { continue }` on lines 307 and 385 skip without incrementing malformed. These are hard to trigger in tests (require actual corrupt CSV bytes), but we should still fix the code. Let's write the fix directly and verify existing tests still pass, plus add a test for a more realistic malformed scenario.

**Step 1 (revised): Fix CSV read error tracking**

In `src/importer.rs`, `parse_bofa_checking` (line 307), change:
```rust
let Ok(record) = result else { continue };
```
to:
```rust
let Ok(record) = result else {
    malformed += 1;
    continue;
};
```

But `malformed` isn't declared yet at that point — it's declared on line 304. The `for` loop starts on line 306, after `malformed` is declared, so this works.

In `src/importer.rs`, `parse_bofa_card_format` (line 385), change:
```rust
let Ok(record) = result else { continue };
```
to:
```rust
let Ok(record) = result else {
    malformed += 1;
    continue;
};
```

**Step 2: Run all tests to verify nothing breaks**

Run: `cargo test`
Expected: All 201 tests pass.

**Step 3: Commit**

```bash
git add src/importer.rs
git commit -m "fix: count CSV read errors as malformed rows in BofA parsers (#97)"
```

---

### Task 2: Add `sample` field to ImportResult and `dry_run` param to `import_file`

**Files:**
- Modify: `src/importer.rs:193-198` (ImportResult struct)
- Modify: `src/importer.rs:200-277` (import_file function)
- Modify: `src/cli/import.rs` (run function — pass dry_run=false)
- Modify: `src/cli/import_manager.rs:291` (call site — pass dry_run=false)
- Test: `src/importer.rs` (test module)

**Step 1: Write the failing test**

Add to `src/importer.rs` `mod tests`:

```rust
#[test]
fn test_import_result_has_sample() {
    let (dir, conn) = test_db();
    add_test_account(&conn);
    let csv_path = write_bofa_csv(
        dir.path(),
        "stmt.csv",
        &[
            ("01/15/2025", "PAYMENT ONE", "-100.00"),
            ("01/16/2025", "PAYMENT TWO", "-250.00"),
            ("01/17/2025", "DEPOSIT", "500.00"),
        ],
    );
    let result = import_file(&conn, &csv_path, "Test Checking", Some("bofa_checking"), false).unwrap();
    assert_eq!(result.sample.len(), 3);
    assert_eq!(result.sample[0].description, "PAYMENT ONE");
}

#[test]
fn test_dry_run_does_not_write_to_db() {
    let (dir, conn) = test_db();
    add_test_account(&conn);
    let csv_path = write_bofa_csv(
        dir.path(),
        "stmt.csv",
        &[
            ("01/15/2025", "PAYMENT ONE", "-100.00"),
            ("01/16/2025", "PAYMENT TWO", "-250.00"),
        ],
    );
    let result = import_file(&conn, &csv_path, "Test Checking", Some("bofa_checking"), true).unwrap();
    assert_eq!(result.imported, 2);
    assert_eq!(result.skipped, 0);
    assert!(!result.duplicate_file);

    // Verify nothing was written
    let tx_count: i64 = conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0)).unwrap();
    assert_eq!(tx_count, 0, "dry run should not insert transactions");
    let import_count: i64 = conn.query_row("SELECT count(*) FROM imports", [], |r| r.get(0)).unwrap();
    assert_eq!(import_count, 0, "dry run should not insert import record");
}

#[test]
fn test_dry_run_detects_duplicates() {
    let (dir, conn) = test_db();
    add_test_account(&conn);
    let csv_path = write_bofa_csv(
        dir.path(),
        "stmt.csv",
        &[
            ("01/15/2025", "PAYMENT ONE", "-100.00"),
            ("01/16/2025", "PAYMENT TWO", "-250.00"),
        ],
    );
    // Real import first
    import_file(&conn, &csv_path, "Test Checking", Some("bofa_checking"), false).unwrap();

    // Dry run of a file with one overlapping row
    let csv2 = write_bofa_csv(
        dir.path(),
        "stmt2.csv",
        &[
            ("01/16/2025", "PAYMENT TWO", "-250.00"),
            ("01/18/2025", "PAYMENT THREE", "-300.00"),
        ],
    );
    let result = import_file(&conn, &csv2, "Test Checking", Some("bofa_checking"), true).unwrap();
    assert_eq!(result.imported, 1);
    assert_eq!(result.skipped, 1);
}

#[test]
fn test_dry_run_sample_capped_at_five() {
    let (dir, conn) = test_db();
    add_test_account(&conn);
    let rows: Vec<(&str, &str, &str)> = vec![
        ("01/01/2025", "TXN 1", "-10.00"),
        ("01/02/2025", "TXN 2", "-20.00"),
        ("01/03/2025", "TXN 3", "-30.00"),
        ("01/04/2025", "TXN 4", "-40.00"),
        ("01/05/2025", "TXN 5", "-50.00"),
        ("01/06/2025", "TXN 6", "-60.00"),
        ("01/07/2025", "TXN 7", "-70.00"),
    ];
    let csv_path = write_bofa_csv(dir.path(), "stmt.csv", &rows);
    let result = import_file(&conn, &csv_path, "Test Checking", Some("bofa_checking"), true).unwrap();
    assert_eq!(result.sample.len(), 5, "sample should be capped at 5");
    assert_eq!(result.imported, 7);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_import_result_has_sample test_dry_run -- --nocapture`
Expected: FAIL — `import_file` doesn't accept 5th argument yet.

**Step 3: Implement the changes**

In `src/importer.rs`, update `ImportResult`:
```rust
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub malformed: usize,
    pub duplicate_file: bool,
    pub sample: Vec<ParsedRow>,
}
```

Update `import_file` signature to add `dry_run: bool`:
```rust
pub fn import_file(
    conn: &Connection,
    file_path: &Path,
    account_name: &str,
    format_key: Option<&str>,
    dry_run: bool,
) -> Result<ImportResult>
```

In the body:
- After `compute_checksum`, when checking file-level duplicates: keep as-is (dry-run should still detect file duplicates), but add `sample: Vec::new()` to the early-return `ImportResult`
- After `importer.parse()`: compute `sample = parsed_rows.iter().take(5).cloned().collect()`
- When `dry_run` is true: skip the `INSERT INTO imports`, skip the transaction insert loop (but still iterate to count imported/skipped), skip `post_import`. Specifically:
  ```rust
  if !dry_run {
      // INSERT INTO imports ...
      let import_id = conn.last_insert_rowid();
      // ... transaction insert loop ...
      // ... post_import ...
  } else {
      // Count without inserting
      for row in &parsed_rows {
          if is_duplicate_row(conn, account_id, row) {
              skipped += 1;
          } else {
              imported += 1;
          }
      }
  }
  ```
- Add `sample` to the final `ImportResult`

Update all existing call sites to pass `false` for `dry_run`:
- `src/cli/import.rs:21` — `import_file(&conn, &file_path, account, format, false)`
- `src/cli/import_manager.rs:291` — `import_file(conn, file_path, account_name, None, false)`

Update all existing tests that call `import_file` to pass `false` as 5th argument. There are 6 tests:
- `test_import_file_inserts_transactions`
- `test_import_file_detects_file_duplicate`
- `test_import_file_detects_row_duplicates`
- `test_import_file_records_batch`
- `test_import_skips_malformed_amount_rows`
- `test_import_file_links_transactions_to_import_batch`

**Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass (201 existing + 4 new).

**Step 5: Commit**

```bash
git add src/importer.rs src/cli/import.rs src/cli/import_manager.rs
git commit -m "feat: add dry_run param to import_file and sample field to ImportResult (#74)"
```

---

### Task 3: Add `--dry-run` CLI flag and output formatting

**Files:**
- Modify: `src/cli/mod.rs:72-81` (Import args)
- Modify: `src/cli/import.rs` (run function)
- Modify: `src/main.rs:107-111` (dispatch)
- Test: `tests/cli_dispatch.rs` (integration test)

**Step 1: Write the failing test**

Add to `tests/cli_dispatch.rs`:

```rust
#[test]
fn test_import_dry_run_no_db_writes() {
    let dir = setup_test_env();
    let csv_path = dir.path().join("stmt.csv");
    std::fs::write(
        &csv_path,
        "Date,Description,Amount,Running Bal.\n01/15/2025,PAYMENT,-100.00,900.00\n",
    )
    .unwrap();

    let output = Command::cargo_bin("nigel")
        .unwrap()
        .args([
            "import",
            csv_path.to_str().unwrap(),
            "--account",
            "BofA Checking",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dry run"), "should show dry run label");
    assert!(stdout.contains("1 would be imported"), "should show would-be count");
}
```

Note: `setup_test_env` needs to create a test DB with an account. Check the existing test setup in `tests/cli_dispatch.rs` first and use the same pattern.

**Step 2: Run test to verify it fails**

Run: `cargo test test_import_dry_run_no_db_writes -- --nocapture`
Expected: FAIL — `--dry-run` flag doesn't exist.

**Step 3: Implement**

In `src/cli/mod.rs`, add to Import variant:
```rust
Import {
    /// Path to CSV or XLSX file to import
    file: String,
    /// Account name to import into
    #[arg(long)]
    account: String,
    /// Importer format key (e.g. bofa_checking)
    #[arg(long)]
    format: Option<String>,
    /// Preview import without writing to database
    #[arg(long)]
    dry_run: bool,
},
```

In `src/main.rs` dispatch, update the Import match arm:
```rust
Commands::Import {
    file,
    account,
    format,
    dry_run,
} => cli::import::run(&file, &account, format.as_deref(), dry_run),
```

In `src/cli/import.rs`, update `run` signature and logic:
```rust
pub fn run(file: &str, account: &str, format: Option<&str>, dry_run: bool) -> Result<()> {
    let file_path = PathBuf::from(file);
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;

    if !dry_run {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let snap_path = data_dir.join(format!("snapshots/pre-import-{stamp}.db"));
        backup::snapshot(&conn, &snap_path)?;
        println!("Pre-import snapshot saved to {}", snap_path.display());
    }

    let result = import_file(&conn, &file_path, account, format, dry_run)?;

    if result.duplicate_file {
        println!("This file has already been imported (duplicate checksum).");
        return Ok(());
    }

    if dry_run {
        println!("Dry run \u{2014} no changes made");
        if result.malformed > 0 {
            println!(
                "{} would be imported, {} duplicates, {} malformed",
                result.imported, result.skipped, result.malformed
            );
        } else {
            println!(
                "{} would be imported, {} duplicates",
                result.imported, result.skipped
            );
        }
        if !result.sample.is_empty() {
            println!("\nSample transactions:");
            for row in &result.sample {
                let sign = if row.amount >= 0.0 { "+" } else { "" };
                println!("  {}  {:40} {:>10}", row.date, row.description, format!("{sign}{:.2}", row.amount));
            }
        }
        return Ok(());
    }

    if result.malformed > 0 {
        println!(
            "{} imported, {} skipped (duplicates), {} skipped (malformed data)",
            result.imported, result.skipped, result.malformed
        );
    } else {
        println!(
            "{} imported, {} skipped (duplicates)",
            result.imported, result.skipped
        );
    }

    let cat_result = categorize_transactions(&conn)?;
    println!(
        "{} categorized, {} still flagged",
        cat_result.categorized, cat_result.still_flagged
    );

    Ok(())
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/cli/mod.rs src/cli/import.rs src/main.rs tests/cli_dispatch.rs
git commit -m "feat: add --dry-run flag to import command (#74)"
```

---

### Task 4: Add csv_profiles migration

**Files:**
- Modify: `src/migrations.rs` (add migration v2)
- Test: `src/migrations.rs` (test module)

**Step 1: Write the failing test**

Add to `src/migrations.rs` `mod tests`:

```rust
#[test]
fn test_csv_profiles_table_exists_after_migration() {
    let (_dir, conn) = test_db();
    let exists: bool = conn
        .query_row(
            "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='csv_profiles'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(exists, "csv_profiles table should exist after init_db");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_csv_profiles_table_exists_after_migration -- --nocapture`
Expected: FAIL — table doesn't exist.

**Step 3: Implement the migration**

In `src/migrations.rs`, update `MIGRATIONS`:
```rust
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "baseline — establish schema version tracking",
        up: |_conn| Ok(()),
    },
    Migration {
        version: 2,
        description: "add csv_profiles table for generic CSV column mappings",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE csv_profiles (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    date_col INTEGER NOT NULL,
                    desc_col INTEGER NOT NULL,
                    amount_col INTEGER NOT NULL,
                    date_format TEXT NOT NULL DEFAULT '%m/%d/%Y',
                    created_at TEXT DEFAULT (datetime('now'))
                )",
            )?;
            Ok(())
        },
    },
];
```

**Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass. `LATEST_VERSION` auto-updates to 2 since it reads from the array.

**Step 5: Commit**

```bash
git add src/migrations.rs
git commit -m "feat: add csv_profiles migration for generic CSV importer (#80)"
```

---

### Task 5: Implement GenericCsv parser

**Files:**
- Modify: `src/importer.rs` (add GenericCsvConfig, parse_generic_csv function, profile DB helpers)
- Test: `src/importer.rs` (test module)

**Step 1: Write the failing tests**

Add to `src/importer.rs` `mod tests`:

```rust
#[test]
fn test_parse_generic_csv_default_date_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("generic.csv");
    let content = "\
Date,Memo,Debit,Credit,Balance
01/15/2025,COFFEE SHOP,-5.50,,994.50
01/16/2025,PAYCHECK,,2000.00,2994.50
01/17/2025,RENT,-1500.00,,1494.50
";
    std::fs::write(&path, content).unwrap();
    let config = GenericCsvConfig {
        date_col: 0,
        desc_col: 1,
        amount_col: 2,
        date_format: "%m/%d/%Y".to_string(),
    };
    let (rows, malformed) = parse_generic_csv(&path, &config).unwrap();
    assert_eq!(rows.len(), 2); // row 2 has empty amount col → skipped
    assert_eq!(malformed, 1);
    assert_eq!(rows[0].date, "2025-01-15");
    assert_eq!(rows[0].description, "COFFEE SHOP");
    assert_eq!(rows[0].amount, -5.50);
}

#[test]
fn test_parse_generic_csv_custom_date_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("generic.csv");
    let content = "\
transaction_date,description,amount
2025-01-15,COFFEE SHOP,-5.50
2025-01-16,PAYCHECK,2000.00
";
    std::fs::write(&path, content).unwrap();
    let config = GenericCsvConfig {
        date_col: 0,
        desc_col: 1,
        amount_col: 2,
        date_format: "%Y-%m-%d".to_string(),
    };
    let (rows, _) = parse_generic_csv(&path, &config).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].date, "2025-01-15");
    assert_eq!(rows[1].amount, 2000.0);
}

#[test]
fn test_generic_csv_profile_save_and_load() {
    let (_dir, conn) = test_db();
    let config = GenericCsvConfig {
        date_col: 0,
        desc_col: 2,
        amount_col: 4,
        date_format: "%d/%m/%Y".to_string(),
    };
    save_csv_profile(&conn, "chase", &config).unwrap();
    let loaded = load_csv_profile(&conn, "chase").unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.date_col, 0);
    assert_eq!(loaded.desc_col, 2);
    assert_eq!(loaded.amount_col, 4);
    assert_eq!(loaded.date_format, "%d/%m/%Y");
}

#[test]
fn test_generic_csv_profile_overwrite() {
    let (_dir, conn) = test_db();
    let config1 = GenericCsvConfig {
        date_col: 0,
        desc_col: 1,
        amount_col: 2,
        date_format: "%m/%d/%Y".to_string(),
    };
    save_csv_profile(&conn, "chase", &config1).unwrap();

    let config2 = GenericCsvConfig {
        date_col: 1,
        desc_col: 3,
        amount_col: 5,
        date_format: "%Y-%m-%d".to_string(),
    };
    save_csv_profile(&conn, "chase", &config2).unwrap();

    let loaded = load_csv_profile(&conn, "chase").unwrap().unwrap();
    assert_eq!(loaded.date_col, 1);
    assert_eq!(loaded.desc_col, 3);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_parse_generic_csv test_generic_csv_profile -- --nocapture`
Expected: FAIL — `GenericCsvConfig`, `parse_generic_csv`, `save_csv_profile`, `load_csv_profile` don't exist.

**Step 3: Implement**

Add to `src/importer.rs` before `import_file`:

```rust
// ---------------------------------------------------------------------------
// Generic CSV config and profile helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GenericCsvConfig {
    pub date_col: usize,
    pub desc_col: usize,
    pub amount_col: usize,
    pub date_format: String,
}

pub fn save_csv_profile(conn: &Connection, name: &str, config: &GenericCsvConfig) -> Result<()> {
    conn.execute(
        "INSERT INTO csv_profiles (name, date_col, desc_col, amount_col, date_format)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(name) DO UPDATE SET
            date_col = excluded.date_col,
            desc_col = excluded.desc_col,
            amount_col = excluded.amount_col,
            date_format = excluded.date_format",
        rusqlite::params![
            name,
            config.date_col as i64,
            config.desc_col as i64,
            config.amount_col as i64,
            config.date_format,
        ],
    )?;
    Ok(())
}

pub fn load_csv_profile(conn: &Connection, name: &str) -> Result<Option<GenericCsvConfig>> {
    let mut stmt = conn.prepare(
        "SELECT date_col, desc_col, amount_col, date_format FROM csv_profiles WHERE name = ?1",
    )?;
    let result = stmt.query_row([name], |row| {
        Ok(GenericCsvConfig {
            date_col: row.get::<_, i64>(0)? as usize,
            desc_col: row.get::<_, i64>(1)? as usize,
            amount_col: row.get::<_, i64>(2)? as usize,
            date_format: row.get(3)?,
        })
    });
    match result {
        Ok(config) => Ok(Some(config)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
```

Add the parser function:

```rust
pub fn parse_generic_csv(file_path: &Path, config: &GenericCsvConfig) -> Result<(Vec<ParsedRow>, usize)> {
    let mut rdr = create_csv_reader(file_path)?;
    let mut rows = Vec::new();
    let mut malformed = 0usize;
    let mut first = true;
    let min_cols = [config.date_col, config.desc_col, config.amount_col]
        .into_iter()
        .max()
        .unwrap_or(0)
        + 1;

    for result in rdr.records() {
        let Ok(record) = result else {
            malformed += 1;
            continue;
        };
        // Skip header row
        if first {
            first = false;
            continue;
        }
        if record.len() < min_cols {
            malformed += 1;
            continue;
        }
        let raw_date = record[config.date_col].trim();
        let date = match chrono::NaiveDate::parse_from_str(raw_date, &config.date_format) {
            Ok(d) => d.format("%Y-%m-%d").to_string(),
            Err(_) => {
                malformed += 1;
                continue;
            }
        };
        let description = record[config.desc_col].trim().to_string();
        if description.is_empty() {
            continue;
        }
        let Some(amount) = parse_amount(&record[config.amount_col]) else {
            malformed += 1;
            continue;
        };
        rows.push(ParsedRow {
            date,
            description,
            amount,
        });
    }
    Ok((rows, malformed))
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/importer.rs
git commit -m "feat: add GenericCsvConfig, parser, and profile persistence (#80)"
```

---

### Task 6: Wire generic CSV into import_file and CLI

**Files:**
- Modify: `src/importer.rs:200-233` (import_file format resolution)
- Modify: `src/cli/mod.rs:72-81` (add CLI flags)
- Modify: `src/cli/import.rs` (pass new flags, handle save-profile)
- Modify: `src/main.rs:107-111` (dispatch new fields)
- Test: `src/importer.rs` + `tests/cli_dispatch.rs`

**Step 1: Write the failing test**

Add to `src/importer.rs` `mod tests`:

```rust
#[test]
fn test_import_file_with_generic_csv() {
    let (dir, conn) = test_db();
    add_test_account(&conn);
    let path = dir.path().join("generic.csv");
    let content = "\
Date,Note,Amount
01/15/2025,COFFEE,-5.50
01/16/2025,SALARY,3000.00
";
    std::fs::write(&path, content).unwrap();
    let config = GenericCsvConfig {
        date_col: 0,
        desc_col: 1,
        amount_col: 2,
        date_format: "%m/%d/%Y".to_string(),
    };
    save_csv_profile(&conn, "test_bank", &config).unwrap();

    let result = import_file(&conn, &path, "Test Checking", Some("test_bank"), false).unwrap();
    assert_eq!(result.imported, 2);
    let count: i64 = conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_import_file_with_generic_csv -- --nocapture`
Expected: FAIL — `import_file` doesn't know about csv_profiles.

**Step 3: Implement**

In `src/importer.rs`, update `import_file` format resolution (around line 228-233). Replace:
```rust
let importer = if let Some(key) = format_key {
    get_by_key(key).ok_or_else(|| NigelError::UnknownFormat(key.to_string()))?
} else {
    get_for_file(&account_type, file_path)
        .ok_or_else(|| NigelError::NoImporter(account_type.clone()))?
};

let (parsed_rows, malformed) = importer.parse(file_path)?;
```

With:
```rust
enum ResolvedImporter {
    BuiltIn(ImporterKind),
    Generic(GenericCsvConfig),
}

let resolved = if let Some(key) = format_key {
    if let Some(kind) = get_by_key(key) {
        ResolvedImporter::BuiltIn(kind)
    } else if let Some(config) = load_csv_profile(conn, key)? {
        ResolvedImporter::Generic(config)
    } else {
        return Err(NigelError::UnknownFormat(key.to_string()));
    }
} else {
    ResolvedImporter::BuiltIn(
        get_for_file(&account_type, file_path)
            .ok_or_else(|| NigelError::NoImporter(account_type.clone()))?,
    )
};

let (parsed_rows, malformed) = match &resolved {
    ResolvedImporter::BuiltIn(kind) => kind.parse(file_path)?,
    ResolvedImporter::Generic(config) => parse_generic_csv(file_path, config)?,
};
```

Update the post-import section too:
```rust
if let ResolvedImporter::BuiltIn(importer) = &resolved {
    if importer.has_post_import() {
        importer.post_import(conn, account_id, &parsed_rows)?;
    }
}
```

Now add CLI flags. In `src/cli/mod.rs`, update Import variant:
```rust
Import {
    /// Path to CSV or XLSX file to import
    file: String,
    /// Account name to import into
    #[arg(long)]
    account: String,
    /// Importer format key (e.g. bofa_checking, or a saved profile name)
    #[arg(long)]
    format: Option<String>,
    /// Preview import without writing to database
    #[arg(long)]
    dry_run: bool,
    /// Column index for date (0-based, used with --format generic)
    #[arg(long)]
    date_col: Option<usize>,
    /// Column index for description (0-based, used with --format generic)
    #[arg(long)]
    desc_col: Option<usize>,
    /// Column index for amount (0-based, used with --format generic)
    #[arg(long)]
    amount_col: Option<usize>,
    /// Date format string (default: %m/%d/%Y, used with --format generic)
    #[arg(long, default_value = None)]
    date_format: Option<String>,
    /// Save column mapping as a reusable profile name
    #[arg(long)]
    save_profile: Option<String>,
},
```

Update `src/main.rs` dispatch:
```rust
Commands::Import {
    file,
    account,
    format,
    dry_run,
    date_col,
    desc_col,
    amount_col,
    date_format,
    save_profile,
} => cli::import::run(
    &file,
    &account,
    format.as_deref(),
    dry_run,
    date_col,
    desc_col,
    amount_col,
    date_format.as_deref(),
    save_profile.as_deref(),
),
```

Update `src/cli/import.rs`:
```rust
use crate::importer::{import_file, save_csv_profile, GenericCsvConfig};

pub fn run(
    file: &str,
    account: &str,
    format: Option<&str>,
    dry_run: bool,
    date_col: Option<usize>,
    desc_col: Option<usize>,
    amount_col: Option<usize>,
    date_format: Option<&str>,
    save_profile: Option<&str>,
) -> Result<()> {
    let file_path = PathBuf::from(file);
    let data_dir = get_data_dir();
    let conn = get_connection(&data_dir.join("nigel.db"))?;

    // If --save-profile is provided, save the generic CSV config
    if let Some(profile_name) = save_profile {
        let config = GenericCsvConfig {
            date_col: date_col.ok_or_else(|| {
                crate::error::NigelError::Other("--date-col is required with --save-profile".into())
            })?,
            desc_col: desc_col.ok_or_else(|| {
                crate::error::NigelError::Other("--desc-col is required with --save-profile".into())
            })?,
            amount_col: amount_col.ok_or_else(|| {
                crate::error::NigelError::Other("--amount-col is required with --save-profile".into())
            })?,
            date_format: date_format.unwrap_or("%m/%d/%Y").to_string(),
        };
        save_csv_profile(&conn, profile_name, &config)?;
        println!("Saved profile '{profile_name}'");
    }

    // If column flags are provided without a profile, use format=generic inline
    let effective_format = if format.is_none() && date_col.is_some() {
        // Build inline GenericCsvConfig, save as temporary profile "__inline__"
        let config = GenericCsvConfig {
            date_col: date_col.unwrap(),
            desc_col: desc_col.ok_or_else(|| {
                crate::error::NigelError::Other("--desc-col is required with --date-col".into())
            })?,
            amount_col: amount_col.ok_or_else(|| {
                crate::error::NigelError::Other("--amount-col is required with --date-col".into())
            })?,
            date_format: date_format.unwrap_or("%m/%d/%Y").to_string(),
        };
        save_csv_profile(&conn, "__inline__", &config)?;
        Some("__inline__")
    } else {
        format
    };

    if !dry_run {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let snap_path = data_dir.join(format!("snapshots/pre-import-{stamp}.db"));
        backup::snapshot(&conn, &snap_path)?;
        println!("Pre-import snapshot saved to {}", snap_path.display());
    }

    let result = import_file(&conn, &file_path, account, effective_format, dry_run)?;

    // ... rest of dry_run / normal output as in Task 3 ...
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/importer.rs src/cli/mod.rs src/cli/import.rs src/main.rs
git commit -m "feat: wire generic CSV importer into CLI with column flags and profiles (#80)"
```

---

### Task 7: Add CLI integration tests for generic CSV

**Files:**
- Modify: `tests/cli_dispatch.rs`

**Step 1: Write integration tests**

```rust
#[test]
fn test_import_generic_csv_with_column_flags() {
    let dir = setup_test_env();
    let csv_path = dir.path().join("bank.csv");
    std::fs::write(
        &csv_path,
        "trans_date,ref,memo,amount,balance\n01/15/2025,001,COFFEE,-5.50,994.50\n01/16/2025,002,SALARY,3000.00,3994.50\n",
    )
    .unwrap();

    let output = Command::cargo_bin("nigel")
        .unwrap()
        .args([
            "import",
            csv_path.to_str().unwrap(),
            "--account", "BofA Checking",
            "--date-col", "0",
            "--desc-col", "2",
            "--amount-col", "3",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2 imported"), "should import 2 rows: {stdout}");
}

#[test]
fn test_import_generic_csv_with_saved_profile() {
    let dir = setup_test_env();
    let csv1 = dir.path().join("bank1.csv");
    std::fs::write(
        &csv1,
        "trans_date,ref,memo,amount,balance\n01/15/2025,001,COFFEE,-5.50,994.50\n",
    )
    .unwrap();

    // Import with --save-profile
    Command::cargo_bin("nigel")
        .unwrap()
        .args([
            "import",
            csv1.to_str().unwrap(),
            "--account", "BofA Checking",
            "--date-col", "0",
            "--desc-col", "2",
            "--amount-col", "3",
            "--save-profile", "mybank",
        ])
        .output()
        .unwrap();

    // Second import using saved profile
    let csv2 = dir.path().join("bank2.csv");
    std::fs::write(
        &csv2,
        "trans_date,ref,memo,amount,balance\n01/20/2025,003,GROCERIES,-42.00,952.50\n",
    )
    .unwrap();

    let output = Command::cargo_bin("nigel")
        .unwrap()
        .args([
            "import",
            csv2.to_str().unwrap(),
            "--account", "BofA Checking",
            "--format", "mybank",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 imported"), "should import via saved profile: {stdout}");
}
```

**Step 2: Run tests**

Run: `cargo test test_import_generic -- --nocapture`
Expected: All pass.

**Step 3: Commit**

```bash
git add tests/cli_dispatch.rs
git commit -m "test: add integration tests for generic CSV import (#80)"
```

---

### Task 8: Update documentation

**Files:**
- Modify: `CLAUDE.md` (Commands section, Architecture section)
- Modify: `docs/importers.md` (if it exists, add generic CSV docs)

**Step 1: Update CLAUDE.md**

Add to Commands section:
```
nigel import <file> --account <name> --dry-run           # Preview without importing
nigel import <file> --account <name> --date-col 0 --desc-col 1 --amount-col 3  # Generic CSV
nigel import <file> --account <name> --date-col 0 --desc-col 1 --amount-col 3 --save-profile chase  # Save profile
nigel import <file> --account <name> --format chase      # Use saved profile
```

Update Architecture > Importers bullet to mention generic CSV:
```
- **Importers:** `src/importer.rs` — `ImporterKind` enum dispatch (bofa_checking, bofa_credit_card, bofa_line_of_credit, gusto_payroll); each variant implements `detect()` and `parse()`; `GenericCsvConfig` supports user-defined column mappings stored as profiles in `csv_profiles` table; no plugin registry
```

Add to Key Design Constraints:
```
- Generic CSV profiles are stored in `csv_profiles` table; `--format <name>` resolves built-in importers first, then csv_profiles; generic CSV is never auto-detected
- `--dry-run` skips snapshot creation, imports table insertion, and transaction insertion; still runs full parse and duplicate detection
```

**Step 2: Commit**

```bash
git add CLAUDE.md docs/importers.md
git commit -m "docs: update CLAUDE.md and importers.md for import enhancements (#74, #97, #80)"
```

---

### Task 9: Final verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass (201 existing + ~10 new).

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

**Step 3: Verify build**

Run: `cargo build`
Expected: Clean build.
