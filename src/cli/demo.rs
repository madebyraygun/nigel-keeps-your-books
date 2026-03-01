use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate};
use rusqlite::Connection;

use crate::categorizer::categorize_transactions;
use crate::db::{get_connection, init_db};
use crate::error::Result;
use crate::settings::load_settings;

const ACCOUNT_NAME: &str = "BofA Checking";

struct DemoTxn {
    date: String,
    description: &'static str,
    amount: f64,
}

/// Recurring transactions generated every month.
struct RecurringTxn {
    day: u32,
    description: &'static str,
    amount: f64,
}

const RECURRING: &[RecurringTxn] = &[
    RecurringTxn { day: 5, description: "ADOBE CREATIVE CLOUD", amount: -54.99 },
    RecurringTxn { day: 5, description: "GITHUB INC", amount: -21.00 },
    RecurringTxn { day: 5, description: "SLACK TECHNOLOGIES", amount: -12.50 },
    RecurringTxn { day: 5, description: "GOOGLE WORKSPACE", amount: -14.40 },
    RecurringTxn { day: 8, description: "AMAZON WEB SERVICES", amount: -189.00 },
    RecurringTxn { day: 8, description: "FLYWHEEL HOSTING", amount: -89.00 },
];

/// Rotating one-off expenses — each month picks a subset based on index.
struct RotatingTxn {
    day: u32,
    description: &'static str,
    amount: f64,
}

const ROTATING: &[RotatingTxn] = &[
    RotatingTxn { day: 15, description: "CHECK 1042", amount: -2400.00 },
    RotatingTxn { day: 20, description: "VENMO PAYMENT", amount: -150.00 },
    RotatingTxn { day: 25, description: "COMCAST BUSINESS", amount: -129.99 },
    RotatingTxn { day: 28, description: "STAPLES OFFICE SUPPLY", amount: -67.23 },
    RotatingTxn { day: 7, description: "WEWORK MEMBERSHIP", amount: -450.00 },
    RotatingTxn { day: 12, description: "ZOOM VIDEO COMMUNICATIONS", amount: -14.99 },
    RotatingTxn { day: 19, description: "COSTCO WHOLESALE", amount: -29.33 },
    RotatingTxn { day: 25, description: "FEDEX SHIPPING", amount: -18.75 },
    RotatingTxn { day: 14, description: "DROPBOX BUSINESS", amount: -19.99 },
    RotatingTxn { day: 18, description: "TARGET STORE", amount: -43.67 },
];

/// Meal delivery vendors rotated across months.
const MEALS: &[(&str, &str)] = &[
    ("UBER EATS", "GRUBHUB DELIVERY"),
    ("GRUBHUB DELIVERY", "DOORDASH DELIVERY"),
    ("UBER EATS", "DOORDASH DELIVERY"),
];

/// Base income amounts for the two monthly Stripe transfers.
const INCOME_BASES: &[(f64, f64)] = &[
    (12000.0, 8500.0),
    (15000.0, 9200.0),
    (11500.0, 13800.0),
    (10200.0, 11000.0),
    (14500.0, 7800.0),
    (13000.0, 9500.0),
];

/// Base meal amounts cycled per month.
const MEAL_AMOUNTS: &[(f64, f64)] = &[
    (-32.18, -28.45),
    (-41.50, -35.72),
    (-27.90, -33.10),
    (-38.25, -24.60),
    (-29.99, -31.40),
    (-44.15, -26.80),
];

/// Clamp a day to the last valid day of the given year/month.
fn clamp_day(year: i32, month: u32, day: u32) -> u32 {
    let last_day = NaiveDate::from_ymd_opt(year, month + 1, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap())
        .pred_opt()
        .unwrap()
        .day();
    day.min(last_day)
}

fn make_date(year: i32, month: u32, day: u32) -> String {
    let d = clamp_day(year, month, day);
    format!("{year:04}-{month:02}-{d:02}")
}

/// Build 18 months of demo transactions ending at the current month.
fn generate_transactions() -> Vec<DemoTxn> {
    let today = Local::now().date_naive();
    let mut txns = Vec::new();

    for i in 0..18u32 {
        // Count backwards: i=0 is 17 months ago, i=17 is current month
        let months_ago = 17 - i;
        let target = today - chrono::Months::new(months_ago);
        let year = target.year();
        let month = target.month();
        let idx = i as usize;

        // — Income: two Stripe transfers per month —
        let (base1, base2) = INCOME_BASES[idx % INCOME_BASES.len()];
        // Small deterministic variation: +/- up to ~3% based on month index
        let vary = 1.0 + ((idx % 7) as f64 - 3.0) * 0.01;
        txns.push(DemoTxn {
            date: make_date(year, month, 3),
            description: "STRIPE TRANSFER",
            amount: (base1 * vary * 100.0).round() / 100.0,
        });
        txns.push(DemoTxn {
            date: make_date(year, month, 17),
            description: "STRIPE TRANSFER",
            amount: (base2 * vary * 100.0).round() / 100.0,
        });

        // — Recurring subscriptions & hosting —
        for r in RECURRING {
            // AWS varies slightly each month
            let amt = if r.description == "AMAZON WEB SERVICES" {
                let base = r.amount;
                let delta = ((idx % 5) as f64 - 2.0) * 3.5;
                ((base + delta) * 100.0).round() / 100.0
            } else {
                r.amount
            };
            txns.push(DemoTxn {
                date: make_date(year, month, r.day),
                description: r.description,
                amount: amt,
            });
        }

        // — Meals: two per month, rotating vendors —
        let (meal1_desc, meal2_desc) = MEALS[idx % MEALS.len()];
        let (meal1_amt, meal2_amt) = MEAL_AMOUNTS[idx % MEAL_AMOUNTS.len()];
        txns.push(DemoTxn {
            date: make_date(year, month, 12),
            description: meal1_desc,
            amount: meal1_amt,
        });
        txns.push(DemoTxn {
            date: make_date(year, month, 22),
            description: meal2_desc,
            amount: meal2_amt,
        });

        // — Rotating extras: pick 3 per month from the pool —
        for j in 0..3usize {
            let pick = (idx * 3 + j) % ROTATING.len();
            let rot = &ROTATING[pick];
            txns.push(DemoTxn {
                date: make_date(year, month, rot.day),
                description: rot.description,
                amount: rot.amount,
            });
        }

        // — Interest payment on last day of month —
        let interest = 1.50 + (idx % 5) as f64 * 0.25;
        txns.push(DemoTxn {
            date: make_date(year, month, 31),
            description: "INTEREST PAYMENT",
            amount: (interest * 100.0).round() / 100.0,
        });
    }

    txns
}

struct DemoRule {
    pattern: &'static str,
    category: &'static str,
    vendor: &'static str,
}

const RULES: &[DemoRule] = &[
    DemoRule { pattern: "STRIPE TRANSFER", category: "Client Services", vendor: "Stripe" },
    DemoRule { pattern: "ADOBE", category: "Software & Subscriptions", vendor: "Adobe" },
    DemoRule { pattern: "GITHUB", category: "Software & Subscriptions", vendor: "GitHub" },
    DemoRule { pattern: "SLACK", category: "Software & Subscriptions", vendor: "Slack" },
    DemoRule { pattern: "GOOGLE WORKSPACE", category: "Software & Subscriptions", vendor: "Google" },
    DemoRule { pattern: "AMAZON WEB SERVICES", category: "Hosting & Infrastructure", vendor: "AWS" },
    DemoRule { pattern: "FLYWHEEL", category: "Hosting & Infrastructure", vendor: "Flywheel" },
    DemoRule { pattern: "UBER EATS", category: "Meals", vendor: "Uber Eats" },
    DemoRule { pattern: "GRUBHUB", category: "Meals", vendor: "Grubhub" },
];

fn insert_demo_data(conn: &Connection) -> Result<usize> {
    let txns = generate_transactions();
    let txn_count = txns.len();

    // Create account
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution) VALUES (?1, 'checking', 'Bank of America')",
        [ACCOUNT_NAME],
    )?;
    let account_id = conn.last_insert_rowid();

    // Insert transactions — all flagged initially
    for txn in &txns {
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, is_flagged, flag_reason) \
             VALUES (?1, ?2, ?3, ?4, 1, 'No matching rule')",
            rusqlite::params![account_id, txn.date, txn.description, txn.amount],
        )?;
    }

    // Insert rules
    for rule in RULES {
        let cat_id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = ?1",
            [rule.category],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO rules (pattern, match_type, vendor, category_id, priority, is_active) \
             VALUES (?1, 'contains', ?2, ?3, 0, 1)",
            rusqlite::params![rule.pattern, rule.vendor, cat_id],
        )?;
    }

    Ok(txn_count)
}

pub fn run() -> Result<()> {
    let settings = load_settings();
    let db_path = PathBuf::from(&settings.data_dir).join("nigel.db");

    if !db_path.exists() {
        eprintln!("No database found. Run `nigel init` first.");
        std::process::exit(1);
    }

    let conn = get_connection(&db_path)?;
    init_db(&conn)?;

    // Idempotency guard
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
        [ACCOUNT_NAME],
        |r| r.get(0),
    )?;
    if exists {
        println!("Demo data already loaded (account '{}' exists).", ACCOUNT_NAME);
        return Ok(());
    }

    let txn_count = insert_demo_data(&conn)?;
    let result = categorize_transactions(&conn)?;

    println!("Demo data loaded!");
    println!("  Account:      {ACCOUNT_NAME}");
    println!("  Transactions: {txn_count}");
    println!("  Rules:        {}", RULES.len());
    println!("  Categorized:  {}", result.categorized);
    println!("  Flagged:      {}", result.still_flagged);
    println!();
    println!("Try these next:");
    println!("  nigel accounts list");
    println!("  nigel rules list");
    println!("  nigel report pnl");
    println!("  nigel report flagged");
    println!("  nigel review");

    Ok(())
}

/// Create a demo data directory with its own nigel.db and switch settings
/// to point at it. Used by the onboarding flow so the user's real books
/// stay clean.
pub fn setup_demo() -> Result<()> {
    use crate::settings::save_settings;

    let mut settings = load_settings();
    let base_dir = PathBuf::from(&settings.data_dir);
    let demo_dir = base_dir.join("demo");
    std::fs::create_dir_all(&demo_dir)?;
    std::fs::create_dir_all(demo_dir.join("exports"))?;

    let db_path = demo_dir.join("nigel.db");
    let conn = get_connection(&db_path)?;
    init_db(&conn)?;

    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
        [ACCOUNT_NAME],
        |r| r.get(0),
    )?;
    if !exists {
        insert_demo_data(&conn)?;
        categorize_transactions(&conn)?;
    }

    // Point settings at the demo directory
    settings.data_dir = demo_dir.to_string_lossy().to_string();
    save_settings(&settings)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn test_generate_transactions_count() {
        let txns = generate_transactions();
        // 18 months × 14 txns per month (2 income + 6 recurring + 2 meals + 3 rotating + 1 interest)
        assert_eq!(txns.len(), 18 * 14);
    }

    #[test]
    fn test_generate_transactions_span_current_year() {
        let txns = generate_transactions();
        let current_year = Local::now().date_naive().year();
        let year_prefix = format!("{current_year}-");
        let in_current_year = txns.iter().filter(|t| t.date.starts_with(&year_prefix)).count();
        assert!(in_current_year > 0, "should have transactions in the current year");
    }

    #[test]
    fn test_generate_transactions_span_18_months() {
        let txns = generate_transactions();
        let dates: Vec<NaiveDate> = txns
            .iter()
            .map(|t| NaiveDate::parse_from_str(&t.date, "%Y-%m-%d").unwrap())
            .collect();
        let min_date = dates.iter().min().unwrap();
        let max_date = dates.iter().max().unwrap();
        let span_months =
            (max_date.year() - min_date.year()) * 12 + max_date.month() as i32 - min_date.month() as i32;
        assert!(span_months >= 17, "transactions should span at least 17 months, got {span_months}");
    }

    #[test]
    fn test_demo_creates_data() {
        let (_dir, conn) = test_db();
        let txn_count = insert_demo_data(&conn).unwrap();
        let result = categorize_transactions(&conn).unwrap();

        let acct_count: i64 =
            conn.query_row("SELECT count(*) FROM accounts", [], |r| r.get(0)).unwrap();
        let db_txn_count: i64 =
            conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0)).unwrap();
        let rule_count: i64 =
            conn.query_row("SELECT count(*) FROM rules", [], |r| r.get(0)).unwrap();

        assert_eq!(acct_count, 1);
        assert_eq!(db_txn_count, txn_count as i64);
        assert_eq!(rule_count, RULES.len() as i64);
        assert!(result.categorized > 0, "should categorize some transactions");
        assert!(result.still_flagged > 0, "should leave some flagged");
    }

    #[test]
    fn test_demo_ytd_income_nonzero() {
        let (_dir, conn) = test_db();
        insert_demo_data(&conn).unwrap();
        categorize_transactions(&conn).unwrap();

        let current_year = Local::now().date_naive().year();
        let ytd_income: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE amount > 0 AND date LIKE ?1",
                [format!("{current_year}%")],
                |r| r.get(0),
            )
            .unwrap();
        assert!(ytd_income > 0.0, "YTD income should be non-zero, got {ytd_income}");
    }

    #[test]
    fn test_demo_idempotent() {
        let (_dir, conn) = test_db();
        insert_demo_data(&conn).unwrap();
        categorize_transactions(&conn).unwrap();

        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
                [ACCOUNT_NAME],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "account should exist after first insert");

        let txn_count_before: i64 =
            conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0)).unwrap();

        // Simulate what run() does: check guard, skip if exists
        if !exists {
            insert_demo_data(&conn).unwrap();
        }

        let txn_count_after: i64 =
            conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0)).unwrap();
        assert_eq!(txn_count_before, txn_count_after, "no duplicates on second run");
    }

    #[test]
    fn test_dates_are_valid() {
        let txns = generate_transactions();
        for txn in &txns {
            let parsed = NaiveDate::parse_from_str(&txn.date, "%Y-%m-%d");
            assert!(parsed.is_ok(), "invalid date: {}", txn.date);
        }
    }
}
