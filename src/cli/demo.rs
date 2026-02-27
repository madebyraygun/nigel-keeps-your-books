use std::path::PathBuf;

use rusqlite::Connection;

use crate::categorizer::categorize_transactions;
use crate::db::{get_connection, init_db};
use crate::error::Result;
use crate::settings::load_settings;

const ACCOUNT_NAME: &str = "BofA Checking";

struct DemoTxn {
    date: &'static str,
    description: &'static str,
    amount: f64,
}

const TRANSACTIONS: &[DemoTxn] = &[
    // ── January 2025 ──
    DemoTxn { date: "2025-01-03", description: "STRIPE TRANSFER", amount: 12000.00 },
    DemoTxn { date: "2025-01-17", description: "STRIPE TRANSFER", amount: 8500.00 },
    DemoTxn { date: "2025-01-05", description: "ADOBE CREATIVE CLOUD", amount: -54.99 },
    DemoTxn { date: "2025-01-05", description: "GITHUB INC", amount: -21.00 },
    DemoTxn { date: "2025-01-05", description: "SLACK TECHNOLOGIES", amount: -12.50 },
    DemoTxn { date: "2025-01-05", description: "GOOGLE WORKSPACE", amount: -14.40 },
    DemoTxn { date: "2025-01-08", description: "AMAZON WEB SERVICES", amount: -189.43 },
    DemoTxn { date: "2025-01-08", description: "FLYWHEEL HOSTING", amount: -89.00 },
    DemoTxn { date: "2025-01-12", description: "UBER EATS", amount: -32.18 },
    DemoTxn { date: "2025-01-22", description: "GRUBHUB DELIVERY", amount: -28.45 },
    DemoTxn { date: "2025-01-15", description: "CHECK 1042", amount: -2400.00 },
    DemoTxn { date: "2025-01-20", description: "VENMO PAYMENT", amount: -150.00 },
    DemoTxn { date: "2025-01-25", description: "COMCAST BUSINESS", amount: -129.99 },
    DemoTxn { date: "2025-01-28", description: "STAPLES OFFICE SUPPLY", amount: -67.23 },
    DemoTxn { date: "2025-01-31", description: "INTEREST PAYMENT", amount: 2.14 },
    // ── February 2025 ──
    DemoTxn { date: "2025-02-03", description: "STRIPE TRANSFER", amount: 15000.00 },
    DemoTxn { date: "2025-02-18", description: "STRIPE TRANSFER", amount: 9200.00 },
    DemoTxn { date: "2025-02-05", description: "ADOBE CREATIVE CLOUD", amount: -54.99 },
    DemoTxn { date: "2025-02-05", description: "GITHUB INC", amount: -21.00 },
    DemoTxn { date: "2025-02-05", description: "SLACK TECHNOLOGIES", amount: -12.50 },
    DemoTxn { date: "2025-02-05", description: "GOOGLE WORKSPACE", amount: -14.40 },
    DemoTxn { date: "2025-02-10", description: "AMAZON WEB SERVICES", amount: -195.87 },
    DemoTxn { date: "2025-02-10", description: "FLYWHEEL HOSTING", amount: -89.00 },
    DemoTxn { date: "2025-02-14", description: "UBER EATS", amount: -41.50 },
    DemoTxn { date: "2025-02-20", description: "GRUBHUB DELIVERY", amount: -35.72 },
    DemoTxn { date: "2025-02-07", description: "WEWORK MEMBERSHIP", amount: -450.00 },
    DemoTxn { date: "2025-02-12", description: "ZOOM VIDEO COMMUNICATIONS", amount: -14.99 },
    DemoTxn { date: "2025-02-19", description: "DOORDASH DELIVERY", amount: -29.33 },
    DemoTxn { date: "2025-02-25", description: "FEDEX SHIPPING", amount: -18.75 },
    DemoTxn { date: "2025-02-28", description: "INTEREST PAYMENT", amount: 1.87 },
    // ── March 2025 ──
    DemoTxn { date: "2025-03-04", description: "STRIPE TRANSFER", amount: 11500.00 },
    DemoTxn { date: "2025-03-19", description: "STRIPE TRANSFER", amount: 13800.00 },
    DemoTxn { date: "2025-03-05", description: "ADOBE CREATIVE CLOUD", amount: -54.99 },
    DemoTxn { date: "2025-03-05", description: "GITHUB INC", amount: -21.00 },
    DemoTxn { date: "2025-03-05", description: "SLACK TECHNOLOGIES", amount: -12.50 },
    DemoTxn { date: "2025-03-05", description: "GOOGLE WORKSPACE", amount: -14.40 },
    DemoTxn { date: "2025-03-09", description: "AMAZON WEB SERVICES", amount: -201.15 },
    DemoTxn { date: "2025-03-09", description: "FLYWHEEL HOSTING", amount: -89.00 },
    DemoTxn { date: "2025-03-11", description: "UBER EATS", amount: -27.90 },
    DemoTxn { date: "2025-03-21", description: "GRUBHUB DELIVERY", amount: -33.10 },
    DemoTxn { date: "2025-03-14", description: "DROPBOX BUSINESS", amount: -19.99 },
    DemoTxn { date: "2025-03-18", description: "TARGET STORE", amount: -43.67 },
    DemoTxn { date: "2025-03-26", description: "COMCAST BUSINESS", amount: -129.99 },
    DemoTxn { date: "2025-03-31", description: "INTEREST PAYMENT", amount: 2.31 },
];

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

fn insert_demo_data(conn: &Connection) -> Result<()> {
    // Create account
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution) VALUES (?1, 'checking', 'Bank of America')",
        [ACCOUNT_NAME],
    )?;
    let account_id = conn.last_insert_rowid();

    // Insert transactions — all flagged initially
    for txn in TRANSACTIONS {
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

    Ok(())
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

    insert_demo_data(&conn)?;
    let result = categorize_transactions(&conn)?;

    println!("Demo data loaded!");
    println!("  Account:      {ACCOUNT_NAME}");
    println!("  Transactions: {}", TRANSACTIONS.len());
    println!("  Rules:        {}", RULES.len());
    println!("  Categorized:  {}", result.categorized);
    println!("  Flagged:      {}", result.still_flagged);
    println!();
    println!("Try these next:");
    println!("  nigel accounts list");
    println!("  nigel rules list");
    println!("  nigel report pnl --year 2025");
    println!("  nigel report flagged");
    println!("  nigel review");

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
    fn test_demo_creates_data() {
        let (_dir, conn) = test_db();
        insert_demo_data(&conn).unwrap();
        let result = categorize_transactions(&conn).unwrap();

        let acct_count: i64 =
            conn.query_row("SELECT count(*) FROM accounts", [], |r| r.get(0)).unwrap();
        let txn_count: i64 =
            conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0)).unwrap();
        let rule_count: i64 =
            conn.query_row("SELECT count(*) FROM rules", [], |r| r.get(0)).unwrap();

        assert_eq!(acct_count, 1);
        assert_eq!(txn_count, TRANSACTIONS.len() as i64);
        assert_eq!(rule_count, RULES.len() as i64);
        assert!(result.categorized > 0, "should categorize some transactions");
        assert!(result.still_flagged > 0, "should leave some flagged");
    }

    #[test]
    fn test_demo_idempotent() {
        let (_dir, conn) = test_db();
        insert_demo_data(&conn).unwrap();
        categorize_transactions(&conn).unwrap();

        // Check guard condition
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
                [ACCOUNT_NAME],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "account should exist after first insert");

        // Second insert should fail (unique constraint) if attempted — our guard prevents it
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
}
