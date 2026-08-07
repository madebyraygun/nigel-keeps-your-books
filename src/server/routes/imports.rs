//! Imports: the history (`GET /api/imports`), undoing one by id, the saved
//! column mappings, and the three-step pipeline a browser uses to run one.
//!
//! The pipeline is upload → preview → confirm, because a browser has no path to
//! hand over and no chance to look at the file before committing it. Upload
//! spools the bytes; preview is `import_file` with `dry_run`, which reads
//! everything and writes nothing; confirm repeats the TUI's sequence exactly —
//! snapshot, import, categorize — against the same spooled file.

use axum::extract::multipart::{MultipartError, MultipartRejection};
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::categorizer::categorize_transactions;
use crate::cli::backup;
use crate::cli::undo::{self, ImportListItem};
use crate::error::NigelError;
use crate::importer::{self, CsvProfile, GenericCsvConfig, ImportResult, ImporterFormat};

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::super::uploads::{self, StoredUpload};
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/imports", get(list_imports))
        .route("/imports/{id}", delete(undo_import))
        .route("/imports/formats", get(list_formats))
        .route(
            "/imports/upload",
            post(upload).layer(DefaultBodyLimit::max(uploads::MAX_UPLOAD_BYTES)),
        )
        .route("/imports/preview", post(preview))
        .route("/imports/confirm", post(confirm))
        .route("/csv-profiles", get(list_csv_profiles))
}

/// What undoing an import removed. The count is the number that matters — the
/// import record itself going away is implied.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoneImport {
    id: i64,
    deleted_transactions: usize,
}

/// Roll back one import: its transactions and its record, exactly what
/// `nigel undo` does to the most recent one.
async fn undo_import(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<UndoneImport>> {
    let deleted_transactions = with_conn(&state, move |conn| {
        // `delete_import` reports a missing import as zero rows deleted, which
        // over HTTP would read as a successful undo of nothing.
        if !undo::import_exists(conn, id)? {
            return Err(NigelError::NotFound(format!("No import with ID {id}")));
        }
        undo::delete_import(conn, id)
    })
    .await?;

    Ok(Json(UndoneImport {
        id,
        deleted_transactions,
    }))
}

async fn list_imports(State(state): State<AppState>) -> ApiResult<Json<Vec<ImportListItem>>> {
    Ok(Json(with_conn(&state, undo::list_imports).await?))
}

async fn list_csv_profiles(State(state): State<AppState>) -> ApiResult<Json<Vec<CsvProfile>>> {
    Ok(Json(with_conn(&state, importer::list_csv_profiles).await?))
}

/// The importers this build has, for the client's format picker. Gusto is
/// missing from a build without its feature, which is why the list is served
/// rather than hardcoded in the SPA.
async fn list_formats() -> Json<Vec<ImporterFormat>> {
    Json(importer::built_in_formats())
}

// ---------------------------------------------------------------------------
// upload → preview → confirm
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadResponse {
    upload_id: String,
    filename: String,
    size: u64,
}

/// Take one file and park it on disk. The response's `uploadId` is what preview
/// and confirm are given; the bytes are never held between requests.
async fn upload(
    State(state): State<AppState>,
    multipart: Result<Multipart, MultipartRejection>,
) -> ApiResult<Json<UploadResponse>> {
    let mut multipart = multipart.map_err(|r| ApiError::bad_request(r.body_text()))?;

    let mut file = None;
    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        // The field that carries a filename is the file; a form is free to send
        // others alongside it.
        let Some(name) = field.file_name().map(str::to_owned) else {
            continue;
        };
        let bytes = field.bytes().await.map_err(multipart_error)?;
        file = Some((name, bytes));
        break;
    }

    let Some((raw_name, bytes)) = file else {
        return Err(ApiError::bad_request(
            "Expected a multipart form with a file field named `file`.",
        ));
    };
    let filename = uploads::sanitize_filename(&raw_name).map_err(ApiError::bad_request)?;

    let stored = blocking(&state, move |db_path| {
        let dir = uploads::uploads_dir(&db_path);
        // Every upload is also a chance to collect the ones nobody came back
        // for, so an abandoned statement never lingers past its hour.
        uploads::purge_stale(&dir, uploads::MAX_AGE);
        uploads::store(&dir, &filename, &bytes).map_err(ApiError::from)
    })
    .await?;

    Ok(Json(UploadResponse {
        upload_id: stored.id,
        filename: stored.filename,
        size: stored.size,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewRequest {
    upload_id: String,
    account: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    mapping: Option<GenericCsvConfig>,
}

/// Parse the upload and report what an import would do, touching nothing.
///
/// `dry_run` skips the snapshot, the `imports` row and every transaction
/// insert; the checksum and duplicate lookups it still does are reads.
async fn preview(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<PreviewRequest>,
) -> ApiResult<Json<ImportResult>> {
    let plan = ImportPlan::build(
        &state,
        &request.upload_id,
        request.account,
        request.format,
        request.mapping,
    )?;

    let result = blocking(&state, move |db_path| plan.run(&db_path, true)).await?;

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfirmRequest {
    upload_id: String,
    account: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    mapping: Option<GenericCsvConfig>,
    #[serde(default)]
    save_profile: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmResponse {
    #[serde(flatten)]
    result: ImportResult,
    categorized: usize,
    still_flagged: usize,
    /// Where the pre-import snapshot went, the same line the CLI prints.
    snapshot: String,
}

/// Run the import for real: snapshot, import, categorize — the sequence the
/// dashboard's import screen has always used, in that order.
async fn confirm(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<ConfirmRequest>,
) -> ApiResult<Json<ConfirmResponse>> {
    let plan = ImportPlan::build(
        &state,
        &request.upload_id,
        request.account,
        request.format,
        request.mapping,
    )?;
    let profile = SaveProfile::build(request.save_profile, plan.mapping.as_ref())?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let snapshot = state
        .data_dir()
        .join(format!("snapshots/pre-import-{stamp}.db"));

    let upload_id = plan.upload.id.clone();
    let snapshot_path = snapshot.clone();

    let response = blocking(&state, move |db_path| {
        let conn = crate::db::get_connection(&db_path)?;
        // Checked before the snapshot: `import_file` would catch it a moment
        // later, but only after writing a snapshot for an import that was never
        // going to happen.
        super::ensure_account_exists(&conn, &plan.account)?;
        backup::snapshot(&conn, &snapshot_path)?;

        let result = plan.import(&conn, false)?;
        // A file already imported is answered, not undone: the CLI stops here
        // too, before categorizing.
        let (categorized, still_flagged) = if result.duplicate_file {
            (0, 0)
        } else {
            let counts = categorize_transactions(&conn).map_err(ApiError::from)?;
            (counts.categorized, counts.still_flagged)
        };

        // After the import rather than before it, so a file that would not
        // parse does not leave a profile behind.
        if let Some(profile) = profile {
            profile.save(&conn)?;
        }

        Ok(ConfirmResponse {
            result,
            categorized,
            still_flagged,
            snapshot: snapshot_path.display().to_string(),
        })
    })
    .await?;

    // The file has done its job. A failed confirm keeps it, so the same
    // uploadId can be retried once the caller fixes the request.
    blocking(&state, move |db_path| {
        uploads::delete(&uploads::uploads_dir(&db_path), &upload_id);
        Ok(())
    })
    .await?;

    Ok(Json(response))
}

/// A validated import request: which file, which account, and which importer.
struct ImportPlan {
    upload: StoredUpload,
    account: String,
    format: Option<String>,
    mapping: Option<GenericCsvConfig>,
}

impl ImportPlan {
    /// Everything preview and confirm can reject before touching the database.
    fn build(
        state: &AppState,
        upload_id: &str,
        account: String,
        format: Option<String>,
        mapping: Option<GenericCsvConfig>,
    ) -> ApiResult<Self> {
        if format.is_some() && mapping.is_some() {
            return Err(ApiError::bad_request(
                "Send either `format` or `mapping`, not both.",
            ));
        }
        if let Some(key) = format.as_deref() {
            ensure_format_is_available(state, key)?;
        }

        let dir = uploads::uploads_dir(&state.db_path());
        let upload = uploads::resolve(&dir, upload_id).ok_or_else(upload_not_found)?;

        Ok(Self {
            upload,
            account,
            format,
            mapping,
        })
    }

    fn import(&self, conn: &rusqlite::Connection, dry_run: bool) -> ApiResult<ImportResult> {
        importer::import_file(
            conn,
            &self.upload.path,
            &self.account,
            self.format.as_deref(),
            dry_run,
            self.mapping.as_ref(),
        )
        .map_err(import_error)
    }

    fn run(&self, db_path: &std::path::Path, dry_run: bool) -> ApiResult<ImportResult> {
        let conn = crate::db::get_connection(db_path)?;
        self.import(&conn, dry_run)
    }
}

/// A column mapping the caller wants remembered under a name.
struct SaveProfile {
    name: String,
    config: GenericCsvConfig,
}

impl SaveProfile {
    fn build(name: Option<String>, mapping: Option<&GenericCsvConfig>) -> ApiResult<Option<Self>> {
        let Some(name) = name else {
            return Ok(None);
        };
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(ApiError::bad_request("A profile name cannot be empty."));
        }
        // save_csv_profile refuses this too, but only once the import has
        // already run, and as an error that would read as a server fault.
        if importer::get_by_key(&name).is_some() {
            return Err(ApiError::bad_request(format!(
                "'{name}' is the name of a built-in importer; choose another."
            )));
        }
        let Some(config) = mapping else {
            return Err(ApiError::bad_request(
                "Saving a profile needs the `mapping` it should remember.",
            ));
        };

        Ok(Some(Self {
            name,
            config: config.clone(),
        }))
    }

    fn save(&self, conn: &rusqlite::Connection) -> ApiResult<()> {
        importer::save_csv_profile(conn, &self.name, &self.config).map_err(ApiError::from)
    }
}

/// Run blocking database and filesystem work off the async runtime.
///
/// Takes the `db_gate` read guard for the duration: these routes open their own
/// connections rather than going through `with_conn`, and encrypt, decrypt and
/// the data-directory switch rewrite the database file itself. Both the spool
/// directory and the database live in the data directory, so filesystem-only
/// work belongs under the same guard.
///
/// The database path is handed to the closure rather than captured by it, so it
/// is always read after the guard opens — one taken beforehand would name the
/// database a pending switch has already replaced.
async fn blocking<T, F>(state: &AppState, work: F) -> ApiResult<T>
where
    F: FnOnce(PathBuf) -> ApiResult<T> + Send + 'static,
    T: Send + 'static,
{
    let _gate = state.db_gate.read().await;
    let db_path = state.db_path();
    tokio::task::spawn_blocking(move || work(db_path))
        .await
        .map_err(ApiError::internal)?
}

/// A format key the caller named that this build cannot honour.
///
/// Without the `gusto` feature the key is not just unknown — resolution would
/// fall through to the saved profiles and report a typo — it is absent because
/// of how the binary was built, which is what 501 says.
fn ensure_format_is_available(state: &AppState, key: &str) -> ApiResult<()> {
    if key == "gusto_payroll" && !state.features.gusto {
        return Err(ApiError::feature_disabled(
            "This build has no Gusto payroll support. Rebuild with the `gusto` feature to import Gusto files.",
        ));
    }
    Ok(())
}

/// Expired, already imported, or never real — from the caller's side these are
/// the same thing, and the reason code tells the client to start over with a
/// fresh upload rather than retry this one.
fn upload_not_found() -> ApiError {
    ApiError::not_found("That upload is no longer available — upload the file again.")
        .with_details(serde_json::json!({ "reason": "upload_not_found" }))
}

/// Import failures the caller can do something about.
///
/// `import_file` runs the parsers, and a file that is not what the caller said
/// it was surfaces as `Csv` or `Other` — which the blanket mapping would call a
/// server fault. Scoped to these routes; the global mapping is unchanged.
fn import_error(err: NigelError) -> ApiError {
    match err {
        NigelError::Csv(_) | NigelError::Other(_) => ApiError::bad_request(err.to_string()),
        other => ApiError::from(other),
    }
}

fn multipart_error(err: MultipartError) -> ApiError {
    if err.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::payload_too_large(format!(
            "That file is larger than the {} MB upload limit.",
            uploads::MAX_UPLOAD_BYTES / (1024 * 1024)
        ))
    } else {
        ApiError::bad_request(err.body_text())
    }
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use crate::server::uploads;
    use axum::http::StatusCode;
    use axum::Router;
    use serde_json::{json, Value};
    use std::path::{Path, PathBuf};

    /// A Bank of America checking export, which is what the seeded account's
    /// type resolves to when no format is named.
    fn bofa_csv(rows: &[(&str, &str, &str)]) -> Vec<u8> {
        let mut content = String::from("Date,Description,Amount,Running Bal.\n");
        for (date, description, amount) in rows {
            content.push_str(&format!("{date},{description},{amount},0.00\n"));
        }
        content.into_bytes()
    }

    fn statement() -> Vec<u8> {
        bofa_csv(&[
            ("04/01/2025", "ACME CORP INVOICE 002", "3000.00"),
            ("04/03/2025", "ADOBE CREATIVE CLOUD", "-59.99"),
            ("04/09/2025", "WEBFLOW SUBSCRIPTION", "-42.00"),
        ])
    }

    /// The same three rows with the columns in another order and dashes for
    /// dates: nothing built in can read it, an inline mapping can.
    fn foreign_csv() -> Vec<u8> {
        let mut content = String::from("Posted,Memo,Ref,Value\n");
        content.push_str("2025-05-01,NORTHWIND DEPOSIT,001,4000.00\n");
        content.push_str("2025-05-04,LINEAR SUBSCRIPTION,002,-96.00\n");
        content.into_bytes()
    }

    fn mapping() -> Value {
        json!({"dateCol": 0, "descCol": 1, "amountCol": 3, "dateFormat": "%Y-%m-%d"})
    }

    fn spool_dir(db_path: &Path) -> PathBuf {
        uploads::uploads_dir(db_path)
    }

    fn spooled_count(db_path: &Path) -> usize {
        std::fs::read_dir(spool_dir(db_path))
            .map(|entries| entries.flatten().count())
            .unwrap_or(0)
    }

    /// Upload a file and hand back its id.
    async fn upload_ok(app: &Router, token: &str, name: &str, content: &[u8]) -> String {
        let (status, body) = upload_file(app, "/api/imports/upload", token, name, content).await;
        assert_eq!(status, StatusCode::OK, "upload {name}: {body}");
        body["uploadId"].as_str().expect("an uploadId").to_string()
    }

    fn counts(db_path: &Path) -> (i64, i64) {
        let conn = crate::db::open_connection(db_path, None).expect("open db");
        let count = |table: &str| {
            conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
        };
        (count("transactions"), count("imports"))
    }

    #[tokio::test]
    async fn import_history_is_newest_first_with_counts() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/imports", &token).await;
        let expected = serde_json::to_value(super::undo::list_imports(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["filename"], "empty.csv");
        assert_eq!(rows[0]["transactionCount"], 0);
        assert_eq!(rows[1]["filename"], "jan-2025.csv");
        assert_eq!(rows[1]["transactionCount"], 8);
        assert_eq!(rows[1]["accountName"], "BofA Checking");
    }

    #[tokio::test]
    async fn undoing_an_import_removes_its_transactions() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let imports = ok_json(&app, "/api/imports", &token).await;
        let loaded = imports
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["filename"] == "jan-2025.csv")
            .expect("the seeded import");
        let id = loaded["id"].as_i64().unwrap();
        assert_eq!(loaded["transactionCount"], 8);

        let (status, body) = delete_json(&app, &format!("/api/imports/{id}"), &token).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["deletedTransactions"], 8);
        assert_eq!(body["id"], id);

        let register = ok_json(&app, "/api/reports/register", &token).await;
        assert!(register["report"]["rows"].as_array().unwrap().is_empty());
        let remaining = ok_json(&app, "/api/imports", &token).await;
        assert_eq!(remaining.as_array().unwrap().len(), 1);

        // Undoing it twice is a 404, not a cheerful zero.
        let (status, body) = delete_json(&app, &format!("/api/imports/{id}"), &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn csv_profiles_carry_their_column_mapping() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/csv-profiles", &token).await;
        let expected =
            serde_json::to_value(super::importer::list_csv_profiles(&conn).unwrap()).unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "chase");
        assert_eq!(rows[0]["config"]["amountCol"], 3);
        assert_eq!(rows[0]["config"]["dateFormat"], "%m/%d/%Y");
    }

    #[tokio::test]
    async fn built_in_formats_are_listed_with_their_account_types() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/imports/formats", &token).await;
        let rows = body.as_array().expect("a bare array");

        let checking = rows
            .iter()
            .find(|f| f["key"] == "bofa_checking")
            .expect("the checking importer");
        assert_eq!(checking["name"], "Bank of America Checking");
        assert_eq!(checking["accountTypes"], json!(["checking"]));

        // Gusto is present exactly when this build can read a Gusto file.
        let has_gusto = rows.iter().any(|f| f["key"] == "gusto_payroll");
        assert_eq!(has_gusto, cfg!(feature = "gusto"));
    }

    #[tokio::test]
    async fn upload_preview_confirm_walks_a_statement_into_the_ledger() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, uploaded) = upload_file(
            &app,
            "/api/imports/upload",
            &token,
            "april.csv",
            &statement(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{uploaded}");
        assert_eq!(uploaded["filename"], "april.csv");
        assert_eq!(uploaded["size"], statement().len());
        let upload_id = uploaded["uploadId"].as_str().expect("an uploadId");

        let request = json!({"uploadId": upload_id, "account": "BofA Checking"});
        let (status, preview) = post_json(&app, "/api/imports/preview", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{preview}");
        assert_eq!(preview["format"], "bofa_checking");
        assert_eq!(preview["imported"], 3);
        assert_eq!(preview["skipped"], 0);
        assert_eq!(preview["malformed"], 0);
        assert_eq!(preview["duplicateFile"], false);
        assert_eq!(preview["importId"], Value::Null);
        assert_eq!(preview["sample"][0]["date"], "2025-04-01");
        assert_eq!(preview["sample"][0]["description"], "ACME CORP INVOICE 002");
        assert_eq!(preview["sample"][0]["amount"], 3000.0);

        let (status, confirmed) = post_json(&app, "/api/imports/confirm", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{confirmed}");
        assert_eq!(confirmed["imported"], 3);
        assert_eq!(confirmed["format"], "bofa_checking");
        assert!(confirmed["importId"].is_i64(), "{confirmed}");

        // The seeded ADOBE rule matches one of the three. The two left over
        // join the one the fixture already had flagged — the count is the
        // ledger's, not this import's.
        assert_eq!(confirmed["categorized"], 1);
        assert_eq!(confirmed["stillFlagged"], 3);

        let register = ok_json(&app, "/api/reports/register?year=2025", &token).await;
        let descriptions: Vec<&str> = register["report"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["description"].as_str())
            .collect();
        assert!(
            descriptions.contains(&"WEBFLOW SUBSCRIPTION"),
            "{descriptions:?}"
        );
    }

    #[tokio::test]
    async fn preview_changes_nothing() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let before = counts(&db_path);

        let upload_id = upload_ok(&app, &token, "april.csv", &statement()).await;
        let request = json!({"uploadId": upload_id, "account": "BofA Checking"});

        let (status, preview) = post_json(&app, "/api/imports/preview", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{preview}");
        assert_eq!(preview["imported"], 3);

        assert_eq!(counts(&db_path), before, "preview wrote to the database");
        assert!(
            !db_path.parent().unwrap().join("snapshots").exists(),
            "preview took a snapshot"
        );
        // The upload survives a preview: confirm is given the same id.
        assert_eq!(spooled_count(&db_path), 1);
    }

    #[tokio::test]
    async fn confirm_snapshots_and_keeps_the_users_filename() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let upload_id = upload_ok(&app, &token, "april.csv", &statement()).await;
        let request = json!({"uploadId": upload_id, "account": "BofA Checking"});
        let (status, confirmed) = post_json(&app, "/api/imports/confirm", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{confirmed}");

        let snapshot = PathBuf::from(confirmed["snapshot"].as_str().expect("a snapshot path"));
        assert!(snapshot.exists(), "no snapshot at {}", snapshot.display());
        assert!(snapshot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("pre-import-"));

        // The imports row carries the name the user sent, not the upload id.
        let history = ok_json(&app, "/api/imports", &token).await;
        let newest = &history[0];
        assert_eq!(newest["filename"], "april.csv");
        assert_eq!(newest["transactionCount"], 3);
        assert_eq!(newest["id"], confirmed["importId"]);

        assert_eq!(spooled_count(&db_path), 0, "the upload was not cleaned up");
    }

    #[tokio::test]
    async fn confirming_the_same_file_twice_reports_a_duplicate() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let first = upload_ok(&app, &token, "april.csv", &statement()).await;
        let (status, _) = post_json(
            &app,
            "/api/imports/confirm",
            &token,
            &json!({"uploadId": first, "account": "BofA Checking"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let after_first = counts(&db_path);

        let second = upload_ok(&app, &token, "april-again.csv", &statement()).await;
        let request = json!({"uploadId": second, "account": "BofA Checking"});

        // The checksum is what catches it, so a preview says so too.
        let (status, preview) = post_json(&app, "/api/imports/preview", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{preview}");
        assert_eq!(preview["duplicateFile"], true);

        let (status, confirmed) = post_json(&app, "/api/imports/confirm", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{confirmed}");
        assert_eq!(confirmed["duplicateFile"], true);
        assert_eq!(confirmed["imported"], 0);
        assert_eq!(confirmed["importId"], Value::Null);
        assert_eq!(confirmed["format"], Value::Null);
        assert_eq!(confirmed["categorized"], 0);

        assert_eq!(
            counts(&db_path),
            after_first,
            "a duplicate changed the data"
        );
    }

    #[tokio::test]
    async fn an_inline_mapping_imports_and_can_be_saved_as_a_profile() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let upload_id = upload_ok(&app, &token, "northwind.csv", &foreign_csv()).await;
        let request = json!({
            "uploadId": upload_id,
            "account": "BofA Checking",
            "mapping": mapping(),
        });

        let (status, preview) = post_json(&app, "/api/imports/preview", &token, &request).await;
        assert_eq!(status, StatusCode::OK, "{preview}");
        assert_eq!(preview["format"], "generic");
        assert_eq!(preview["imported"], 2);

        let mut confirm_body = request.clone();
        confirm_body["saveProfile"] = json!("northwind");
        let (status, confirmed) =
            post_json(&app, "/api/imports/confirm", &token, &confirm_body).await;
        assert_eq!(status, StatusCode::OK, "{confirmed}");
        assert_eq!(confirmed["imported"], 2);

        let profiles = ok_json(&app, "/api/csv-profiles", &token).await;
        let saved = profiles
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "northwind")
            .expect("the saved profile");
        assert_eq!(saved["config"], mapping());

        // And the saved name now resolves as a format on its own.
        let next = upload_ok(
            &app,
            &token,
            "northwind-june.csv",
            b"Posted,Memo,Ref,Value\n2025-06-02,NORTHWIND DEPOSIT,003,1500.00\n",
        )
        .await;
        let (status, preview) = post_json(
            &app,
            "/api/imports/preview",
            &token,
            &json!({"uploadId": next, "account": "BofA Checking", "format": "northwind"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{preview}");
        assert_eq!(preview["format"], "northwind");
        assert_eq!(preview["imported"], 1);
    }

    #[tokio::test]
    async fn an_oversize_upload_is_refused() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let too_big = vec![b'a'; uploads::MAX_UPLOAD_BYTES + 1024];
        let (status, body) =
            upload_file(&app, "/api/imports/upload", &token, "huge.csv", &too_big).await;

        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
        assert_eq!(body["error"]["code"], "payload_too_large");
        assert_eq!(spooled_count(&db_path), 0, "an oversize file reached disk");
    }

    #[tokio::test]
    async fn a_file_type_no_importer_reads_is_refused() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) =
            upload_file(&app, "/api/imports/upload", &token, "notes.txt", b"hello").await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
        assert_eq!(spooled_count(&db_path), 0);
    }

    #[tokio::test]
    async fn a_request_without_a_file_is_refused() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(&app, "/api/imports/upload", &token, &json!({})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
    }

    #[tokio::test]
    async fn an_unknown_upload_id_says_to_start_over() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // Well formed but never issued, and a traversal attempt.
        for id in [uploads::new_id().as_str(), "../../etc", ""] {
            for route in ["/api/imports/preview", "/api/imports/confirm"] {
                let (status, body) = post_json(
                    &app,
                    route,
                    &token,
                    &json!({"uploadId": id, "account": "BofA Checking"}),
                )
                .await;
                assert_eq!(status, StatusCode::NOT_FOUND, "{route} with {id:?}: {body}");
                assert_eq!(body["error"]["code"], "not_found");
                assert_eq!(body["error"]["details"]["reason"], "upload_not_found");
            }
        }
    }

    #[tokio::test]
    async fn a_failed_confirm_leaves_the_upload_to_retry() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let before = counts(&db_path);

        let upload_id = upload_ok(&app, &token, "april.csv", &statement()).await;
        let (status, body) = post_json(
            &app,
            "/api/imports/confirm",
            &token,
            &json!({"uploadId": upload_id, "account": "No Such Account"}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(counts(&db_path), before);
        assert!(
            !db_path.parent().unwrap().join("snapshots").exists(),
            "a rejected confirm still snapshotted"
        );

        // Same id, right account: the retry works.
        let (status, confirmed) = post_json(
            &app,
            "/api/imports/confirm",
            &token,
            &json!({"uploadId": upload_id, "account": "BofA Checking"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{confirmed}");
        assert_eq!(confirmed["imported"], 3);
    }

    #[tokio::test]
    async fn contradictory_and_unusable_requests_are_refused() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let upload_id = upload_ok(&app, &token, "april.csv", &statement()).await;
        let before = counts(&db_path);

        let cases: [(&str, Value); 5] = [
            (
                "both a format and a mapping",
                json!({"uploadId": upload_id, "account": "BofA Checking",
                       "format": "bofa_checking", "mapping": mapping()}),
            ),
            (
                "a format nothing answers to",
                json!({"uploadId": upload_id, "account": "BofA Checking", "format": "nope"}),
            ),
            (
                "a profile with nothing to save",
                json!({"uploadId": upload_id, "account": "BofA Checking",
                       "saveProfile": "chase"}),
            ),
            (
                "a profile named after a built-in",
                json!({"uploadId": upload_id, "account": "BofA Checking",
                       "mapping": mapping(), "saveProfile": "bofa_checking"}),
            ),
            (
                "a profile with a blank name",
                json!({"uploadId": upload_id, "account": "BofA Checking",
                       "mapping": mapping(), "saveProfile": "   "}),
            ),
        ];

        for (what, body) in cases {
            let (status, response) = post_json(&app, "/api/imports/confirm", &token, &body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {response}");
            assert_eq!(response["error"]["code"], "bad_request", "{what}");
        }

        assert_eq!(counts(&db_path), before, "a refused confirm still imported");
    }

    #[tokio::test]
    async fn an_unknown_account_is_a_not_found_on_preview() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let upload_id = upload_ok(&app, &token, "april.csv", &statement()).await;

        let (status, body) = post_json(
            &app,
            "/api/imports/preview",
            &token,
            &json!({"uploadId": upload_id, "account": "No Such Account"}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["code"], "not_found");
    }

    #[cfg(not(feature = "gusto"))]
    #[tokio::test]
    async fn gusto_without_the_feature_is_not_implemented() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let upload_id = upload_ok(&app, &token, "april.csv", &statement()).await;

        let (status, body) = post_json(
            &app,
            "/api/imports/preview",
            &token,
            &json!({"uploadId": upload_id, "account": "BofA Checking",
                    "format": "gusto_payroll"}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{body}");
        assert_eq!(body["error"]["code"], "feature_disabled");
    }

    #[tokio::test]
    async fn stale_uploads_are_collected_on_the_next_one() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let stale = upload_ok(&app, &token, "old.csv", b"Date,Description,Amount,Bal\n").await;
        let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 60 * 60);
        std::fs::File::open(spool_dir(&db_path).join(&stale))
            .unwrap()
            .set_modified(long_ago)
            .unwrap();

        let fresh = upload_ok(&app, &token, "april.csv", &statement()).await;

        assert_eq!(spooled_count(&db_path), 1);
        assert!(uploads::resolve(&spool_dir(&db_path), &fresh).is_some());
        let (status, body) = post_json(
            &app,
            "/api/imports/preview",
            &token,
            &json!({"uploadId": stale, "account": "BofA Checking"}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    }
}
