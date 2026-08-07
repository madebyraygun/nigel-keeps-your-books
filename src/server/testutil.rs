//! Shared scaffolding for the server tests: temporary databases, a router with
//! a valid session, and JSON request helpers.
//!
//! These tests move the process-global database password around, so the suite
//! runs under `--test-threads=1` (see `db.rs`); every constructor here clears
//! the global first so a test never inherits the previous one's key.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::response::Response;
use axum::Router;
use rusqlite::Connection;
use tower::ServiceExt;

use super::auth;
use super::state::AppState;

pub const HOST: &str = "127.0.0.1:5731";
pub const PASSWORD: &str = "correct horse battery staple";

/// A router with no database behind it, for tests that never reach one.
pub fn test_app() -> (Router, String) {
    let token = auth::generate_token();
    let state = AppState::new(PathBuf::from("/nonexistent/nigel.db"), token.clone());
    (super::build_router(state), token)
}

pub fn get_request(uri: &str) -> axum::http::request::Builder {
    Request::builder().uri(uri).header(header::HOST, HOST)
}

pub async fn body_string(response: Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    String::from_utf8(bytes.to_vec()).expect("utf-8 body")
}

pub fn content_type(response: &Response) -> String {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

pub async fn json_body(response: Response) -> serde_json::Value {
    serde_json::from_str(&body_string(response).await).expect("json body")
}

/// A data directory holding an initialized, unencrypted database.
pub fn temp_db() -> (tempfile::TempDir, PathBuf) {
    crate::db::set_db_password(None);
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("nigel.db");
    let conn = crate::db::open_connection(&db_path, None).expect("open db");
    crate::db::init_db(&conn).expect("init db");
    (dir, db_path)
}

pub fn encrypt(db_path: &Path) {
    crate::cli::password::encrypt_database(db_path, PASSWORD).expect("encrypt db");
}

pub fn app_for(db_path: &Path) -> (Router, String) {
    let token = auth::generate_token();
    let state = AppState::new(db_path.to_path_buf(), token.clone());
    (super::build_router(state), token)
}

pub fn session_get(uri: &str, token: &str) -> Request<Body> {
    get_request(uri)
        .header(header::COOKIE, format!("nigel_session={token}"))
        .body(Body::empty())
        .expect("request")
}

pub fn session_post(uri: &str, token: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::HOST, HOST)
        .header(header::COOKIE, format!("nigel_session={token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .expect("request")
}

/// A request with a session cookie, a method, and an optional JSON body.
pub fn session_request(method: &str, uri: &str, token: &str, body: Option<&str>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, HOST)
        .header(header::COOKIE, format!("nigel_session={token}"));
    match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_owned()))
            .expect("request"),
        None => builder.body(Body::empty()).expect("request"),
    }
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let uri = request.uri().to_string();
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    assert!(
        content_type(&response).starts_with("application/json"),
        "{uri} did not answer with JSON"
    );
    (status, json_body(response).await)
}

/// POST a JSON body with a valid session.
pub async fn post_json(
    app: &Router,
    uri: &str,
    token: &str,
    body: &serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        session_request("POST", uri, token, Some(&body.to_string())),
    )
    .await
}

/// PATCH a JSON body with a valid session.
pub async fn patch_json(
    app: &Router,
    uri: &str,
    token: &str,
    body: &serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        session_request("PATCH", uri, token, Some(&body.to_string())),
    )
    .await
}

/// DELETE with a valid session.
pub async fn delete_json(app: &Router, uri: &str, token: &str) -> (StatusCode, serde_json::Value) {
    send(app, session_request("DELETE", uri, token, None)).await
}

/// A `multipart/form-data` body with one file field, built by hand — the
/// alternative is a client crate the production build has no use for.
pub fn multipart_body(field: &str, filename: &str, content: &[u8]) -> (String, Vec<u8>) {
    const BOUNDARY: &str = "----nigeltestboundary";
    let mut body = format!(
        "--{BOUNDARY}\r\n\
         Content-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

    (format!("multipart/form-data; boundary={BOUNDARY}"), body)
}

/// POST a file to an upload route with a valid session.
pub async fn upload_file(
    app: &Router,
    uri: &str,
    token: &str,
    filename: &str,
    content: &[u8],
) -> (StatusCode, serde_json::Value) {
    let (content_type, body) = multipart_body("file", filename, content);
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::HOST, HOST)
        .header(header::COOKIE, format!("nigel_session={token}"))
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .expect("request");
    send(app, request).await
}

pub fn unlock_body(password: &str) -> String {
    serde_json::json!({ "password": password }).to_string()
}

pub async fn status_json(app: &Router, token: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(session_get("/api/status", token))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

/// Fetch a route with a valid session and decode the JSON body.
pub async fn get_json(app: &Router, uri: &str, token: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(session_get(uri, token))
        .await
        .expect("response");
    let status = response.status();
    assert!(
        content_type(&response).starts_with("application/json"),
        "{uri} did not answer with JSON"
    );
    (status, json_body(response).await)
}

/// Assert a route answers 200 and hand back the body.
pub async fn ok_json(app: &Router, uri: &str, token: &str) -> serde_json::Value {
    let (status, json) = get_json(app, uri, token).await;
    assert_eq!(status, StatusCode::OK, "GET {uri} returned {json}");
    json
}

/// Every route that reads the database, for tests that must hold across all of
/// them — the locked guard especially, where a route mounted in the wrong place
/// would silently answer while the database is still encrypted.
pub const DATA_ROUTES: [&str; 17] = [
    "/api/reports/pnl",
    "/api/reports/expenses",
    "/api/reports/tax",
    "/api/reports/cashflow",
    "/api/reports/balance",
    "/api/reports/flagged",
    "/api/reports/register",
    "/api/reports/k1",
    "/api/accounts",
    "/api/categories",
    "/api/rules",
    "/api/imports",
    "/api/imports/formats",
    "/api/csv-profiles",
    "/api/review/queue",
    "/api/review/1",
    "/api/reconciliations",
];

/// Every route that writes, as method, path, and a body good enough to reach
/// the handler. The locked guard has to refuse all of them: a mutation that
/// slipped past it would be changing a database nobody has unlocked.
pub const WRITE_ROUTES: [(&str, &str, &str); 16] = [
    ("PATCH", "/api/transactions/1", r#"{"flag":true}"#),
    ("POST", "/api/categorize", "{}"),
    ("POST", "/api/review/1/apply", r#"{"categoryId":1}"#),
    ("POST", "/api/review/1/undo", "{}"),
    (
        "POST",
        "/api/accounts",
        r#"{"name":"X","accountType":"checking"}"#,
    ),
    ("PATCH", "/api/accounts/1", r#"{"name":"X"}"#),
    ("DELETE", "/api/accounts/1", ""),
    (
        "POST",
        "/api/categories",
        r#"{"name":"X","categoryType":"expense"}"#,
    ),
    ("PATCH", "/api/categories/1", r#"{"name":"X"}"#),
    ("DELETE", "/api/categories/1", ""),
    ("POST", "/api/rules", r#"{"pattern":"X","categoryId":1}"#),
    ("POST", "/api/rules/test", r#"{"pattern":"X"}"#),
    ("DELETE", "/api/imports/1", ""),
    // The guard runs before the extractors, so these bodies only have to reach
    // the router — the upload route never gets as far as wanting multipart.
    ("POST", "/api/imports/upload", "{}"),
    (
        "POST",
        "/api/imports/preview",
        r#"{"uploadId":"x","account":"X"}"#,
    ),
    (
        "POST",
        "/api/imports/confirm",
        r#"{"uploadId":"x","account":"X"}"#,
    ),
];

/// Send one entry of [`WRITE_ROUTES`].
pub async fn send_write(
    app: &Router,
    (method, uri, body): (&str, &str, &str),
    token: &str,
) -> (StatusCode, serde_json::Value) {
    let body = (!body.is_empty()).then_some(body);
    send(app, session_request(method, uri, token, body)).await
}

/// An initialized database with a fixed, hand-built data set.
///
/// The dates are literals rather than offsets from today so that a test pinning
/// a year or a month keeps meaning the same thing next January. Demo generation
/// would have been less typing and less stable.
pub fn seeded_db() -> (tempfile::TempDir, PathBuf) {
    let (dir, db_path) = temp_db();
    let conn = crate::db::open_connection(&db_path, None).expect("open db");
    seed(&conn);
    drop(conn);
    (dir, db_path)
}

fn category_id(conn: &Connection, name: &str) -> i64 {
    conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |row| {
        row.get(0)
    })
    .unwrap_or_else(|e| panic!("category {name}: {e}"))
}

/// One seeded transaction. A struct rather than a tuple so the fixture table
/// below reads as data instead of a row of unlabelled positions.
struct Fixture {
    account: i64,
    date: &'static str,
    description: &'static str,
    amount: f64,
    category: Option<i64>,
    vendor: Option<&'static str>,
    flagged: bool,
}

impl Fixture {
    fn new(account: i64, date: &'static str, description: &'static str, amount: f64) -> Self {
        Self {
            account,
            date,
            description,
            amount,
            category: None,
            vendor: None,
            flagged: false,
        }
    }

    fn categorized(mut self, category: i64, vendor: &'static str) -> Self {
        self.category = Some(category);
        self.vendor = Some(vendor);
        self
    }

    fn flagged(mut self) -> Self {
        self.flagged = true;
        self
    }
}

fn seed(conn: &Connection) {
    conn.execute(
        "INSERT INTO accounts (name, account_type, institution, last_four) \
         VALUES ('BofA Checking', 'checking', 'Bank of America', '1234')",
        [],
    )
    .expect("checking account");
    let checking = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO accounts (name, account_type) VALUES ('BofA Credit Card', 'credit_card')",
        [],
    )
    .expect("card account");
    let card = conn.last_insert_rowid();

    let services = category_id(conn, "Client Services");
    let software = category_id(conn, "Software & Subscriptions");
    let fees = category_id(conn, "Bank & Merchant Fees");

    conn.execute(
        "INSERT INTO imports (filename, account_id, record_count, checksum) \
         VALUES ('jan-2025.csv', ?1, 4, 'sum-jan')",
        [checking],
    )
    .expect("import");
    let import = conn.last_insert_rowid();

    // A second import with nothing attached: import history must still list it.
    conn.execute(
        "INSERT INTO imports (filename, account_id, record_count, checksum) \
         VALUES ('empty.csv', ?1, 0, 'sum-empty')",
        [card],
    )
    .expect("empty import");

    let rows = [
        Fixture::new(checking, "2025-01-15", "ACME CORP INVOICE 001", 5_000.00)
            .categorized(services, "Acme Corp"),
        Fixture::new(checking, "2025-02-10", "ADOBE CREATIVE CLOUD", -59.99)
            .categorized(software, "Adobe"),
        Fixture::new(checking, "2025-02-28", "MONTHLY SERVICE FEE", -12.00)
            .categorized(fees, "Bank of America"),
        Fixture::new(checking, "2025-03-05", "GLOBEX RETAINER", 2_500.00)
            .categorized(services, "Globex"),
        Fixture::new(checking, "2025-03-19", "ADOBE CREATIVE CLOUD", -59.99)
            .categorized(software, "Adobe"),
        // Uncategorized and flagged: the review queue and the flagged report
        // both need something to find.
        Fixture::new(card, "2025-03-22", "UNKNOWN VENDOR 8812", -240.50).flagged(),
        Fixture::new(card, "2024-11-04", "ACME CORP INVOICE 000", 1_200.00)
            .categorized(services, "Acme Corp"),
        Fixture::new(card, "2024-12-30", "ADOBE CREATIVE CLOUD", -49.99)
            .categorized(software, "Adobe"),
    ];

    for row in rows {
        conn.execute(
            "INSERT INTO transactions \
             (account_id, date, description, amount, category_id, vendor, is_flagged, import_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                row.account,
                row.date,
                row.description,
                row.amount,
                row.category,
                row.vendor,
                row.flagged,
                import
            ],
        )
        .expect("transaction");
    }

    conn.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority, hit_count) \
         VALUES ('ADOBE', 'contains', 'Adobe', ?1, 10, 3)",
        [software],
    )
    .expect("rule");
    conn.execute(
        "INSERT INTO rules (pattern, match_type, vendor, category_id, priority, hit_count) \
         VALUES ('SERVICE FEE', 'contains', NULL, ?1, 0, 1)",
        [fees],
    )
    .expect("rule");

    let profile = crate::importer::GenericCsvConfig {
        date_col: 0,
        desc_col: 1,
        amount_col: 3,
        date_format: "%m/%d/%Y".to_string(),
    };
    crate::importer::save_csv_profile(conn, "chase", &profile).expect("profile");
}
