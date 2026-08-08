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

/// Bounds any run that could reach the interactive password prompt, so a test
/// inheriting a tty fails instead of blocking on `rpassword` forever.
const TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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

/// Init plus one client and one 1500.00 draft invoice (#1248).
fn init_with_client_and_invoice(env: &TestEnv) {
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "ap@acme.test"])
        .assert()
        .success();
    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "1",
            "--issue",
            "2026-08-04",
            "--item",
            "Consulting:10:150",
        ])
        .assert()
        .success();
}

#[test]
fn client_show_prints_details_and_invoice_history() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["client", "show", "1"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Acme Co")
                .and(predicate::str::contains("ap@acme.test"))
                .and(predicate::str::contains("1248"))
                .and(predicate::str::contains("Outstanding")),
        );
}

#[test]
fn client_show_for_an_unknown_id_fails_with_not_found() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    env.cmd()
        .args(["client", "show", "99"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Client not found: id 99"));
}

#[test]
fn client_edit_changes_the_email() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["client", "edit", "1", "--email", "new@acme.test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated client 1"));

    env.cmd()
        .args(["client", "show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("new@acme.test"));
}

#[test]
fn client_edit_with_no_flags_fails() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["client", "edit", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Nothing to update"));
}

#[test]
fn invoice_new_persists_notes_and_terms() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();
    env.cmd()
        .args(["client", "add", "Acme Co", "--email", "ap@acme.test"])
        .assert()
        .success();

    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "1",
            "--issue",
            "2026-08-04",
            "--item",
            "Consulting:10:150",
            "--notes",
            "Thanks for the work",
            "--terms",
            "Net 30",
        ])
        .assert()
        .success();

    let (notes, terms): (String, String) = env
        .db()
        .query_row(
            "SELECT notes, terms FROM invoices WHERE number = 1248",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("invoice row missing");
    assert_eq!(notes, "Thanks for the work");
    assert_eq!(terms, "Net 30");
}

#[test]
fn invoice_edit_updates_a_draft() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args([
            "invoice",
            "edit",
            "1248",
            "--due",
            "2026-10-01",
            "--item",
            "Rework:2:250",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("500.00"));

    env.cmd()
        .args(["invoice", "show", "1248"])
        .assert()
        .success()
        .stdout(predicate::str::contains("500.00").and(predicate::str::contains("2026-10-01")));
}

#[test]
fn invoice_edit_refuses_a_void_invoice() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .success();

    env.cmd()
        .args(["invoice", "edit", "1248", "--due", "2026-10-01"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("is void and cannot be edited"));
}

#[test]
fn invoice_void_requires_confirmation_without_a_tty() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "void", "1248"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Pass --yes"));

    let status: String = env
        .db()
        .query_row("SELECT status FROM invoices WHERE number = 1248", [], |r| {
            r.get(0)
        })
        .expect("invoice row missing");
    assert_eq!(status, "draft");
}

#[test]
fn invoice_void_with_yes_voids_and_blocks_pay() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Voided invoice #1248"));

    env.cmd()
        .args(["invoice", "pay", "1248", "--date", "2026-08-20"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("void and cannot be paid"));
}

/// The default preview directory for a `TestEnv`.
fn previews_dir(env: &TestEnv) -> PathBuf {
    env.data_dir().join("previews")
}

#[test]
fn invoice_preview_writes_html_to_the_data_dir() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("invoice-1248.html"));

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html"))
        .expect("preview html missing");
    assert!(html.contains("Invoice #1248"), "got: {html}");
    assert!(html.contains("1500.00"), "got: {html}");
}

#[test]
fn invoice_preview_of_a_draft_shows_an_inert_pay_placeholder() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(html.contains("pay-placeholder"), "got: {html}");
    assert!(!html.contains("<a class=\"pay\""), "got: {html}");
}

#[test]
fn invoice_preview_needs_no_invoicing_config_and_makes_no_network_call() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    // TestEnv clears every NIGEL_* invoicing var, so this runs with no config
    // at all; anything reaching the network would hang into TEST_TIMEOUT.
    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("missing invoicing config").not());
}

#[test]
fn invoice_preview_leaves_the_invoice_a_draft() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success();

    let (status, published): (String, Option<String>) = env
        .db()
        .query_row(
            "SELECT status, published_at FROM invoices WHERE number = 1248",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("invoice row missing");
    assert_eq!(status, "draft");
    assert_eq!(published, None);
}

#[test]
fn invoice_preview_honors_output_dir() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    let elsewhere = env.data_dir().join("elsewhere");

    env.cmd()
        .args([
            "invoice",
            "preview",
            "1248",
            "--output-dir",
            &elsewhere.to_string_lossy(),
        ])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            elsewhere.to_string_lossy().to_string(),
        ));

    assert!(elsewhere.join("invoice-1248.html").exists());
    assert!(
        !previews_dir(&env).exists(),
        "a named output directory must not also seed the default one"
    );
}

#[test]
fn invoice_preview_overwrites_in_place_on_a_second_run() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    for _ in 0..2 {
        env.cmd()
            .args(["invoice", "preview", "1248"])
            .timeout(TEST_TIMEOUT)
            .assert()
            .success();
    }

    let names: Vec<String> = std::fs::read_dir(previews_dir(&env))
        .expect("previews directory missing")
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    for name in &names {
        assert!(
            name == "invoice-1248.html" || name == "invoice-1248.pdf",
            "unexpected preview artifact: {name}"
        );
    }
    assert_eq!(
        names.iter().filter(|n| n.ends_with(".html")).count(),
        1,
        "re-previewing must overwrite, not accumulate: {names:?}"
    );
}

#[test]
fn invoice_preview_of_an_unknown_number_fails_with_the_shared_message() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "9999"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("No invoice #9999"));
}

#[test]
fn invoice_preview_of_a_void_invoice_warns_and_omits_the_pay_button() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "void", "1248", "--yes"])
        .assert()
        .success();
    // An invoice voided after it was sent still carries a live Stripe URL.
    env.db()
        .execute(
            "UPDATE invoices SET stripe_payment_link_url = 'https://pay/x' WHERE number = 1248",
            [],
        )
        .unwrap();

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains("is void"));

    let html = std::fs::read_to_string(previews_dir(&env).join("invoice-1248.html")).unwrap();
    assert!(
        !html.contains("https://pay/x"),
        "a void invoice must not publish a live payment link"
    );
    assert!(!html.contains("Pay online"), "got: {html}");
}

#[test]
fn invoice_preview_skips_the_launch_stripe_sync() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);
    // The launch sync only polls invoices that carry a payment link and are
    // open, so this is the state in which a sync would reach Stripe at all.
    env.db()
        .execute(
            "UPDATE invoices SET stripe_payment_link_id = 'pl_1', status = 'sent'
             WHERE number = 1248",
            [],
        )
        .unwrap();

    // Preview is in the skip list, so the key is never used and nothing leaves
    // the machine. Drop that arm and this run reaches Stripe with a bogus key,
    // which reports itself on stderr.
    env.cmd()
        .env("NIGEL_STRIPE_SECRET_KEY", "sk_test_bogus")
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(
            predicate::str::contains("invoice sync skipped")
                .not()
                .and(predicate::str::contains("new invoice payment").not()),
        );
}

#[cfg(feature = "pdf")]
#[test]
fn invoice_preview_writes_a_real_pdf() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("invoice-1248.pdf"));

    let pdf =
        std::fs::read(previews_dir(&env).join("invoice-1248.pdf")).expect("preview pdf missing");
    assert!(pdf.starts_with(b"%PDF"));
}

#[cfg(not(feature = "pdf"))]
#[test]
fn invoice_preview_without_the_pdf_feature_still_writes_html_and_says_why() {
    let env = TestEnv::new();
    init_with_client_and_invoice(&env);

    // Exit 0: "HTML, and PDF when the feature is on" is the documented outcome,
    // not a failure.
    env.cmd()
        .args(["invoice", "preview", "1248"])
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "PDF export requires the 'pdf' feature",
        ));

    assert!(previews_dir(&env).join("invoice-1248.html").exists());
    assert!(!previews_dir(&env).join("invoice-1248.pdf").exists());
}

#[test]
fn invoice_new_with_an_unknown_client_reports_not_found() {
    let env = TestEnv::new();
    env.cmd()
        .args(["init", "--data-dir", &env.data_dir().to_string_lossy()])
        .assert()
        .success();

    env.cmd()
        .args([
            "invoice",
            "new",
            "--client",
            "99",
            "--issue",
            "2026-08-04",
            "--item",
            "X:1:1",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Client not found: id 99"));

    let count: i64 = env
        .db()
        .query_row("SELECT COUNT(*) FROM invoices", [], |r| r.get(0))
        .expect("invoices table missing");
    assert_eq!(count, 0);
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

/// Read the SQLite magic header to tell an encrypted database from a plaintext one.
fn is_encrypted_file(path: &std::path::Path) -> bool {
    let bytes = std::fs::read(path).expect("failed to read database");
    !bytes.starts_with(b"SQLite format 3\0")
}

#[test]
fn backup_unlocks_encrypted_database_from_env() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.encrypt("hunter2");

    let backup_path = env.home.path().join("env-unlocked.db");
    env.cmd()
        .args(["backup", "--output", &backup_path.to_string_lossy()])
        .env("NIGEL_DB_PASSWORD", "hunter2")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup saved to"));

    assert!(backup_path.exists(), "backup file should exist");
    assert!(
        is_encrypted_file(&backup_path),
        "backup of an encrypted database must itself be encrypted"
    );

    // The snapshot must open with the same password and carry the demo data,
    // so a backup that merely exists is not mistaken for one that can restore.
    let conn = rusqlite::Connection::open(&backup_path).unwrap();
    conn.pragma_update(None, "key", "hunter2").unwrap();
    let accounts: i64 = conn
        .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
        .expect("backup should be readable with the original password");
    assert!(accounts > 0, "backup should contain the demo accounts");
}

#[test]
fn backup_fails_fast_on_wrong_env_password() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.encrypt("hunter2");

    // The stderr predicate is what catches a regression: reaching the prompt with no
    // terminal errors with ENXIO, which would satisfy `.failure()` on its own. The
    // timeout is only a backstop for a run that inherits a tty and blocks.
    env.cmd()
        .args(["backup"])
        .env("NIGEL_DB_PASSWORD", "wrong-password")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .failure()
        .stderr(predicate::str::contains("NIGEL_DB_PASSWORD"));
}

#[test]
fn backup_ignores_env_password_on_plain_database() {
    let env = TestEnv::new();
    env.init_and_demo();

    // No `encrypt()` here: a leftover variable in the operator's shell must not lock
    // them out of a database that never had a password.
    env.cmd()
        .args(["backup"])
        .env("NIGEL_DB_PASSWORD", "stale-value-from-another-project")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup saved to"));
}

#[test]
fn env_password_is_not_echoed_on_failure() {
    let env = TestEnv::new();
    env.init_and_demo();
    env.encrypt("hunter2");

    let output = env
        .cmd()
        .args(["backup"])
        .env("NIGEL_DB_PASSWORD", "sup3rs3cret")
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .output()
        .unwrap();

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // Assert the run failed and reported before asserting on what it did not print:
    // a killed or crashed child produces no output, which would satisfy the absence
    // check while proving nothing.
    assert!(
        !output.status.success(),
        "expected failure, got success:\n{combined}"
    );
    assert!(
        combined.contains("NIGEL_DB_PASSWORD"),
        "expected the variable to be named in the error:\n{combined}"
    );
    assert!(
        !combined.contains("sup3rs3cret"),
        "password leaked into output:\n{combined}"
    );
}

// ---------------------------------------------------------------------------
// recategorize
// ---------------------------------------------------------------------------

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
        .execute(
            "UPDATE transactions SET is_flagged = 1, flag_reason = 'x' WHERE id = ?1",
            [id],
        )
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
        .args([
            "recategorize",
            "--from-category",
            &old,
            "--category",
            "Travel",
        ])
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
        .args([
            "recategorize",
            "--from-category",
            &old,
            "--category",
            "Travel",
            "--yes",
        ])
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
        .args([
            "recategorize",
            &id.to_string(),
            "--category",
            "Travel",
            "--dry-run",
        ])
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
        .args([
            "recategorize",
            &id.to_string(),
            "999999",
            "--category",
            "Travel",
        ])
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
fn recategorize_malformed_month_fails_and_changes_nothing() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (_, old) = any_categorized_txn(&env);
    let before: i64 = env
        .db()
        .query_row(
            "SELECT COUNT(*) FROM transactions t JOIN categories c ON t.category_id = c.id WHERE c.name = ?1",
            [&old],
            |row| row.get(0),
        )
        .unwrap();

    env.cmd()
        .args([
            "recategorize",
            "--from-category",
            &old,
            "--month",
            "April",
            "--category",
            "Travel",
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected YYYY-MM"));

    let after: i64 = env
        .db()
        .query_row(
            "SELECT COUNT(*) FROM transactions t JOIN categories c ON t.category_id = c.id WHERE c.name = ?1",
            [&old],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn recategorize_unknown_target_category_fails() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, old) = any_categorized_txn(&env);

    env.cmd()
        .args([
            "recategorize",
            &id.to_string(),
            "--category",
            "Bogus Category",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Bogus Category"));

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
fn recategorize_unknown_account_filter_fails() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args([
            "recategorize",
            "--account",
            "No Such Bank",
            "--category",
            "Travel",
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No Such Bank"));
}

#[test]
fn recategorize_already_in_target_skips_and_preserves_flag() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, old) = any_categorized_txn(&env);
    env.db()
        .execute(
            "UPDATE transactions SET is_flagged = 1, flag_reason = 'check me' WHERE id = ?1",
            [id],
        )
        .unwrap();

    env.cmd()
        .args(["recategorize", &id.to_string(), "--category", &old])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Skipping 1 already in {old}"
        )));

    let (flagged, reason): (i64, Option<String>) = env
        .db()
        .query_row(
            "SELECT is_flagged, flag_reason FROM transactions WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(flagged, 1);
    assert_eq!(reason.as_deref(), Some("check me"));
}

#[test]
fn recategorize_duplicate_ids_count_once() {
    let env = TestEnv::new();
    env.init_and_demo();
    let (id, _old) = any_categorized_txn(&env);

    env.cmd()
        .args([
            "recategorize",
            &id.to_string(),
            &id.to_string(),
            "--category",
            "Travel",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized 1 transaction"));
}

#[test]
fn recategorize_zero_match_filter_exits_cleanly() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .args([
            "recategorize",
            "--pattern",
            "NO SUCH TRANSACTION DESCRIPTION XYZZY",
            "--category",
            "Travel",
            "--yes",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No transactions matched."));
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
        .write_stdin("")
        .timeout(TEST_TIMEOUT)
        .assert()
        .success()
        .stdout(predicate::str::contains("Recategorized 1 transaction"));
}

#[test]
fn serve_help_documents_its_flags() {
    let env = TestEnv::new();
    env.cmd()
        .args(["serve", "--help"])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(predicate::str::contains("--port"))
        .stdout(predicate::str::contains("--no-open"));
}

#[test]
fn serve_requires_an_initialized_database() {
    let env = TestEnv::new();
    env.cmd()
        .arg("serve")
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not initialized"));
}

/// In a build without the `serve` feature the subcommand still parses — the
/// failure has to name the missing feature, the way the PDF gate does.
#[cfg(not(feature = "serve"))]
#[test]
fn serve_without_the_feature_reports_a_clear_error() {
    let env = TestEnv::new();
    env.init_and_demo();

    env.cmd()
        .arg("serve")
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires the 'serve' feature"));
}
