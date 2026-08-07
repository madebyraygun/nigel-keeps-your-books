//! Captures the web UI's report fixtures from a real server response.
//!
//! The SPA's figure-parity test needs two things per report: the JSON the
//! browser would receive, and the text the CLI would export for the same
//! period. Both come from here, from one seeded database, so the numbers in
//! the browser can be compared against the numbers in `nigel report … --mode
//! export --format text` without the web test ever starting a server.
//!
//! It is an ignored test rather than a script because everything it needs —
//! the seeded database, the router, a session — already exists as test-only
//! code, and because the alternative (a shell script driving `nigel serve`)
//! would have to run `nigel init --data-dir`, which rewrites the developer's
//! real `~/.config/nigel/settings.json` and repoints their books.
//!
//! Run it deliberately, after changing a report's shape:
//!
//! ```text
//! cargo test --features serve capture_web_report_fixtures -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::server::testutil::{app_for, body_string, get_response, seeded_db};

const COMPANY: &str = "Raygun LLC";

/// The period every dated report is captured for. The seed's transactions are
/// in 2024 and 2025, and a fixed year is what keeps a committed fixture from
/// meaning something different next January.
const YEAR: &str = "2025";

struct Capture {
    /// The report's slug, which is also its filename stem.
    slug: &'static str,
    /// The query string both routes are asked with, empty for the undated ones.
    query: &'static str,
}

const CAPTURES: [Capture; 8] = [
    Capture {
        slug: "pnl",
        query: "year=2025",
    },
    Capture {
        slug: "expenses",
        query: "year=2025",
    },
    Capture {
        slug: "tax",
        query: "year=2025",
    },
    Capture {
        slug: "cashflow",
        query: "year=2025",
    },
    Capture {
        slug: "balance",
        query: "",
    },
    Capture {
        slug: "flagged",
        query: "",
    },
    Capture {
        slug: "register",
        query: "year=2025",
    },
    Capture {
        slug: "k1",
        query: "year=2025",
    },
];

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("web/apps/app/src/__fixtures__/reports")
}

/// Adds the two categories the K-1 worksheet's mapping states need.
///
/// The stock chart of accounts has `form_line` backfilled on every category, so
/// a seeded database can never produce a "needs mapping" section or an
/// auto-mapped note. These two do: an expense with no line to sit on, and
/// income that falls back to gross receipts.
fn seed_unmapped(conn: &Connection) {
    conn.execute(
        "INSERT INTO categories (name, category_type, tax_line, form_line) \
         VALUES ('Studio Sundries', 'expense', NULL, NULL)",
        [],
    )
    .expect("unmapped expense category");
    let sundries = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO categories (name, category_type, tax_line, form_line) \
         VALUES ('Workshop Fees', 'income', NULL, NULL)",
        [],
    )
    .expect("unmapped income category");
    let workshop = conn.last_insert_rowid();

    let account: i64 = conn
        .query_row("SELECT id FROM accounts LIMIT 1", [], |row| row.get(0))
        .expect("an account");

    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) \
         VALUES (?1, '2025-04-02', 'STUDIO SUNDRIES', -118.40, ?2, 'Blick')",
        rusqlite::params![account, sundries],
    )
    .expect("unmapped expense transaction");

    conn.execute(
        "INSERT INTO transactions (account_id, date, description, amount, category_id, vendor) \
         VALUES (?1, '2025-04-11', 'WORKSHOP TICKETS', 640.00, ?2, 'Eventbrite')",
        rusqlite::params![account, workshop],
    )
    .expect("unmapped income transaction");
}

fn write(name: &str, contents: &str) {
    let path = fixtures_dir().join(name);
    std::fs::write(&path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("wrote {}", path.display());
}

fn uri(kind: &str, slug: &str, query: &str, extra: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if !query.is_empty() {
        parts.push(query);
    }
    if !extra.is_empty() {
        parts.push(extra);
    }
    if parts.is_empty() {
        format!("/api/{kind}/{slug}")
    } else {
        format!("/api/{kind}/{slug}?{}", parts.join("&"))
    }
}

/// Captures a database's reports under a filename prefix, skipping any slug
/// the caller did not ask for.
async fn capture_all(
    db_path: &Path,
    prefix: &str,
    only: Option<&[&str]>,
    manifest: &mut Vec<serde_json::Value>,
) {
    let (app, token) = app_for(db_path);

    for Capture { slug, query } in CAPTURES {
        if only.is_some_and(|wanted| !wanted.contains(&slug)) {
            continue;
        }
        let report_uri = uri("reports", slug, query, "");
        let export_uri = uri("exports", slug, query, "format=text");

        let report = get_response(&app, &report_uri, &token).await;
        assert!(
            report.status().is_success(),
            "GET {report_uri} answered {}",
            report.status()
        );
        let json = body_string(report).await;

        let export = get_response(&app, &export_uri, &token).await;
        assert!(
            export.status().is_success(),
            "GET {export_uri} answered {}",
            export.status()
        );
        let text = body_string(export).await;

        let stem = format!("{prefix}{slug}");
        write(&format!("{stem}.json"), &format!("{json}\n"));
        write(&format!("{stem}.txt"), &text);

        manifest.push(serde_json::json!({
            "report": slug,
            "route": report_uri,
            "exportRoute": export_uri,
            "params": if query.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::json!({ "year": YEAR.parse::<i32>().expect("year") })
            },
            "json": format!("{stem}.json"),
            "text": format!("{stem}.txt"),
        }));
    }
}

#[tokio::test]
#[ignore = "writes fixtures into web/; run deliberately"]
async fn capture_web_report_fixtures() {
    let dir = fixtures_dir();
    std::fs::create_dir_all(&dir).expect("fixtures dir");

    let mut manifest: Vec<serde_json::Value> = Vec::new();

    let (_seed_dir, db_path) = seeded_db();
    {
        let conn = crate::db::open_connection(&db_path, None).expect("open db");
        crate::db::set_metadata(&conn, "company_name", COMPANY).expect("company name");
    }
    capture_all(&db_path, "", None, &mut manifest).await;

    // A second database for the K-1 mapping states, so the shared seed every
    // other test reads stays exactly as it is.
    let (_variant_dir, variant_path) = seeded_db();
    {
        let conn = crate::db::open_connection(&variant_path, None).expect("open variant db");
        crate::db::set_metadata(&conn, "company_name", COMPANY).expect("company name");
        seed_unmapped(&conn);
    }
    let mut variant: Vec<serde_json::Value> = Vec::new();
    capture_all(&variant_path, "needs-mapping-", Some(&["k1"]), &mut variant).await;
    manifest.extend(variant.into_iter().map(|mut entry| {
        entry["report"] = serde_json::json!("k1-needs-mapping");
        entry
    }));

    let rendered = serde_json::to_string_pretty(&serde_json::json!({
        "note": "Generated by `cargo test --features serve capture_web_report_fixtures -- --ignored`. Do not edit by hand.",
        "company": COMPANY,
        "reports": manifest,
    }))
    .expect("manifest");
    write("manifest.json", &format!("{rendered}\n"));
}
