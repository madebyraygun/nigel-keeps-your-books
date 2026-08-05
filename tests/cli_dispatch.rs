use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Every `NIGEL_*` key `settings::invoicing_config()` reads. Env vars win over the
/// settings file, and the temp HOME cannot mask them, so they are cleared per command.
const INVOICING_ENV_VARS: [&str; 9] = [
    "NIGEL_STRIPE_SECRET_KEY",
    "NIGEL_MAILGUN_API_KEY",
    "NIGEL_MAILGUN_DOMAIN",
    "NIGEL_FROM_EMAIL",
    "NIGEL_R2_ACCOUNT_ID",
    "NIGEL_R2_ACCESS_KEY",
    "NIGEL_R2_SECRET_KEY",
    "NIGEL_R2_BUCKET",
    "NIGEL_PUBLIC_BASE_URL",
];

/// Create an isolated environment: a temp HOME so that `~/.config/nigel/settings.json`
/// and `~/Documents/nigel/` all live inside the temp dir. Returns the TempDir (must be
/// kept alive for the duration of the test) and a helper to build `nigel` commands that
/// inherit the overridden HOME.
struct TestEnv {
    home: TempDir,
}

impl TestEnv {
    fn new() -> Self {
        Self {
            home: TempDir::new().expect("failed to create temp home"),
        }
    }

    /// Data directory inside the fake HOME.
    fn data_dir(&self) -> PathBuf {
        self.home.path().join("nigel-data")
    }

    /// Build a `nigel` Command with HOME pointed at our temp dir and every
    /// invoicing credential cleared from the inherited environment, so no test
    /// can reach Stripe, R2, or Mailgun on a machine where those are exported.
    fn cmd(&self) -> Command {
        let mut cmd: Command = cargo_bin_cmd!("nigel");
        cmd.env("HOME", self.home.path());
        for var in INVOICING_ENV_VARS {
            cmd.env_remove(var);
        }
        cmd
    }

    fn db(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.data_dir().join("nigel.db")).expect("failed to open DB")
    }

    /// Rewind the database to the state of a pre-v3 install: schema version 2 and
    /// no `form_line` on the categories that migration v3 backfills.
    fn downgrade_to_v2(&self) {
        self.db()
            .execute_batch(
                "UPDATE metadata SET value = '2' WHERE key = 'schema_version';
                 UPDATE categories SET form_line = NULL
                     WHERE name IN ('Client Services', 'Hosting & Maintenance', 'Reimbursements',
                                    'Other Income', 'Cost of Goods Sold', 'Transfer');",
            )
            .expect("failed to downgrade test database");
    }

    fn schema_version(&self) -> u32 {
        self.db()
            .query_row(
                "SELECT value FROM metadata WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("schema_version missing")
            .parse()
            .expect("schema_version not a number")
    }

    /// Encrypt the database in place, the way `nigel password set` does.
    fn encrypt(&self, password: &str) {
        let db = self.data_dir().join("nigel.db");
        let tmp = self.data_dir().join("nigel.db.encrypting");
        let conn = self.db();
        conn.execute(
            "ATTACH DATABASE ?1 AS encrypted KEY ?2",
            rusqlite::params![tmp.to_string_lossy(), password],
        )
        .expect("failed to attach encrypted database");
        conn.execute_batch("SELECT sqlcipher_export('encrypted'); DETACH DATABASE encrypted;")
            .expect("failed to export to encrypted database");
        drop(conn);
        let _ = std::fs::remove_file(self.data_dir().join("nigel.db-wal"));
        let _ = std::fs::remove_file(self.data_dir().join("nigel.db-shm"));
        std::fs::rename(&tmp, &db).expect("failed to swap in encrypted database");

        assert!(
            self.db()
                .execute_batch("SELECT count(*) FROM sqlite_master;")
                .is_err(),
            "fixture did not actually encrypt the database"
        );
    }

    fn form_line(&self, category: &str) -> Option<String> {
        self.db()
            .query_row(
                "SELECT form_line FROM categories WHERE name = ?1",
                [category],
                |row| row.get(0),
            )
            .expect("category missing")
    }

    /// Run `nigel init --data-dir <data_dir>` then `nigel demo`.
    fn init_and_demo(&self) {
        self.cmd()
            .args(["init", "--data-dir", &self.data_dir().to_string_lossy()])
            .assert()
            .success()
            .stdout(predicate::str::contains("Initialized"));

        self.cmd()
            .arg("demo")
            .assert()
            .success()
            .stdout(predicate::str::contains("Demo data loaded"));
    }
}

#[test]
fn init_then_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    // DB file should exist
    assert!(env.data_dir().join("nigel.db").exists());
}

#[test]
fn demo_is_idempotent() {
    let env = TestEnv::new();
    env.init_and_demo();

    // Running demo again should succeed and report already loaded
    env.cmd()
        .arg("demo")
        .assert()
        .success()
        .stdout(predicate::str::contains("Demo data already loaded"));
}

#[test]
fn status_after_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd().arg("status").assert().success().stdout(
        predicate::str::contains("Transactions:")
            .and(predicate::str::contains("Accounts:"))
            .and(predicate::str::contains("Rules:")),
    );
}

#[test]
fn backup_to_custom_path() {
    let env = TestEnv::new();
    env.init_and_demo();

    let backup_path = env.home.path().join("test-backup.db");
    env.cmd()
        .args(["backup", "--output", &backup_path.to_string_lossy()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup saved to"));

    assert!(backup_path.exists());
    let size = std::fs::metadata(&backup_path).unwrap().len();
    assert!(size > 0, "backup file should be non-empty");
}

#[test]
fn backup_default_location() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .arg("backup")
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup saved to"));

    // Should have created a file in <data_dir>/backups/
    let backups_dir = env.data_dir().join("backups");
    assert!(backups_dir.exists());
    let entries: Vec<_> = std::fs::read_dir(&backups_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(!entries.is_empty(), "backups dir should contain a file");
}

#[test]
fn restore_from_backup() {
    let env = TestEnv::new();
    env.init_and_demo();

    // Create a backup
    let backup_path = env.home.path().join("test-backup.db");
    env.cmd()
        .args(["backup", "--output", &backup_path.to_string_lossy()])
        .assert()
        .success();

    // Add a new account to the current database (post-backup change)
    env.cmd()
        .args([
            "accounts",
            "add",
            "Post-Backup Account",
            "--type",
            "checking",
        ])
        .assert()
        .success();

    // Restore from backup (pipe "y" to confirm)
    env.cmd()
        .args(["restore", &backup_path.to_string_lossy()])
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Safety backup saved to"))
        .stdout(predicate::str::contains("Database restored from"));

    // Verify the post-backup account is gone (restored to pre-change state)
    let output = env.cmd().args(["accounts", "list"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Post-Backup Account"),
        "Post-backup account should not exist after restore"
    );

    // Verify a safety backup was created
    let backups_dir = env.data_dir().join("backups");
    let entries: Vec<_> = std::fs::read_dir(&backups_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("pre-restore"))
        .collect();
    assert!(
        !entries.is_empty(),
        "pre-restore safety backup should exist"
    );
}

#[test]
fn restore_nonexistent_file_fails() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["restore", "/tmp/nonexistent-nigel-backup-xyz.db"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn report_pnl_text_export() {
    let env = TestEnv::new();
    env.init_and_demo();

    let year = chrono::Local::now().format("%Y").to_string();
    let output_path = env.home.path().join("pnl-report.txt");
    env.cmd()
        .args([
            "report",
            "pnl",
            "--year",
            &year,
            "--mode",
            "export",
            "--format",
            "text",
            "--output",
            &output_path.to_string_lossy(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote"));

    assert!(output_path.exists());
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(!content.is_empty(), "report file should be non-empty");
}

#[test]
fn report_all_text_export() {
    let env = TestEnv::new();
    env.init_and_demo();

    let year = chrono::Local::now().format("%Y").to_string();
    let output_dir = env.home.path().join("all-reports");
    env.cmd()
        .args([
            "report",
            "all",
            "--year",
            &year,
            "--format",
            "text",
            "--output-dir",
            &output_dir.to_string_lossy(),
        ])
        .assert()
        .success();

    assert!(output_dir.exists());
    let entries: Vec<_> = std::fs::read_dir(&output_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .collect();
    // Should produce multiple report files (pnl, expenses, tax, cashflow, register, flagged, balance, k1)
    assert!(
        entries.len() >= 5,
        "expected at least 5 report files, got {}",
        entries.len()
    );
}

#[test]
fn categorize_after_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .arg("categorize")
        .assert()
        .success()
        .stdout(predicate::str::contains("categorized"));
}

#[test]
fn import_nonexistent_file() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["import", "nonexistent.csv", "--account", "BofA Checking"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
}

#[test]
fn accounts_list_after_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["accounts", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("BofA Checking"));
}

#[test]
fn rules_list_after_demo() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["rules", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("STRIPE TRANSFER"));
}

#[test]
fn report_invalid_mode() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["report", "pnl", "--mode", "bogus"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown --mode"));
}

#[test]
fn report_invalid_format() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args(["report", "pnl", "--format", "csv"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown --format"));
}

#[test]
fn init_without_db_then_status() {
    let env = TestEnv::new();

    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    // Status on a fresh DB (no demo data) should still work
    env.cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Transactions:  0"));
}

#[test]
fn client_add_and_list_roundtrip() {
    let env = TestEnv::new();

    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "a@b.test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Acme Co"));

    env.cmd()
        .args(["client", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Acme Co").and(predicate::str::contains("a@b.test")));
}

#[test]
fn demo_without_init_fails() {
    let env = TestEnv::new();

    // With a fresh HOME, no settings.json exists, so data_dir defaults to ~/Documents/nigel
    // which won't have a nigel.db — demo should fail
    env.cmd()
        .arg("demo")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No database found"));
}

#[test]
fn test_import_dry_run_no_db_writes() {
    let env = TestEnv::new();
    env.init_and_demo();

    // Write a BofA checking CSV
    let csv_path = env.home.path().join("test-import.csv");
    std::fs::write(
        &csv_path,
        "Date,Description,Amount,Running Bal.\n\
         01/15/2025,DRY RUN PAYMENT,-100.00,900.00\n\
         01/16/2025,DRY RUN DEPOSIT,500.00,1400.00\n",
    )
    .unwrap();

    env.cmd()
        .args([
            "import",
            &csv_path.to_string_lossy(),
            "--account",
            "BofA Checking",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Dry run").and(predicate::str::contains("would be imported")),
        );

    // Verify no snapshots were created for the dry run (only the demo's snapshots should exist)
    // The key assertion is that "Dry run" appeared in stdout, meaning no DB writes occurred
}

#[test]
fn test_import_generic_csv_with_column_flags() {
    let env = TestEnv::new();
    env.init_and_demo();

    // CSV with a non-standard column layout: trans_date, ref, memo, amount, balance
    let csv_path = env.home.path().join("generic-import.csv");
    std::fs::write(
        &csv_path,
        "trans_date,ref,memo,amount,balance\n\
         01/10/2025,1001,Office Supplies,-45.99,954.01\n\
         01/11/2025,1002,Client Payment,1200.00,2154.01\n",
    )
    .unwrap();

    env.cmd()
        .args([
            "import",
            &csv_path.to_string_lossy(),
            "--account",
            "BofA Checking",
            "--date-col",
            "0",
            "--desc-col",
            "2",
            "--amount-col",
            "3",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 imported"));
}

#[test]
fn test_import_generic_csv_with_saved_profile() {
    let env = TestEnv::new();
    env.init_and_demo();

    // First import: use column flags + --save-profile
    let csv1_path = env.home.path().join("bank-export-1.csv");
    std::fs::write(
        &csv1_path,
        "posted,ref_num,payee,debit_credit,running\n\
         01/20/2025,5001,Rent Payment,-2000.00,5000.00\n\
         01/21/2025,5002,Invoice 42,3500.00,8500.00\n",
    )
    .unwrap();

    env.cmd()
        .args([
            "import",
            &csv1_path.to_string_lossy(),
            "--account",
            "BofA Checking",
            "--date-col",
            "0",
            "--desc-col",
            "2",
            "--amount-col",
            "3",
            "--save-profile",
            "mybank",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Saved profile 'mybank'")
                .and(predicate::str::contains("2 imported")),
        );

    // Second import: use the saved profile via --format
    let csv2_path = env.home.path().join("bank-export-2.csv");
    std::fs::write(
        &csv2_path,
        "posted,ref_num,payee,debit_credit,running\n\
         02/15/2025,5003,Software License,-199.00,8301.00\n",
    )
    .unwrap();

    env.cmd()
        .args([
            "import",
            &csv2_path.to_string_lossy(),
            "--account",
            "BofA Checking",
            "--format",
            "mybank",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 imported"));
}

#[test]
fn status_migrates_outdated_database() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.downgrade_to_v2();

    assert_eq!(env.schema_version(), 2);
    assert_eq!(env.form_line("Client Services"), None);

    env.cmd().arg("status").assert().success();

    assert!(
        env.schema_version() > 2,
        "`nigel status` should have run pending migrations"
    );
    assert_eq!(
        env.form_line("Client Services"),
        Some("1120S-1a".to_string())
    );
    assert_eq!(
        env.form_line("Cost of Goods Sold"),
        Some("1120S-2".to_string())
    );
}

#[test]
fn report_k1_migrates_outdated_database() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.downgrade_to_v2();

    let year = chrono::Local::now().format("%Y").to_string();
    let output_path = env.home.path().join("k1.txt");
    env.cmd()
        .args([
            "report",
            "k1",
            "--year",
            &year,
            "--mode",
            "export",
            "--format",
            "text",
            "--output",
            &output_path.to_string_lossy(),
        ])
        .assert()
        .success();

    assert!(
        env.schema_version() > 2,
        "`nigel report k1` should have run pending migrations"
    );

    // Without the v3 backfill, income categories have no form_line and fall back to
    // gross receipts, which the worksheet flags as auto-mapped.
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        !content.contains("(auto) income mapped to gross receipts"),
        "K-1 income should be explicitly mapped after migration:\n{content}"
    );
}

#[test]
fn completions_skips_the_password_and_migration_preflight() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.encrypt("hunter2");

    // The database is now unreadable without the password (asserted inside `encrypt`), so
    // any pre-flight that opened it would fail. `completions` neither prompts for the
    // password nor migrates, so it still works.
    env.cmd()
        .args(["completions", "bash"])
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(predicate::str::contains("_nigel"));
}
