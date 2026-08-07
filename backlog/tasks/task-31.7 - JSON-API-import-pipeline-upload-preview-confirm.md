---
id: TASK-31.7
title: 'JSON API: import pipeline (upload, preview, confirm)'
status: Done
assignee:
  - '@agent-31.7'
created_date: '2026-08-06 16:26'
updated_date: '2026-08-07 11:41'
labels:
  - web
  - backend
dependencies:
  - TASK-31.3
references:
  - src/importer.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.7-import-api.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser import flow on top of import_file(): multipart upload to a temp file; a preview step runs import_file with dry_run=true and returns detected format, sample rows, and imported/skipped/malformed counts without mutating anything; a confirm step mirrors the import_manager.rs sequence — pre-import snapshot, import, auto-categorize — and returns full results. Supports explicit format keys including saved csv_profiles, inline generic CSV column mappings, and saving a new profile.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Upload plus preview returns detected format, sample rows, and counts with no database mutation
- [x] #2 Confirm performs snapshot, import, and auto-categorization, returning imported/skipped/malformed/flagged counts
- [x] #3 Duplicate files (checksum) and duplicate rows are reported the same way the CLI reports them
- [x] #4 Generic CSV column mapping and save-profile are supported
- [x] #5 Upload size limits are enforced and temp files are cleaned up
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
IMPLEMENTS AFTER 31.5 AND 31.6 LAND (my routes join their routes/imports.rs).

## 0. Landing order and shared seams

- 31.5 creates src/server/routes/imports.rs (GET /api/imports, GET
  /api/csv-profiles) and src/server/testutil.rs (seeded_db/app/get_json). I add
  three POST handlers to that same module and reuse testutil; if testutil did
  not land, fall back to server/mod.rs's temp_db()/app_for() test helpers.
- 31.6 owns DELETE /api/imports/{id}. Static /imports/upload|preview|confirm and
  a param route at the same depth coexist in matchit 0.8 (static wins), but a
  conflict panics at router build, so the existing router-construction tests
  catch it immediately. Note for 31.6: axum 0.8 path syntax is `{id}`, not `:id`.
- No overlap with 31.8 (exports) or the SPA tasks.

## 1. Files

New:
- src/server/uploads.rs — the spool: dir resolution, id generation, filename
  sanitizing, store/resolve/delete/purge. No axum types, unit-testable directly.

Changed:
- src/importer.rs — ImportResult gains two fields (no signature change, see §2).
- src/server/routes/imports.rs — three POST handlers + router wiring.
- src/server/mod.rs — `pub mod uploads;` and a startup purge call in serve().
- Cargo.toml — axum "multipart" feature.
- docs/api.md, CLAUDE.md.

## 2. Data-layer change: extend ImportResult, do NOT change import_file's signature

The spec asks import_file to report the effective format and the created imports
row id. Doing that through the *return type* instead of the parameter list means
zero call-site churn: nothing outside importer.rs constructs ImportResult, and
every caller reads named fields.

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ImportResult {
        pub imported: usize,
        pub skipped: usize,
        pub malformed: usize,
        pub duplicate_file: bool,
        pub sample: Vec<ParsedRow>,
        pub format: Option<String>,      // NEW - resolved format key
        pub import_id: Option<i64>,      // NEW - created imports row
    }

- `format`: `Some(kind.key())` for a built-in; `Some(profile_name)` when the
  format key resolved through csv_profiles; `Some("generic")` for an inline
  mapping. `None` only on the duplicate-file short-circuit, which returns before
  format resolution — keeping that order preserves today's behaviour, where a
  re-imported file reports "already imported" rather than erroring on a bogus
  --format.
- `import_id`: `Some(conn.last_insert_rowid())` after the imports insert; `None`
  for dry runs and duplicate files.
- Implementation detail: `ResolvedImporter::Generic(config)` becomes
  `Generic { config, label: String }` so the profile-vs-inline distinction
  survives to the return value. Both existing construction sites in import_file
  are updated; nothing else constructs the struct.

Call sites — the complete list, verified by grep:
1. src/cli/import.rs:65 — reads `.duplicate_file/.imported/.skipped/.malformed/
   .sample`. No edit. Output byte-identical.
2. src/cli/import_manager.rs:292 — same fields. No edit. Output identical.
3. src/importer.rs — ~15 test call sites, all field reads, no construction, no
   exhaustive destructuring. No edits.

Correction to the spec: it says "the CLI prints this [the resolved format] but
the struct doesn't carry it". The CLI prints no format today — neither
cli/import.rs nor import_manager.rs mentions it. So there is no CLI output to
keep identical here; the new field is purely additive.

## 3. src/server/uploads.rs — the spool

Layout: `<data_dir>/tmp/uploads/<uploadId>/<sanitized-filename>`, where
`data_dir = state.db_path.parent()` (the same "the database we actually opened,
not settings.json" rule status.rs already follows).

A directory per upload rather than the spec's `<id>-<filename>` prefix, because
import_file records `file_path.file_name()` into imports.filename — with a
prefix scheme every import in the history would read
`a3f1…-statement.csv`. The directory carries the id; the file keeps the user's
name. It also makes purge a single remove_dir_all and lookup a single join.

    pub struct StoredUpload { pub id: String, pub path: PathBuf, pub filename: String, pub size: u64 }

    pub fn uploads_dir(db_path: &Path) -> PathBuf                  // <data_dir>/tmp/uploads
    pub fn new_id() -> String                                       // 16 random bytes, hex
    pub fn sanitize_filename(raw: &str) -> Result<String, String>   // see below
    pub fn store(dir: &Path, filename: &str, bytes: &[u8]) -> Result<StoredUpload>
    pub fn resolve(dir: &Path, id: &str) -> Option<StoredUpload>
    pub fn delete(dir: &Path, id: &str)                             // best-effort
    pub fn purge_stale(dir: &Path, max_age: Duration)               // best-effort

- `new_id`: `rand::thread_rng().fill_bytes(&mut [0u8; 16])` + `hex::encode`,
  exactly as server::auth::generate_token does. No uuid crate; rand and hex are
  already direct dependencies. tempfile stays dev-only.
- `sanitize_filename`: take `Path::new(raw).file_name()` (drops any directory
  component, and with it `..`), reject empty, map every byte outside
  `[A-Za-z0-9._-]` to `_`, collapse a leading `.`, truncate the stem so the
  whole name is <= 100 bytes, then require a case-insensitive extension in
  {csv, xlsx, xls} — rejection is a 400. The extension is load-bearing, not
  cosmetic: `detect_gusto_payroll` refuses anything not named `.xlsx` and
  `calamine::open_workbook_auto` picks its reader from the extension, so a
  stripped or normalised extension would silently break XLSX imports.
- `store`: create_dir_all on the uploads dir and the per-id dir,
  `restrict_dir_permissions` (0700) on both, write the bytes, then
  `restrict_file_permissions` (0600) on the file — reusing settings.rs's
  existing helpers, which are Unix-only no-ops elsewhere.
- `resolve`: reject any id that is not exactly 32 ASCII hex chars *before*
  touching the filesystem (belt-and-braces path traversal guard, since the id
  goes into a join), then read_dir the per-id directory and take its single
  file entry.
- `purge_stale`: for each child directory, if `modified()` is older than
  `max_age`, remove_dir_all. Errors are ignored — a temp file we cannot delete
  must not fail a request. Called with 1 hour at serve() startup and at the top
  of every upload request.

## 4. POST /api/imports/upload

Body: `multipart/form-data` with exactly one file field named `file` carrying a
filename. Other fields are ignored; a missing `file` field is 400.

    async fn upload(State(state): State<AppState>, mut multipart: Multipart)
        -> ApiResult<Json<UploadResponse>>

    #[derive(Serialize)] #[serde(rename_all = "camelCase")]
    struct UploadResponse { upload_id: String, filename: String, size: u64 }

- Size limit: `.layer(DefaultBodyLimit::max(25 * 1024 * 1024))` on this route
  only. axum's default is 2 MB, which would reject ordinary statements, and the
  limit wraps the request body in `http_body_util::Limited`, so it applies to
  multipart field reads too (confirmed against the axum 0.8 docs). The JSON
  routes keep the 2 MB default.
- `field.bytes().await` buffers the field in memory. Bounded at 25 MB by the
  layer above, and it avoids pulling in futures/tokio-util's StreamReader just
  to stream to disk. The write itself goes through `spawn_blocking`.
- Multipart errors are mapped by status, not by string:
  `err.status() == PAYLOAD_TOO_LARGE` -> 413 (see §7), anything else -> 400 with
  `err.body_text()`.
- Sequence: purge_stale(1h) -> read the field -> sanitize the filename ->
  new_id -> store -> respond.

## 5. POST /api/imports/preview

    #[derive(Deserialize)] #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct PreviewRequest {
        upload_id: String,
        account: String,
        format: Option<String>,
        mapping: Option<GenericCsvConfig>,   // Deserialize from 31.2
    }

Response: the serialized `ImportResult` — `{ format, duplicateFile, imported,
skipped, malformed, sample: [{date, description, amount}], importId: null }`.

Validation before any work:
- `format` and `mapping` together -> 400. (import_file silently lets the inline
  config win; the API refuses the ambiguity rather than guessing.)
- `format == "gusto_payroll"` while `!state.features.gusto` -> 400 naming the
  missing build feature (§7).
- `resolve(uploads_dir, upload_id)` -> None is 404 (§7).

Then one `spawn_blocking`: `get_connection(db_path)` -> `import_file(&conn,
&stored.path, &account, format.as_deref(), true, mapping.as_ref())`.

dry_run already skips the snapshot, the imports insert and every transaction
insert; it still computes the checksum, the duplicate-file lookup and the
per-row duplicate lookup, all reads. Nothing is deleted from the spool — the
same uploadId is what confirm consumes.

`sample` stays at 5 rows: import_file's cap is shared with the CLI's dry-run
printer, and widening it would change CLI output. If 31.14 wants a longer
preview that is a follow-up with its own decision.

## 6. POST /api/imports/confirm

    #[derive(Deserialize)] #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct ConfirmRequest {
        upload_id: String, account: String,
        format: Option<String>, mapping: Option<GenericCsvConfig>,
        save_profile: Option<String>,
    }

    #[derive(Serialize)] #[serde(rename_all = "camelCase")]
    struct ConfirmResponse {
        #[serde(flatten)] result: ImportResult,   // imported/skipped/malformed/duplicateFile/format/importId/sample
        categorized: usize,
        still_flagged: usize,
        snapshot: String,                          // absolute path of the pre-import snapshot
    }

Validation (all before the snapshot, so a bad request never leaves a snapshot
or a half-import behind):
- everything preview validates, plus
- `saveProfile` without `mapping` -> 400 ("Saving a profile needs a column
  mapping").
- `saveProfile` that is empty/whitespace -> 400.
- `saveProfile` that collides with a built-in importer key -> 400 up front,
  reusing `importer::get_by_key`. save_csv_profile enforces this too, but only
  after the import has already run, and it raises `NigelError::Other`, which the
  blanket mapping would turn into a 500.

Then one `spawn_blocking` holding one connection for the whole sequence,
mirroring import_manager.rs::run_import:

1. `backup::snapshot(&conn, data_dir.join("snapshots/pre-import-<stamp>.db"))`
   with `<stamp> = %Y%m%d-%H%M%S`, byte-identical to the CLI and TUI naming.
   NB: the spec asks for `snapshot_with_password(..., get_db_password())`;
   `snapshot()` *is* that call (backup.rs:14). Calling the wrapper keeps one
   code path — the explicit variant exists only for tests that cannot use the
   password global, and the server's own connection already depends on it.
2. `import_file(..., dry_run = false, ...)`.
3. If `duplicate_file`, stop here and answer 200 with zeros and
   `importId: null` — exactly the CLI's early return, which also skips
   categorization. The snapshot has already been taken, again as in the CLI.
4. `categorize_transactions(&conn)` -> categorized / still_flagged.
5. `save_csv_profile(&conn, name, mapping)` when requested. After the import, not
   before as in the CLI: a failed import should not leave a profile behind. The
   upsert is idempotent, so re-running is harmless.

No wrapping transaction: import_file inserts row by row and categorize runs
separately, exactly as the CLI does. The pre-import snapshot is the recovery
path, and diverging here would change import semantics rather than expose them.

On any 200 (including duplicateFile) the spooled upload directory is deleted. On
an error it is left in place so the client can retry the same uploadId; the 1h
purge collects it either way.

## 7. Error mapping

| Case | Code | Status |
|---|---|---|
| unknown/expired uploadId, malformed id | `not_found` + `details.reason = "upload_not_found"` | 404 |
| unknown account (NigelError::UnknownAccount) | `not_found` | 404 |
| unknown format key / no profile (UnknownFormat) | `bad_request` | 400 |
| no importer for the account type (NoImporter) | `bad_request` | 400 |
| `format: "gusto_payroll"` in a no-gusto build | `bad_request` | 400 |
| both `format` and `mapping`; bad saveProfile | `bad_request` | 400 |
| disallowed extension, missing `file` field, malformed multipart | `bad_request` | 400 |
| parse failure (NigelError::Csv, or Other raised by a parser) | `bad_request` | 400 |
| upload over 25 MB | `payload_too_large` | 413 |
| duplicate file (checksum) | **not an error** — 200, `duplicateFile: true` | 200 |
| malformed rows | **not an error** — 200, counted in `malformed` | 200 |
| database not unlocked (31.4 guard) | `locked` | 423 |
| snapshot/IO failure, JoinError | `internal` | 500 |

Three notes on this table:

- `upload_not_found` is not an ApiErrorCode. The epic spec fixes the code list,
  so rather than add a tenth code for one case I use `not_found` with
  `details: {"reason": "upload_not_found"}` — the same discriminator pattern the
  epic already prescribes for `conflict`. The SPA can still branch on it.
- Parse failures: the existing `From<NigelError>` sends `Csv` and `Other` to 500.
  The import routes therefore map import_file's error through a local
  `import_error(NigelError) -> ApiError`: UnknownAccount -> 404;
  UnknownFormat/NoImporter/Csv/Other -> 400 carrying the parser's message;
  Io/Db -> 500. `Other` goes to 400 because on this code path it is raised by
  parsers ("Failed to open XLSX: …"); the profile-name collision, the other
  `Other` producer here, is pre-validated away in §6. Scoped to these three
  handlers — the global mapping is untouched.
- Gusto: with the feature off, `get_by_key("gusto_payroll")` returns None and
  the key falls through to a csv_profiles lookup, ending as
  `UnknownFormat("gusto_payroll")` — a 400 whose message misleadingly suggests
  a typo. The explicit pre-check replaces it with "This build has no Gusto
  payroll support (rebuild with the `gusto` feature)." Per the spec this is a
  400, not the 501 `feature_disabled` the epic uses for PDF (see §12).

## 8. Cargo.toml

    axum = { version = "0.8.9", optional = true, default-features = false,
             features = ["http1", "json", "query", "tokio", "multipart"] }

One added feature, no new top-level crate. `multipart` pulls multer transitively;
it is inside the existing optional `axum` dep, so `--no-default-features` builds
are unaffected. No tempfile, no uuid, no tower-http: rand + hex cover ids and
settings.rs covers permissions.

## 9. Docs

- docs/api.md — a new "Imports" section under Endpoints covering the three
  routes: request/response shapes with JSON examples, the mutual exclusion of
  `format` and `mapping`, the format-key vocabulary (built-in keys, profile
  names, `"generic"`), the 25 MB limit, the .csv/.xlsx/.xls allowlist, the 1h
  expiry, that preview mutates nothing, that confirm snapshots first, and that a
  duplicate file is a 200 rather than an error. The error table gains
  `payload_too_large` / 413 and a note on `details.reason = "upload_not_found"`.
- CLAUDE.md — Architecture: `server/uploads.rs` in the Web server bullet, and the
  ImportResult `format`/`importId` fields on the Importers bullet. Project
  Structure: `server/uploads.rs`. Key Design Constraints: one line — browser
  uploads spool to `<data_dir>/tmp/uploads/<id>/<name>` (0700/0600), are capped
  at 25 MB with a .csv/.xlsx/.xls allowlist, are purged after an hour at startup
  and on each upload, and are deleted on a successful confirm; the confirm
  endpoint runs the same snapshot -> import -> categorize sequence as the TUI.
- README.md — no change (serve is described; endpoint detail lives in api.md).

## 10. Tests

src/server/uploads.rs (unit, feature-gated but no HTTP):
- sanitize_filename: strips directories and `..`, maps hostile bytes, keeps the
  extension case-insensitively, rejects `.txt`/`.pdf`/no-extension/empty,
  truncates long names while preserving the extension.
- store then resolve round-trips id, filename and size; on Unix the file is 0600
  and both directories 0700.
- resolve rejects non-hex, wrong-length, and `../…` ids without touching disk.
- purge_stale removes a directory backdated past the cutoff and keeps a fresh
  one (set mtime with filetime-free `std::fs::File::set_modified`).

src/server/routes/imports.rs (integration, via testutil::app + tower oneshot;
multipart bodies are hand-built strings with a fixed boundary — no new dev-dep):
1. Happy path: upload a fixture BofA checking CSV -> preview -> confirm.
   Preview reports `format: "bofa_checking"` and non-zero `imported`; confirm
   reports the same `imported`, a non-null `importId`, and categorize counts.
2. Preview mutates nothing: `SELECT count(*)` on transactions, imports and
   accounts before and after are equal, and `sample` is non-empty.
3. Confirm side effects: a `snapshots/pre-import-*.db` file exists, the imports
   row exists with the *original* filename (not the uploadId), transactions
   carry that import_id, and `GET /api/imports` (31.5) lists it.
4. Duplicate: upload the same bytes again and confirm -> 200 with
   `duplicateFile: true`, `imported: 0`, `importId: null`, and the transaction
   count unchanged.
5. Generic CSV: upload a CSV in a non-BofA column order, preview with an inline
   `mapping` -> `format: "generic"` and correct counts; confirm with
   `saveProfile: "chase"` -> `GET /api/csv-profiles` then lists `chase` with the
   same mapping (round-trip through 31.5's endpoint).
6. Oversize: a body over 25 MB -> 413 `payload_too_large`.
7. Expired/unknown uploadId on both preview and confirm -> 404 with
   `details.reason == "upload_not_found"`; a traversal-shaped id likewise 404.
8. Cleanup: after a successful confirm the uploads directory is empty; after a
   failed confirm (unknown account) the upload survives and the same uploadId
   still previews.
9. Error cases: unknown account -> 404; `format: "nope"` -> 400; `format` and
   `mapping` together -> 400; `saveProfile` without `mapping` -> 400;
   `saveProfile: "bofa_checking"` -> 400 *and* no import happened; a `.txt`
   upload -> 400.
10. Locked: with an encrypted, un-unlocked database all three routes return 423
    (joins 31.5's table-driven guard test rather than duplicating it).

src/importer.rs (unit, runs under --no-default-features):
11. `format` is the built-in key on auto-detect, the profile name when the key
    resolved through csv_profiles, `"generic"` for an inline mapping, and `None`
    on the duplicate-file short-circuit.
12. `import_id` is `None` for dry runs and `Some(id)` after a real import, and
    that id matches the row transactions were linked to.

Fixtures are written inline in the tests (a small `write_bofa_csv` mirroring the
one already in importer.rs's test module) — the repository ships no CSV files.

## 11. Verification matrix

1. cargo fmt --check
2. cargo build
3. cargo test
4. cargo test --no-default-features
5. cargo test --no-default-features --features serve
6. cargo clippy --all-targets -- -D warnings
7. cargo clippy --no-default-features --all-targets -- -D warnings
   — expected to report exactly the 2 known task-34 lints
   (cli/dashboard.rs:852, cli/report/mod.rs:160) and nothing else
8. Manual smoke: `cargo run -- serve --no-open`, then curl -F upload, preview,
   confirm against a real statement; confirm the snapshot file, the imports row
   filename, an empty tmp/uploads afterwards, and a 413 on a 30 MB file.

## 12. Open decisions (flagged for approval, defaults noted)

a. **413 `payload_too_large` is a tenth error code.** The epic fixes the code
   list and does not include it. Default: add it — AC #5 is about enforcing a
   size limit and 413 is the honest status. Fallback if you'd rather freeze the
   list: 400 with a size-specific message.
b. **Gusto-off is 400, not 501.** The 31.7 spec says 400; the epic reserves 501
   `feature_disabled` for exactly this shape of problem (its example is PDF in a
   no-pdf build). Following the task spec (400). Say the word and it becomes 501.
c. **Per-upload directory instead of `<id>-<filename>`.** Required so
   imports.filename records the user's filename; noted in §3.
d. **`GET /api/imports/formats` is NOT in this plan.** Nothing exposes the
   built-in importer keys/names, so 31.14's format picker has no source for
   anything but saved profiles. The epic forbids screen tasks from adding
   endpoints, which makes this mine by elimination. It is ~15 lines over
   ALL_IMPORTERS (already gusto-cfg'd) returning `[{key, name, accountTypes}]`.
   Add it on your word; left out by default to keep to the spec's three
   endpoints.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented the approved plan with the coordinator's four rulings: 413 `payload_too_large` added as a new `ApiErrorCode`; gusto-without-the-feature answers 501 `feature_disabled` (not the 400 the task spec asked for); per-upload directory spool; `GET /api/imports/formats` in scope.
- `import_file`'s signature is unchanged. `ImportResult` gained `format: Option<String>` and `import_id: Option<i64>` instead, so all 17 call sites (cli/import.rs, cli/import_manager.rs, 15 tests) needed no edit and CLI/TUI output is byte-identical. `ResolvedImporter::Generic` now carries the name to report — a profile name, or `generic` for an inline mapping.
- Spool is `<data_dir>/tmp/uploads/<32 hex>/<sanitized name>`, dirs 0700 and file 0600 via the existing `settings::restrict_*_permissions`. The directory carries the id so the file keeps the user's filename, which is what `import_file` writes into `imports.filename` — verified live: the history row reads `april-2025.csv`, not an id.
- Confirm validates the account before snapshotting (`ensure_account_exists`), so a rejected confirm leaves no snapshot behind. This is a small, deliberate divergence from the CLI, which snapshots first and then fails.
- No new crate. `axum`'s `multipart` feature only (pulls multer transitively); ids reuse `rand` + `hex` exactly as `auth::generate_token` does.
- Verification (8 commands): fmt clean; build clean; `cargo test --test-threads=1` 460+25 pass (was 430+25 — 30 new tests); `--no-default-features` 334+26 pass; `--no-default-features --features serve` 454+25 pass; clippy default clean; clippy no-default-features reports exactly the 2 known task-34 `needless_return` lints (cli/dashboard.rs:852, cli/report/mod.rs:160) and nothing else.
- Manual curl smoke against a live `nigel serve` on an isolated HOME: formats list, upload (0600 file confirmed), preview, confirm (snapshot file written, importId 1, tmp/uploads emptied), duplicate re-confirm, expired id 404 with `details.reason=upload_not_found`, unknown account 404, `.txt` 400, format+mapping 400, 30 MB upload 413 with nothing reaching disk, inline mapping + saveProfile round-tripping through `/api/csv-profiles`. A second run proved preview mutates nothing: 274 register rows before and after, imports count unchanged, upload still resolvable.
- Known pre-existing limitation, not introduced here: two imports inside the same second write the same `pre-import-<YYYYmmdd-HHMMSS>.db` snapshot name and the later one clobbers the earlier. The CLI and TUI have always named snapshots this way; changing it would diverge from them, so it is left alone.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the JSON API import pipeline, so a browser can do what `nigel import` does: upload a statement, look at what it would change, and then commit it.

## Endpoints

- `POST /api/imports/upload` — multipart, one file. Spools to `<data_dir>/tmp/uploads/<32 hex>/<sanitized name>` (dir 0700, file 0600) and answers `{uploadId, filename, size}`. 25 MB `DefaultBodyLimit`, `.csv`/`.xlsx`/`.xls` only — the extension is kept because two importers dispatch on it. Uploads expire after an hour, purged at startup and before each new upload.
- `POST /api/imports/preview` — `import_file` with `dry_run`, returning the resolved format, five sample rows, and the imported/skipped/malformed counts. Writes nothing: no snapshot, no `imports` row, no transactions.
- `POST /api/imports/confirm` — the terminal UI's sequence unchanged: pre-import snapshot, import, auto-categorize, then an optional `saveProfile`. Answers the preview shape plus `categorized`, `stillFlagged`, `importId`, and the snapshot path. Deletes the upload on success and keeps it on failure so the same id can be retried.
- `GET /api/imports/formats` — the built-in importers this binary has (`{key, name, accountTypes}`). Approved as in-scope: nothing else exposed them, and 31.14's format picker has no other source.

`format` and `mapping` are mutually exclusive (400 rather than a guess). A duplicate file and malformed rows are data, not errors — `duplicateFile: true` with zero counts, exactly what the CLI prints.

## Data layer

`import_file`'s signature is untouched; `ImportResult` gained `format` (the importer that actually ran — a built-in key, a profile name, or `generic`) and `import_id` (the batch created). That keeps all 17 call sites and the CLI's and TUI's output byte-identical, which changing the parameter list would not have. `built_in_formats()` is new next to `get_by_key`.

## Error mapping

New `payload_too_large` / 413 code for oversize uploads. An unknown or expired `uploadId` is a 404 carrying `details.reason: "upload_not_found"`, following the existing `conflict` precedent rather than adding a code for one case. A format key this build lacks (`gusto_payroll` without the feature) is 501 `feature_disabled`, matching the PDF precedent. Parse failures map to 400 through a route-scoped mapper — the global `From<NigelError>` sends `Csv`/`Other` to 500, which is right everywhere except here, where they mean "this file is not what you said it was".

## Tests

30 new: 13 unit tests over the spool (sanitizing, traversal, permissions, purge), 13 integration tests over the routes (happy path, preview-mutates-nothing, snapshot and filename provenance, duplicate re-confirm, inline mapping and saveProfile round-trip, 413, 404s, refused-request table, retry-after-failure, stale collection), and 4 in `importer.rs` covering the new fields under `--no-default-features`. The three POSTs joined the locked-guard table, so all of them are proven to refuse an un-unlocked database.

- `cargo test --test-threads=1`: 460 + 25 pass (was 430 + 25)
- `--no-default-features`: 334 + 26; `--no-default-features --features serve`: 454 + 25
- fmt and clippy clean; clippy `--no-default-features` reports only the 2 known task-34 lints
- Manual curl smoke against a live server covered every endpoint and every error case, including a 30 MB upload rejected without touching disk

## Docs

`docs/api.md` gains a "Running an import" section (the three steps, both request shapes, what is data versus an error, the failure table) plus the 413 row and the formats route. `CLAUDE.md` covers `server/uploads.rs`, the new `ImportResult` fields, and a design-constraint line for the pipeline.

## Risks

One pre-existing limitation is now easier to hit: two imports in the same second share a `pre-import-<timestamp>.db` snapshot name and the second clobbers the first. The CLI has always named snapshots this way, so it is left alone rather than diverging.
<!-- SECTION:FINAL_SUMMARY:END -->
