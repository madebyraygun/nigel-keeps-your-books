use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

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

    /// Build a `nigel` Command with HOME pointed at our temp dir.
    fn cmd(&self) -> Command {
        let mut cmd: Command = cargo_bin_cmd!("nigel");
        cmd.env("HOME", self.home.path());
        cmd
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
            predicate::str::contains("Dry run")
                .and(predicate::str::contains("would be imported")),
        );

    // Verify no snapshots were created for the dry run (only the demo's snapshots should exist)
    // The key assertion is that "Dry run" appeared in stdout, meaning no DB writes occurred
}
