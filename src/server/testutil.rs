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

pub async fn body_bytes(response: Response) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .expect("body")
        .to_vec()
}

pub async fn body_string(response: Response) -> String {
    String::from_utf8(body_bytes(response).await).expect("utf-8 body")
}

/// One response header as a string, empty when it is absent.
pub fn header_str(response: &Response, name: header::HeaderName) -> String {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

/// Fetch a route with a valid session and keep the whole response: an export
/// answers with bytes and headers, which `get_json` would throw away.
pub async fn get_response(app: &Router, uri: &str, token: &str) -> Response {
    app.clone()
        .oneshot(session_get(uri, token))
        .await
        .expect("response")
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

/// Redirect `~/.config/nigel` at a temporary directory for the life of the
/// guard.
///
/// Any test that reaches `settings::save_settings` — the whole settings-screen
/// surface — would otherwise rewrite the developer's real settings.json and
/// repoint their data directory at a tempdir that is about to be deleted.
pub type TempConfig = crate::settings::TempConfigDir;

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

pub async fn send(app: &Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
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

/// PUT a JSON body with a valid session.
pub async fn put_json(
    app: &Router,
    uri: &str,
    token: &str,
    body: &serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    send(
        app,
        session_request("PUT", uri, token, Some(&body.to_string())),
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
pub const DATA_ROUTES: [&str; 24] = [
    "/api/settings/app",
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
    "/api/clients",
    "/api/clients/1",
    "/api/invoices",
    "/api/invoices/1248",
    "/api/invoices/aging",
    "/api/invoices/next-number",
];

/// The two invoice preview routes. Kept out of [`DATA_ROUTES`] for the reason
/// [`EXPORT_ROUTES`] is: a successful preview is a document, not JSON — only
/// the failures share a shape with the rest of the API.
pub const PREVIEW_ROUTES: [&str; 2] = [
    "/api/invoices/1248/preview",
    "/api/invoices/1248/preview.pdf",
];

/// Every export route, named without the `format` each of them requires. They
/// are kept out of [`DATA_ROUTES`] because a successful export is bytes, not
/// JSON — only the failure cases share a shape with the rest of the API.
pub const EXPORT_ROUTES: [&str; 8] = [
    "/api/exports/pnl",
    "/api/exports/expenses",
    "/api/exports/tax",
    "/api/exports/cashflow",
    "/api/exports/balance",
    "/api/exports/flagged",
    "/api/exports/register",
    "/api/exports/k1",
];

/// Every non-`GET` route, as method, path, and a body good enough to reach the
/// handler. The locked guard has to refuse all of them: a mutation that slipped
/// past it would be changing a database nobody has unlocked.
///
/// Described by method rather than by effect because two of them write nothing
/// — `rules/test` and `imports/preview` are dry runs — and a rule stated as
/// "the ones that write" invites the next dry run to be left out of a list the
/// guard still has to cover.
pub const WRITE_ROUTES: [(&str, &str, &str); 34] = [
    ("POST", "/api/clients", r#"{"name":"X"}"#),
    ("PATCH", "/api/clients/1", r#"{"name":"X"}"#),
    ("DELETE", "/api/clients/1", ""),
    (
        "POST",
        "/api/invoices",
        r#"{"clientId":1,"issueDate":"2026-04-01","items":[{"description":"X","quantity":1,"unitAmount":1}]}"#,
    ),
    ("PATCH", "/api/invoices/1252", r#"{"notes":"X"}"#),
    ("POST", "/api/invoices/1252/void", "{}"),
    (
        "POST",
        "/api/invoices/1252/pay",
        r#"{"amount":1,"date":"2026-04-01"}"#,
    ),
    // Confirmed, so the guard is what refuses it rather than the missing flag.
    // Neither reaches a gateway: send answers the confirmation and the
    // configuration before it opens a connection, and this suite configures
    // neither.
    ("POST", "/api/invoices/1252/send", r#"{"confirm":true}"#),
    ("POST", "/api/invoices/sync", "{}"),
    ("PATCH", "/api/rules/1", r#"{"priority":5}"#),
    ("DELETE", "/api/rules/1", ""),
    (
        "POST",
        "/api/reconcile",
        r#"{"account":"X","month":"2025-01","statementBalance":0}"#,
    ),
    ("PATCH", "/api/transactions/1", r#"{"flag":true}"#),
    ("PUT", "/api/settings/app", r#"{"updateCheck":true}"#),
    ("PUT", "/api/settings/company-name", r#"{"name":"X"}"#),
    ("POST", "/api/settings/data-dir", r#"{"path":"/tmp"}"#),
    (
        "POST",
        "/api/settings/password/set",
        r#"{"newPassword":"x"}"#,
    ),
    (
        "POST",
        "/api/settings/password/change",
        r#"{"currentPassword":"x","newPassword":"y"}"#,
    ),
    (
        "POST",
        "/api/settings/password/remove",
        r#"{"currentPassword":"x"}"#,
    ),
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

    seed_invoicing(conn);
}

/// The day the invoicing fixtures are read as of.
///
/// Every derived status and every aging bucket below is settled against this
/// date rather than the wall clock, for the same reason the transaction dates
/// are literals: a bucket boundary crossed overnight would otherwise change what
/// a committed fixture means.
pub const AS_OF: &str = "2026-03-15";

fn item(
    description: &str,
    quantity: f64,
    unit_amount: f64,
) -> crate::invoicing::invoices::NewLineItem {
    crate::invoicing::invoices::NewLineItem {
        description: description.to_string(),
        quantity,
        unit_amount,
    }
}

/// Three clients and one invoice per status, with literal dates.
///
/// It lives in the shared seed rather than beside the invoicing tests because
/// [`DATA_ROUTES`] names `/api/clients/1` and `/api/invoices/1248` by hand: a
/// detail route with nothing behind it would answer 404 in the very test that
/// proves the locked guard lets it through.
fn seed_invoicing(conn: &Connection) {
    use crate::invoicing::clients::add_client;
    use crate::invoicing::invoices as inv;

    // Numbering starts at 1248, and the void invoice is the oldest of the six.
    crate::db::set_metadata(conn, "next_invoice_number", "1247").expect("next invoice number");

    let acme = add_client(
        conn,
        "Acme Co",
        Some("ap@acme.test"),
        Some("1 Main St, Portland OR"),
        None,
    )
    .expect("acme");
    // No email: the client a send refuses, and the em dash every list prints.
    let globex = add_client(conn, "Globex", None, None, None).expect("globex");
    let northwind = add_client(
        conn,
        "Northwind Traders",
        Some("billing@nw.test"),
        None,
        None,
    )
    .expect("northwind");

    // 1247 — void.
    let id = inv::create_invoice(
        conn,
        globex,
        "2026-01-05",
        Some("2026-02-04"),
        "USD",
        &[item("Discovery workshop", 1.0, 500.00)],
        None,
        None,
    )
    .expect("1247");
    inv::void_invoice(conn, id, "2026-01-10").expect("void 1247");

    // 1248 — paid in full.
    let id = inv::create_invoice(
        conn,
        northwind,
        "2026-01-15",
        Some("2026-02-14"),
        "USD",
        &[item("Retainer - January", 1.0, 4_000.00)],
        Some("Thanks for your business."),
        Some("Net 30"),
    )
    .expect("1248");
    inv::mark_published(conn, id, "2026-01-15").expect("publish 1248");
    inv::record_payment(conn, id, 4_000.00, "2026-02-10", "direct_deposit", None)
        .expect("pay 1248");

    // 1249 — overdue.
    let id = inv::create_invoice(
        conn,
        globex,
        "2025-12-31",
        Some("2026-01-30"),
        "USD",
        &[item("Site build - phase 2", 8.0, 120.00)],
        None,
        None,
    )
    .expect("1249");
    inv::mark_published(conn, id, "2025-12-31").expect("publish 1249");

    // 1250 — partly paid, through Stripe.
    let id = inv::create_invoice(
        conn,
        acme,
        "2026-02-20",
        Some("2026-03-20"),
        "USD",
        &[
            item("Consulting - February", 16.0, 175.00),
            item("Hosting", 1.0, 400.00),
        ],
        None,
        Some("Net 30"),
    )
    .expect("1250");
    inv::mark_published(conn, id, "2026-02-20").expect("publish 1250");
    inv::record_payment(
        conn,
        id,
        2_000.00,
        "2026-03-01",
        "stripe",
        Some("cs_test_seed_1250"),
    )
    .expect("pay 1250");

    // 1251 — sent, carrying a live payment link.
    let id = inv::create_invoice(
        conn,
        acme,
        "2026-03-06",
        Some("2026-04-06"),
        "USD",
        &[
            item("Consulting - March", 10.0, 150.00),
            item("Hosting", 1.0, 350.00),
        ],
        None,
        None,
    )
    .expect("1251");
    inv::set_payment_link(
        conn,
        id,
        "plink_seed_1251",
        "https://buy.stripe.com/test_seed_1251",
    )
    .expect("link 1251");
    inv::mark_published(conn, id, "2026-03-06").expect("publish 1251");

    // 1252 — draft, with no due date, so it can never go overdue.
    inv::create_invoice(
        conn,
        northwind,
        "2026-03-12",
        None,
        "USD",
        &[item("Brand refresh - deposit", 1.0, 2_400.00)],
        None,
        None,
    )
    .expect("1252");

    // Each write above derived a status from its own date. Re-derive them all as
    // of one day, so `overdue` and `partial` mean what the fixtures claim.
    let mut stmt = conn
        .prepare("SELECT id FROM invoices ORDER BY number")
        .expect("invoice ids");
    let ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))
        .expect("invoice ids")
        .collect::<std::result::Result<Vec<i64>, _>>()
        .expect("invoice ids");
    drop(stmt);
    for id in ids {
        inv::refresh_status(conn, id, AS_OF).expect("refresh status");
    }
}
