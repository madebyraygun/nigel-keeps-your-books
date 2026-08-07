# `nigel recategorize` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A non-interactive `nigel recategorize` command that moves transactions to a new category by explicit IDs or by filters, per `docs/superpowers/specs/2026-08-06-recategorize-command-design.md`.

**Architecture:** Data-layer functions in `src/reviewer.rs` (keeps the `&Connection`-in / plain-structs-out convention), a new CLI module `src/cli/recategorize.rs` for arg validation, table output, and confirmation, a new clap variant in `src/cli/mod.rs`, dispatch in `src/main.rs`. Date filtering reuses `reports::date_filter` (made `pub(crate)`); pattern matching reuses `categorizer::matches`.

**Tech Stack:** Rust, clap (derive), rusqlite, comfy-table, assert_cmd/predicates/tempfile for integration tests.

## Global Constraints

- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` must pass after every task.
- Match types are exactly `contains`, `starts_with`, `regex` (same as rules).
- Applying mirrors a review: `SET category_id = ?, is_flagged = 0, flag_reason = NULL`; vendor untouched; single SQL transaction.
- IDs and filters are mutually exclusive; filter mode with zero filters is an error.

---

### Task 1: Data layer — candidates query + batch update (src/reviewer.rs)

**Files:**
- Modify: `src/reviewer.rs` (add after `toggle_transaction_flag`, tests in existing `mod tests`)
- Modify: `src/reports.rs:17` (`fn date_filter` → `pub(crate) fn date_filter`)

**Interfaces:**
- Consumes: `reports::date_filter(year, month, from_date, to_date) -> Result<(String, Vec<String>)>` (clause references `t.date`), `categorizer::matches(description, pattern, match_type) -> bool`.
- Produces (used by Task 2):

```rust
#[derive(Default)]
pub struct RecategorizeFilter {
    pub from_category_id: Option<i64>,
    pub uncategorized: bool,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub pattern: Option<String>,
    pub match_type: String, // "contains" | "starts_with" | "regex"
    pub account_id: Option<i64>,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
}

pub struct RecategorizeCandidate {
    pub id: i64,
    pub date: String,
    pub description: String,
    pub amount: f64,
    pub category: Option<String>, // current category name
}

pub fn find_transactions_for_recategorize(conn: &Connection, filter: &RecategorizeFilter) -> Result<Vec<RecategorizeCandidate>>;
pub fn get_transactions_by_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<RecategorizeCandidate>>; // error naming missing IDs
pub fn recategorize_transactions(conn: &Connection, ids: &[i64], category_id: i64) -> Result<usize>;
```

- [ ] **Step 1: Write failing unit tests** in `src/reviewer.rs` `mod tests`. The existing helpers `test_db()` and `add_flagged_txn(conn)` are available; add a helper that inserts a categorized transaction:

```rust
fn add_categorized_txn(conn: &Connection, date: &str, desc: &str, amount: f64, category: &str) -> i64 {
    let cat_id: i64 = conn
        .query_row("SELECT id FROM categories WHERE name = ?1", [category], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, is_flagged) \
         VALUES (1, ?1, ?2, ?3, ?4, 0)",
        rusqlite::params![date, desc, amount, cat_id],
    )
    .unwrap();
    conn.last_insert_rowid()
}
```

(Account row: reuse whatever `add_flagged_txn` does to satisfy the FK — if it inserts an account, mirror it; check that helper first and copy its account setup.)

Tests (names/asserts):

```rust
#[test]
fn test_find_for_recategorize_by_category_and_year() {
    // txn A: 2025 'Cost of Goods Sold'; txn B: 2024 'Cost of Goods Sold'; txn C: 2025 'Travel'
    // filter: from_category_id = COGS id, year = 2025 → only A
}

#[test]
fn test_find_for_recategorize_pattern_and_amount() {
    // 'ISTOCKPHOTO' desc, amount -45.0 vs 'MIXAM' -667.10
    // pattern "istock" (contains, case-insensitive per categorizer::matches) → 1 match
    // min_amount 100.0 → only MIXAM
}

#[test]
fn test_find_for_recategorize_uncategorized() {
    // add_flagged_txn (category NULL) + one categorized → uncategorized: true finds only the NULL one
}

#[test]
fn test_get_transactions_by_ids_unknown_id_errors() {
    // ids [real, 99999] → Err mentioning "99999"; and get_transactions_by_ids(&[real]) → Ok(len 1)
}

#[test]
fn test_recategorize_transactions_updates_and_clears_flags() {
    // flagged txn + categorized txn → recategorize both to 'Travel' id
    // assert both category_id = travel, is_flagged = 0, flag_reason IS NULL, return == 2
}
```

- [ ] **Step 2: Run tests, verify they fail to compile** (functions absent): `cargo test recategorize 2>&1 | tail -20`

- [ ] **Step 3: Implement.** In `src/reports.rs` change `fn date_filter` to `pub(crate) fn date_filter`. In `src/reviewer.rs`:

```rust
pub fn find_transactions_for_recategorize(
    conn: &Connection,
    filter: &RecategorizeFilter,
) -> Result<Vec<RecategorizeCandidate>> {
    let (date_clause, date_params) = crate::reports::date_filter(
        filter.year,
        filter.month,
        filter.from_date.as_deref(),
        filter.to_date.as_deref(),
    )?;
    let mut clauses = vec![date_clause];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
        date_params.into_iter().map(|p| Box::new(p) as Box<dyn rusqlite::types::ToSql>).collect();

    if let Some(cat_id) = filter.from_category_id {
        params.push(Box::new(cat_id));
        clauses.push(format!("t.category_id = ?{}", params.len()));
    }
    if filter.uncategorized {
        clauses.push("t.category_id IS NULL".to_string());
    }
    if let Some(acct_id) = filter.account_id {
        params.push(Box::new(acct_id));
        clauses.push(format!("t.account_id = ?{}", params.len()));
    }
    if let Some(min) = filter.min_amount {
        params.push(Box::new(min));
        clauses.push(format!("ABS(t.amount) >= ?{}", params.len()));
    }
    if let Some(max) = filter.max_amount {
        params.push(Box::new(max));
        clauses.push(format!("ABS(t.amount) <= ?{}", params.len()));
    }

    let sql = format!(
        "SELECT t.id, t.date, t.description, t.amount, c.name \
         FROM transactions t LEFT JOIN categories c ON t.category_id = c.id \
         WHERE {} ORDER BY t.date, t.id",
        clauses.join(" AND ")
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let mut rows: Vec<RecategorizeCandidate> = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(RecategorizeCandidate {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                category: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if let Some(ref pattern) = filter.pattern {
        rows.retain(|r| crate::categorizer::matches(&r.description, pattern, &filter.match_type));
    }
    Ok(rows)
}
```

Note the date_filter clause uses `?1`/`?2` numbering and comes first, so its parameter numbers stay aligned with the params vec.

```rust
pub fn get_transactions_by_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<RecategorizeCandidate>> {
    let mut out = Vec::with_capacity(ids.len());
    let mut missing = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT t.id, t.date, t.description, t.amount, c.name \
         FROM transactions t LEFT JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
    )?;
    for &id in ids {
        match stmt.query_row([id], |row| {
            Ok(RecategorizeCandidate {
                id: row.get(0)?,
                date: row.get(1)?,
                description: row.get(2)?,
                amount: row.get(3)?,
                category: row.get(4)?,
            })
        }) {
            Ok(c) => out.push(c),
            Err(rusqlite::Error::QueryReturnedNoRows) => missing.push(id.to_string()),
            Err(e) => return Err(e.into()),
        }
    }
    if !missing.is_empty() {
        return Err(NigelError::Other(format!(
            "No transaction found with ID{} {}. Nothing was changed.",
            if missing.len() == 1 { "" } else { "s" },
            missing.join(", ")
        )));
    }
    Ok(out)
}

pub fn recategorize_transactions(conn: &Connection, ids: &[i64], category_id: i64) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut updated = 0;
    {
        let mut stmt = tx.prepare(
            "UPDATE transactions SET category_id = ?1, is_flagged = 0, flag_reason = NULL WHERE id = ?2",
        )?;
        for &id in ids {
            updated += stmt.execute(rusqlite::params![category_id, id])?;
        }
    }
    tx.commit()?;
    Ok(updated)
}
```

- [ ] **Step 4: Run and pass:** `cargo test 2>&1 | tail -5` → all pass. Then `cargo clippy --all-targets -- -D warnings && cargo fmt`

- [ ] **Step 5: Commit:** `git add -A && git commit -m "feat(reviewer): data layer for bulk recategorization"`

---

### Task 2: CLI — clap variant, validation, output, confirmation

**Files:**
- Modify: `src/cli/mod.rs` (new `Commands::Recategorize` variant + `pub mod recategorize;`)
- Create: `src/cli/recategorize.rs`
- Modify: `src/main.rs` (dispatch arm; `needs_existing_db`/`needs_password` default true for this command — no change to those guards)

**Interfaces:**
- Consumes: Task 1 functions; `cli::parse_month_opt`; `categorizer` match-type semantics; `comfy_table::{Cell, Table}`; `std::io::IsTerminal`.
- Produces: `cli::recategorize::run(args: RecategorizeArgs) -> Result<()>` where `RecategorizeArgs` is a clap `Args` struct flattened into the variant.

- [ ] **Step 1: Add the clap surface.** In `src/cli/mod.rs` add `pub mod recategorize;` to the module list and this variant to `Commands` (after `Categorize`):

```rust
/// Change the category of existing transactions by ID or filters.
Recategorize {
    #[command(flatten)]
    args: recategorize::RecategorizeArgs,
},
```

In `src/cli/recategorize.rs` define the args (clap `Args` derive keeps mod.rs small; precedent: `ReportOutputArgs`):

```rust
use clap::Args;

#[derive(Args)]
pub struct RecategorizeArgs {
    /// Transaction IDs to recategorize (mutually exclusive with filters)
    pub ids: Vec<i64>,
    /// Target category name
    #[arg(long)]
    pub category: String,
    /// Filter: current category name
    #[arg(long = "from-category")]
    pub from_category: Option<String>,
    /// Filter: only uncategorized transactions
    #[arg(long)]
    pub uncategorized: bool,
    /// Filter: year YYYY
    #[arg(long)]
    pub year: Option<i32>,
    /// Filter: month YYYY-MM
    #[arg(long)]
    pub month: Option<String>,
    /// Filter: start date YYYY-MM-DD (requires --to)
    #[arg(long = "from")]
    pub from_date: Option<String>,
    /// Filter: end date YYYY-MM-DD (requires --from)
    #[arg(long = "to")]
    pub to_date: Option<String>,
    /// Filter: description pattern
    #[arg(long)]
    pub pattern: Option<String>,
    /// Match type for --pattern: contains, starts_with, regex
    #[arg(long = "match-type", default_value = "contains")]
    pub match_type: String,
    /// Filter: account name
    #[arg(long)]
    pub account: Option<String>,
    /// Filter: minimum absolute amount
    #[arg(long = "min-amount")]
    pub min_amount: Option<f64>,
    /// Filter: maximum absolute amount
    #[arg(long = "max-amount")]
    pub max_amount: Option<f64>,
    /// Preview without writing
    #[arg(long)]
    pub dry_run: bool,
    /// Apply without confirmation (filter mode; required when stdin is not a TTY)
    #[arg(long)]
    pub yes: bool,
}
```

- [ ] **Step 2: Write failing validation unit tests** in `src/cli/recategorize.rs` `mod tests`. Extract validation into a pure function so it's testable without a DB:

```rust
pub(crate) fn validate(args: &RecategorizeArgs) -> Result<()>;

#[test] fn ids_and_filters_conflict() { /* ids=[1], year=Some(2025) → Err contains "either IDs or filters" */ }
#[test] fn no_ids_no_filters_errors() { /* all empty → Err contains "at least one filter" */ }
#[test] fn uncategorized_conflicts_with_from_category() { /* both set → Err */ }
#[test] fn year_conflicts_with_date_range() { /* year + from/to → Err */ }
#[test] fn bad_match_type_errors() { /* match_type "fuzzy" → Err listing valid types */ }
#[test] fn bad_regex_errors() { /* match_type regex, pattern "(" → Err "Invalid regex" */ }
```

A helper for tests: `fn base_args() -> RecategorizeArgs` building an empty struct with `category: "Travel".into()`, `match_type: "contains".into()`, everything else `None`/`false`/empty.

- [ ] **Step 3: Run tests, verify fail:** `cargo test cli::recategorize 2>&1 | tail -10`

- [ ] **Step 4: Implement `validate` + `run`:**

```rust
fn has_filters(args: &RecategorizeArgs) -> bool {
    args.from_category.is_some()
        || args.uncategorized
        || args.year.is_some()
        || args.month.is_some()
        || args.from_date.is_some()
        || args.to_date.is_some()
        || args.pattern.is_some()
        || args.account.is_some()
        || args.min_amount.is_some()
        || args.max_amount.is_some()
}

pub(crate) fn validate(args: &RecategorizeArgs) -> Result<()> {
    let valid_types = ["contains", "starts_with", "regex"];
    if !valid_types.contains(&args.match_type.as_str()) {
        return Err(NigelError::Other(format!(
            "Invalid match type: {}. Must be one of: {}",
            args.match_type,
            valid_types.join(", ")
        )));
    }
    if args.match_type == "regex" {
        if let Some(ref p) = args.pattern {
            regex::Regex::new(p).map_err(|e| NigelError::Other(format!("Invalid regex: {e}")))?;
        }
    }
    if !args.ids.is_empty() && has_filters(args) {
        return Err(NigelError::Other(
            "Select transactions with either IDs or filters, not both".to_string(),
        ));
    }
    if args.ids.is_empty() && !has_filters(args) {
        return Err(NigelError::Other(
            "Provide transaction IDs or at least one filter (see `nigel recategorize --help`)".to_string(),
        ));
    }
    if args.uncategorized && args.from_category.is_some() {
        return Err(NigelError::Other(
            "--uncategorized and --from-category are mutually exclusive".to_string(),
        ));
    }
    if (args.year.is_some() || args.month.is_some())
        && (args.from_date.is_some() || args.to_date.is_some())
    {
        return Err(NigelError::Other(
            "--year/--month and --from/--to are mutually exclusive".to_string(),
        ));
    }
    Ok(())
}
```

`run` (name resolution mirrors `rules::add`; month parsing mirrors `parse_month_opt` usage):

```rust
pub fn run(args: RecategorizeArgs) -> Result<()> {
    validate(&args)?;
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;

    let target_id: i64 = conn
        .query_row(
            "SELECT id FROM categories WHERE name = ?1 AND is_active = 1",
            [&args.category],
            |row| row.get(0),
        )
        .map_err(|_| NigelError::UnknownCategory(args.category.clone()))?;

    let candidates = if args.ids.is_empty() {
        let from_category_id = match &args.from_category {
            Some(name) => Some(
                conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |r| r.get(0))
                    .map_err(|_| NigelError::UnknownCategory(name.clone()))?,
            ),
            None => None,
        };
        let account_id = match &args.account {
            Some(name) => Some(
                conn.query_row("SELECT id FROM accounts WHERE name = ?1", [name], |r| r.get(0))
                    .map_err(|_| NigelError::UnknownAccount(name.clone()))?,
            ),
            None => None,
        };
        let (year, month) = if args.month.is_some() {
            crate::cli::parse_month_opt(&args.month)
        } else {
            (args.year, None)
        };
        let filter = RecategorizeFilter {
            from_category_id,
            uncategorized: args.uncategorized,
            year,
            month,
            from_date: args.from_date.clone(),
            to_date: args.to_date.clone(),
            pattern: args.pattern.clone(),
            match_type: args.match_type.clone(),
            account_id,
            min_amount: args.min_amount,
            max_amount: args.max_amount,
        };
        find_transactions_for_recategorize(&conn, &filter)?
    } else {
        get_transactions_by_ids(&conn, &args.ids)?
    };

    let (to_move, already): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|c| c.category.as_deref() != Some(args.category.as_str()));

    if to_move.is_empty() && already.is_empty() {
        println!("No transactions matched.");
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(vec!["ID", "Date", "Description", "Amount", "Category", "→"]);
    let mut total = 0.0;
    for c in &to_move {
        total += c.amount.abs();
        table.add_row(vec![
            Cell::new(c.id),
            Cell::new(&c.date),
            Cell::new(&c.description),
            Cell::new(format!("{:.2}", c.amount)),
            Cell::new(c.category.as_deref().unwrap_or("—")),
            Cell::new(&args.category),
        ]);
    }
    println!("{table}");
    if !already.is_empty() {
        println!("Skipping {} already in {}.", already.len(), args.category);
    }
    println!(
        "{} transaction{} → {} (total ${:.2})",
        to_move.len(),
        if to_move.len() == 1 { "" } else { "s" },
        args.category,
        total
    );

    if to_move.is_empty() {
        return Ok(());
    }
    if args.dry_run {
        println!("Dry run — nothing written.");
        return Ok(());
    }

    // Filter mode requires confirmation; explicit IDs are their own confirmation.
    if args.ids.is_empty() && !args.yes {
        if std::io::stdin().is_terminal() {
            print!("Apply? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if !answer.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        } else {
            return Err(NigelError::Other(
                "Refusing to apply a filter-based recategorization without confirmation. Pass --yes.".to_string(),
            ));
        }
    }

    let ids: Vec<i64> = to_move.iter().map(|c| c.id).collect();
    let updated = recategorize_transactions(&conn, &ids, target_id)?;
    println!("Recategorized {updated} transaction{} → {}.", if updated == 1 { "" } else { "s" }, args.category);
    Ok(())
}
```

Imports for the module: `use std::io::IsTerminal;`, `use comfy_table::{Cell, Table};`, `use crate::db::get_connection;`, `use crate::error::{NigelError, Result};`, `use crate::reviewer::{find_transactions_for_recategorize, get_transactions_by_ids, recategorize_transactions, RecategorizeFilter};`, `use crate::settings::get_data_dir;`.

In `src/main.rs` dispatch (after the `Commands::Categorize` arm):

```rust
Commands::Recategorize { args } => cli::recategorize::run(args),
```

- [ ] **Step 5: Run and pass:** `cargo test 2>&1 | tail -5`, then `cargo clippy --all-targets -- -D warnings && cargo fmt`

- [ ] **Step 6: Commit:** `git add -A && git commit -m "feat(cli): nigel recategorize command"`

---

### Task 3: Integration tests (tests/cli_dispatch.rs)

**Files:**
- Modify: `tests/cli_dispatch.rs` (append; reuse `TestEnv`)

**Interfaces:**
- Consumes: `TestEnv::{new, init_and_demo, cmd, db, data_dir, encrypt}` (already in the file), demo data (has categorized transactions in multiple categories).

- [ ] **Step 1: Write the tests** (append at end of file):

```rust
/// Pick a transaction ID and its current category name from the demo data.
fn any_categorized_txn(env: &TestEnv) -> (i64, String) {
    env.db()
        .query_row(
            "SELECT t.id, c.name FROM transactions t JOIN categories c ON t.category_id = c.id \
             WHERE c.name != 'Travel' LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("demo data has categorized transactions")
}

#[test]
fn recategorize_by_id_moves_and_clears_flag() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, _old) = any_categorized_txn(&env);
    env.db()
        .execute("UPDATE transactions SET is_flagged = 1, flag_reason = 'x' WHERE id = ?1", [id])
        .unwrap();

    env.cmd()
        .args(["recategorize", &id.to_string(), "--category", "Travel"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized 1 transaction"));

    let (cat, flagged): (String, i64) = env
        .db()
        .query_row(
            "SELECT c.name, t.is_flagged FROM transactions t JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(cat, "Travel");
    assert_eq!(flagged, 0);
}

#[test]
fn recategorize_filter_requires_yes_without_tty() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (_, old) = any_categorized_txn(&env);

    env.cmd()
        .args(["recategorize", "--from-category", &old, "--category", "Travel"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}

#[test]
fn recategorize_filter_with_yes_applies() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (_, old) = any_categorized_txn(&env);

    env.cmd()
        .args(["recategorize", "--from-category", &old, "--category", "Travel", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized"));

    let remaining: i64 = env
        .db()
        .query_row(
            "SELECT COUNT(*) FROM transactions t JOIN categories c ON t.category_id = c.id WHERE c.name = ?1",
            [&old],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn recategorize_dry_run_writes_nothing() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, old) = any_categorized_txn(&env);

    env.cmd()
        .args(["recategorize", &id.to_string(), "--category", "Travel", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"));

    let cat: String = env
        .db()
        .query_row(
            "SELECT c.name FROM transactions t JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cat, old);
}

#[test]
fn recategorize_unknown_id_changes_nothing() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, old) = any_categorized_txn(&env);

    env.cmd()
        .args(["recategorize", &id.to_string(), "999999", "--category", "Travel"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("999999"));

    let cat: String = env
        .db()
        .query_row(
            "SELECT c.name FROM transactions t JOIN categories c ON t.category_id = c.id WHERE t.id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cat, old);
}

#[test]
fn recategorize_works_on_encrypted_db_via_env_password() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, _old) = any_categorized_txn(&env);
    env.encrypt("hunter2");

    env.cmd()
        .args(["recategorize", &id.to_string(), "--category", "Travel"])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .timeout(std::time::Duration::from_secs(20))
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized 1 transaction"));
}
```

Adjust `any_categorized_txn` if demo data lacks a 'Travel' category — check with `nigel demo` data (`src/cli/demo.rs`); pick a target category that demo always creates (fall back to 'Meals' or read one from the DB).

- [ ] **Step 2: Run, verify current failures are only the new tests:** `cargo test --test cli_dispatch recategorize 2>&1 | tail -20` (they should pass already if Tasks 1–2 are correct; any failure here is a real bug — fix it, don't weaken the test)

- [ ] **Step 3: Full gate:** `cargo test 2>&1 | tail -5 && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

- [ ] **Step 4: Commit:** `git add -A && git commit -m "test: integration coverage for nigel recategorize"`

---

### Task 4: Docs

**Files:**
- Modify: `README.md` (Quick Start block, after the `nigel review` lines)
- Modify: `CHANGELOG.md` (Unreleased/next section, matching existing entry style)

- [ ] **Step 1: README** — add to the Quick Start code block:

```bash
# Recategorize transactions non-interactively (by ID, or in bulk by filters)
nigel recategorize 185 212 --category "Software & Subscriptions"
nigel recategorize --from-category "Cost of Goods Sold" --year 2025 --category "Supplies" --dry-run
nigel recategorize --from-category "Cost of Goods Sold" --year 2025 --category "Supplies" --yes
```

Also add a Features bullet: `- **Bulk recategorization** — \`nigel recategorize\` moves transactions between categories by ID or by filters (category, date range, pattern, account, amount), with \`--dry-run\` preview and confirmation for filter-based moves`

- [ ] **Step 2: CHANGELOG** — add under the unreleased heading (create one if absent, matching the file's existing conventions): `- **`nigel recategorize`** — non-interactive bulk category reassignment by IDs or filters (`--from-category`, date ranges, `--pattern`, `--account`, amount bounds) with `--dry-run` and `--yes`; clears review flags like an in-app review`

- [ ] **Step 3: Gate + commit:** `cargo test 2>&1 | tail -3 && git add -A && git commit -m "docs: recategorize command"`
