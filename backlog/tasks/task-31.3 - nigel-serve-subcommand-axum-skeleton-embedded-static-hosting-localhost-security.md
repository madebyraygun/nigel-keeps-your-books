---
id: TASK-31.3
title: >-
  nigel serve subcommand: axum skeleton, embedded static hosting, localhost
  security
status: Done
assignee:
  - '@agent-31.3'
created_date: '2026-08-06 16:25'
updated_date: '2026-08-06 19:33'
labels:
  - web
  - backend
dependencies:
  - TASK-31.1
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.3-serve-skeleton.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add axum + tokio and a serve subcommand: bind 127.0.0.1 with a --port flag, serve embedded SPA assets (rust-embed) at /, mount an /api router, and run the same pre-flight as the dashboard (init check, migrations). Localhost is not a trust boundary by itself: generate a per-session auth token, open the browser with a tokenized URL that sets a cookie, validate the Host header to block DNS-rebinding, and reject cross-origin requests. Graceful shutdown on Ctrl-C.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nigel serve starts on 127.0.0.1 with --port and --no-open flags working, and prints or opens a tokenized URL
- [x] #2 Requests without a valid session are rejected with 401; non-localhost Host headers are rejected with 403
- [x] #3 Static SPA assets are served from the binary with no filesystem dependency, and /api routes are mounted
- [x] #4 Ctrl-C shuts the server down cleanly
- [x] #5 Builds pass with and without the pdf/gusto features
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## 0. Ground rules

Single commit series on `nigel-31`. No data endpoints (31.4-31.8), no real SPA
(31.9). `cargo test --no-default-features` stays green. Docs ship in the same
commit.

## 1. Cargo.toml

Features:

```toml
[features]
default = ["gusto", "pdf", "serve"]
gusto = ["dep:calamine"]
pdf = ["dep:printpdf"]
serve = ["dep:axum", "dep:tokio", "dep:tower", "dep:rust-embed", "dep:open", "dep:subtle"]
```

Optional deps (versions verified against crates.io on 2026-08-06):

```toml
axum       = { version = "0.8.9", optional = true, default-features = false,
               features = ["http1", "json", "query", "tokio"] }
tokio      = { version = "1.53",  optional = true,
               features = ["rt-multi-thread", "net", "signal", "macros", "time"] }
tower      = { version = "0.5",   optional = true, features = ["util"] }
rust-embed = { version = "8.12",  optional = true, features = ["debug-embed", "mime-guess"] }
open       = { version = "5.4",   optional = true }
subtle     = { version = "2.6",   optional = true }
```

Justification per dep:

- **axum 0.8.9** — current 0.8 line. `default-features = false` drops
  `form`/`matched-path`/`original-uri`/`tower-log`/`tracing`; we keep `http1`
  (localhost only, no h2), `tokio` (`axum::serve` + `TcpListener`), `json`
  (`Json` responses for ping + the error envelope), `query` (`Query<AuthQuery>`
  on `/auth`). Verified API surface for 0.8: `middleware::from_fn_with_state`,
  `Next`, `axum::serve(listener, app).with_graceful_shutdown(fut)`,
  `Router::fallback`, `axum::body::to_bytes`.
- **tokio 1.53** — `rt-multi-thread` + `net` + `signal` per the epic spec;
  `macros`/`time` are for the in-crate `#[tokio::test]`s and the shutdown-test
  timeout. tokio 1.x + hyper 1.x are *already* in the tree via reqwest, so the
  incremental build cost is small.
- **tower 0.5** with `util` — only for `ServiceExt::oneshot` in tests. It is
  already a transitive dep of axum with the same feature, so this adds zero
  crates to the tree. Declaring it here (rather than in `[dev-dependencies]`)
  keeps it out of `--no-default-features` builds entirely — dev-dependencies
  cannot be optional, so a dev-dep would be compiled even for the no-serve test
  run.
- **rust-embed 8.12** with `debug-embed` (assets embedded in debug builds too —
  required by AC #3 "no filesystem dependency", and makes the static tests
  deterministic regardless of cwd) and `mime-guess` (gives
  `file.metadata.mimetype()`, so no direct `mime_guess` dep).
- **open 5.4** — `open::that_detached(url)` (does not block on the browser
  process).
- **subtle 2.6** — `ConstantTimeEq` for the token compare. Zero-dependency
  crate. See open question Q1 if you would rather hand-roll.

Not added: `tower-http` (epic says "only if needed for limits" — no request
bodies exist yet; axum's `Json` extractor already caps at 2 MB), any tracing
stack.

## 2. src/lib.rs

```rust
#[cfg(feature = "serve")]
pub mod server;
```

## 3. src/server/ layout

**`mod.rs`** — router assembly + startup.

- `pub fn run(port: u16, no_open: bool) -> crate::error::Result<()>` — sync
  entry point (main stays sync). Calls `colored::control::set_override(false)`,
  builds a multi-thread runtime via
  `tokio::runtime::Builder::new_multi_thread().enable_all().build()`, then
  `rt.block_on(serve(...))`.
- `async fn serve(db_path, port, no_open) -> Result<()>` — generates the session
  token, builds `AppState`, binds `TcpListener::bind((Ipv4Addr::LOCALHOST,
  port))`, reads `local_addr()` (so `--port 0` reports the real port), prints
  the banner + `http://127.0.0.1:<port>/auth?token=<t>`, `open::that_detached`
  unless `no_open` (failure to open is non-fatal — the URL is already printed),
  then delegates to `serve_with_shutdown`.
- `async fn serve_with_shutdown(listener, state, shutdown: impl Future<Output=()> + Send + 'static)`
  — `axum::serve(listener, build_router(state)).with_graceful_shutdown(shutdown).await?`.
  Split out purely so the shutdown test can drive it with a oneshot receiver
  instead of Ctrl-C.
- `async fn shutdown_signal()` — `tokio::signal::ctrl_c()`, plus SIGTERM via
  `signal::unix::SignalKind::terminate()` under `#[cfg(unix)]`, selected with
  `tokio::select!`.
- `fn build_router(state: AppState) -> Router` — private, the unit under test:

```rust
let api = routes::api_router()                       // includes its own JSON 404 fallback
    .layer(middleware::from_fn_with_state(state.clone(), auth::session_guard));

Router::new()
    .nest("/api", api)
    .route("/auth", get(auth::auth_redirect))
    .fallback(static_files::static_handler)          // SPA + assets, no session
    .layer(middleware::from_fn_with_state(state.clone(), auth::host_guard))
    .with_state(state)
```

`.layer` applied outermost runs first, so the effective order is
**Host/Origin -> session -> routes**, exactly as specified. Note the session
layer is `.layer` and not `.route_layer` deliberately: `route_layer` skips the
nested router's fallback, which would let an unauthenticated `GET /api/nope`
reach the 404 handler. With `.layer` the whole `/api` subtree (fallback
included) is 401 without a cookie. `/auth` and the static fallback sit outside
that nest, which is how the spec's "exempting /auth and static assets" is
satisfied structurally rather than by a path allowlist.

**`state.rs`**

```rust
#[derive(Clone)]
pub struct AppState {
    pub db_path: Arc<PathBuf>,
    pub session_token: Arc<str>,
    pub features: Features,          // { pdf: bool, gusto: bool } from cfg!(feature = ...)
}
impl AppState { pub fn new(db_path: PathBuf, session_token: String) -> Self }
```

`Arc` so cloning per-request/per-layer is cheap. No connection and no pool —
per the epic, handlers open their own connection inside `spawn_blocking`
(nothing in 31.3 touches the DB). `features` is here now because 31.8 needs it
for `feature_disabled`.

**`auth.rs`** — everything security-relevant, all pure helpers testable without
HTTP:

- `pub fn generate_token() -> String` — 32 bytes from `rand::thread_rng()`
  (ThreadRng is a CSPRNG) via `RngCore::fill_bytes`, `hex::encode`. Both crates
  already in the tree; no new dep.
- `pub fn tokens_match(a: &str, b: &str) -> bool` — rejects empty on either
  side, then `subtle::ConstantTimeEq` over the bytes (length compared
  non-secretly first, which is standard — token length is not a secret).
- `pub fn host_is_local(host: &str) -> bool` — strips the port: `[::1]:5731` ->
  `::1` (bracket form), otherwise reject anything with more than one `:`, else
  split at the single `:`. Compares the result ASCII-case-insensitively against
  the exact set `{localhost, 127.0.0.1, ::1}`. Exact match (not suffix, not
  "any 127/8") is what makes `127.0.0.1.evil.com`, `localhost.evil.com`,
  `evil.com`, `0.0.0.0`, `192.168.1.5` and `[::]` all fail.
- `pub fn origin_is_local(origin: &str) -> bool` — requires an `http://` or
  `https://` prefix, takes the authority up to the first `/`, strips any
  `user@`, then defers to `host_is_local`. Literal `null` and `file://` are
  rejected.
- `pub fn cookie_value<'a>(cookies: &'a str, name: &str) -> Option<&'a str>` —
  split on `;`, trim, `split_once('=')`, exact name match (so `xnigel_session=`
  does not collide).
- `pub async fn host_guard(State<AppState>, req: Request, next: Next) -> Response`
  — missing Host, non-UTF-8 Host, or `!host_is_local` -> `ApiError::forbidden()`.
  If an `Origin` header is present it must also pass `origin_is_local`; absent
  Origin is allowed (navigations and non-browser clients omit it). Runs for
  every path including static, per spec.
- `pub async fn session_guard(State<AppState>, req: Request, next: Next) -> Response`
  — concatenates all `Cookie` header values, looks up `nigel_session`,
  `tokens_match` against `state.session_token`; otherwise `ApiError::unauthorized()`.
- `pub async fn auth_redirect(State<AppState>, Query<AuthQuery>) -> Response` —
  `AuthQuery { token: String }`; on mismatch `ApiError::unauthorized()`; on
  match: `302 Found`, `Location: /`, and
  `Set-Cookie: nigel_session=<token>; HttpOnly; SameSite=Strict; Path=/`.
  No `Secure` (plain http on loopback), no `Max-Age`/`Expires` so it is a
  browser-session cookie that dies with the browser.

**`error.rs`**

```rust
pub enum ApiErrorCode { BadRequest, Unauthorized, Forbidden, NotFound, Conflict, Locked, Internal, FeatureDisabled }
impl ApiErrorCode { fn status(self) -> StatusCode; fn as_str(self) -> &'static str; }

pub struct ApiError { code: ApiErrorCode, message: String, details: Option<serde_json::Value> }
impl ApiError {
    pub fn bad_request(msg) -> Self; pub fn unauthorized() -> Self; pub fn forbidden() -> Self;
    pub fn not_found(msg) -> Self;   pub fn conflict(msg, details) -> Self;
    pub fn locked() -> Self;         pub fn feature_disabled(msg) -> Self;
    pub fn internal(e: impl Display) -> Self;
    pub fn with_details(self, v: serde_json::Value) -> Self;
}
impl IntoResponse for ApiError            // { "error": { "code", "message", "details"? } }
impl From<crate::error::NigelError> for ApiError   // NotInitialized/Unknown* -> not_found, Db/Io -> internal
pub type ApiResult<T> = Result<T, ApiError>;
```

Status mapping is the epic's table verbatim (400/401/403/404/409/423/500/501).
Codes are the snake_case strings `bad_request`, `unauthorized`, `forbidden`,
`not_found`, `conflict`, `locked`, `internal`, `feature_disabled`. Serialization
via a private `#[derive(Serialize)] #[serde(rename_all = "camelCase")]` envelope
struct. `internal` never echoes the underlying error into `message` verbatim if
it could carry a path/password — it uses the Display of `NigelError`, which
today contains no secrets; the session token and DB password are never formatted
into any response or any print other than the startup URL.

**`routes/mod.rs`**

- `pub fn api_router() -> Router<AppState>` — `.route("/ping", get(ping))` plus
  `.fallback(api_not_found)`.
- `async fn ping() -> Json<PingResponse>` where
  `PingResponse { ok: true, version: env!("CARGO_PKG_VERSION") }`,
  `#[serde(rename_all = "camelCase")]`. Touches no DB, so tests need no fixture.
- `async fn api_not_found(uri: Uri) -> ApiError` — `not_found` with the path in
  the message; guarantees unknown `/api/*` returns JSON, never the SPA HTML.
- Doc comment noting 31.4+ add one file per domain here.

**`static_files.rs`**

```rust
#[derive(rust_embed::Embed)]
#[folder = "web/dist/"]
struct Assets;

pub async fn static_handler(uri: Uri) -> Response
```

Logic: trim the leading `/`; empty -> `index.html`; reject any path segment
equal to `..` (defensive — with `debug-embed` the lookup is always against the
compiled-in map, never the filesystem); `Assets::get(path)` -> 200 with
`Content-Type` from `file.metadata.mimetype()` and the bytes; on miss, serve
`index.html` as 200 `text/html; charset=utf-8` (SPA deep links); if even
`index.html` is missing, 500 with a plain-text "web assets missing" body. No
cache headers in this task (31.9 owns hashed-asset caching).

## 4. CLI wiring

**`src/cli/mod.rs`** — `pub mod serve;` and a new variant:

```rust
/// Serve the web UI and JSON API on localhost.
Serve {
    /// Port to bind on 127.0.0.1 (0 picks an ephemeral port)
    #[arg(long, default_value_t = 5731)]
    port: u16,
    /// Do not open a browser window
    #[arg(long)]
    no_open: bool,
},
```

The variant exists in **all** builds (mirroring pdf, where the flag parses and
the error comes at dispatch time).

**`src/cli/serve.rs`** — the feature seam plus the serve-specific pre-flight:

```rust
#[cfg(feature = "serve")]
pub fn run(port: u16, no_open: bool) -> Result<()> {
    let db_path = crate::settings::get_data_dir().join("nigel.db");
    // Encrypted DBs cannot be migrated before unlock; 31.4 runs migrations
    // after a successful POST /api/unlock.
    if !crate::db::is_encrypted(&db_path)? {
        let conn = crate::db::get_connection(&db_path)?;
        crate::db::init_db(&conn)?;
    }
    crate::server::run(port, no_open)
}

#[cfg(not(feature = "serve"))]
pub fn run(port: u16, no_open: bool) -> Result<()> {
    let _ = (port, no_open);
    Err(crate::error::NigelError::Other(
        "`nigel serve` requires the 'serve' feature - build with `cargo build --features serve`".into(),
    ))
}
```

**`src/main.rs` dispatch deltas** — exactly two:

1. Add `Commands::Serve { .. }` to the `needs_password` exclusion list (serve is
   password-exempt; the DB unlocks over HTTP in 31.4). Because the generic
   migration guard is `needs_existing_db && needs_password && !replaces_db`,
   this one change *also* removes serve from the generic migration, which is why
   `cli::serve::run` does its own conditional `init_db`.
2. Add the match arm `Commands::Serve { port, no_open } => cli::serve::run(port, no_open)`.

`Commands::Serve` stays inside `needs_existing_db`, so `nigel serve` with no
database returns `NotInitialized` — same posture as every other data command.
The comment above `needs_password` gets one clause added for serve. The comment
above the migration block gets a sentence noting serve migrates itself.

## 5. web/dist placeholder

New committed file `web/dist/index.html` — a self-contained page (no external
refs, inline `<style>`) reading roughly "Nigel web UI - the SPA has not been
built. Run `npm run build` in `web/`." plus a marker string the static tests
assert on. `.gitignore` currently has no `dist/` rule, so no change is needed;
31.9 owns whatever ignore rules the vite build wants.

## 6. Tests

Unit tests live in `#[cfg(test)] mod tests` inside `src/server/auth.rs`;
router-level tests in `src/server/mod.rs`. In-crate (not `tests/`) so tokio and
tower stay behind the `serve` feature — a `[dev-dependencies]` entry cannot be
optional and would be compiled even for the `--no-default-features` run.

**Unit — `auth.rs`**

- `host_is_local` accepts: `localhost`, `localhost:5731`, `LocalHost:5173`,
  `127.0.0.1`, `127.0.0.1:5173`, `[::1]`, `[::1]:5731`.
- rejects: `evil.com`, `evil.com:5731`, `127.0.0.1.evil.com`,
  `localhost.evil.com`, `0.0.0.0:5731`, `192.168.1.5:5731`, `[::]`, `""`,
  `127.0.0.1:5731:80`.
- `origin_is_local` accepts `http://localhost:5173`, `http://127.0.0.1:5731`,
  `https://localhost`; rejects `http://evil.com`, `null`, `file://`,
  `http://127.0.0.1.evil.com`.
- `tokens_match`: equal -> true; one-char difference -> false; different length
  -> false; empty/empty -> false.
- `cookie_value`: single pair; multiple pairs with spaces; missing name;
  prefix-collision `xnigel_session=`; empty header; value is returned intact.
- `generate_token` returns 64 hex chars and two calls differ.

**Router-level — `mod.rs`, `#[tokio::test]` + `tower::ServiceExt::oneshot`**
(no bound port, no DB fixture — ping touches nothing). Helper
`fn test_app() -> (Router, String)` returns a fresh router plus its token, since
`oneshot` consumes the service. Bodies read with `axum::body::to_bytes`. The six
spec scenarios:

1. `GET /api/ping`, `Host: 127.0.0.1:5731`, no cookie -> 401, JSON,
   `error.code == "unauthorized"`.
2. `GET /api/ping`, `Host: evil.com`, valid cookie -> 403,
   `error.code == "forbidden"` (proves the host layer runs *before* session).
3. `GET /auth?token=<t>` -> 302, `location: /`, `set-cookie` contains
   `nigel_session=<t>`, `HttpOnly`, `SameSite=Strict`, `Path=/`; replaying that
   cookie against `GET /api/ping` -> 200 with `ok: true`. Companion case:
   `/auth?token=wrong` -> 401 and no `set-cookie`.
4. `GET /` -> 200, `content-type` starts with `text/html`, body contains the
   placeholder marker.
5. `GET /some/deep/spa/route` -> 200 and byte-identical to the `/` body.
6. `GET /api/does-not-exist` with a valid cookie -> 404, `content-type`
   `application/json`, `error.code == "not_found"`, body is not HTML.

Plus a graceful-shutdown test (AC #4): bind `127.0.0.1:0`, spawn
`serve_with_shutdown` with a `tokio::sync::oneshot` receiver as the signal, fire
it, and assert the join handle resolves `Ok(())` inside a
`tokio::time::timeout`.

**CLI-level — `tests/cli_dispatch.rs`** (existing `TestEnv` harness):

- `nigel serve --help` exits 0 and mentions `--port` and `--no-open` (runs in
  every feature combo; never starts a server).
- Uninitialized env: `nigel serve` fails with "Not initialized".
- `#[cfg(not(feature = "serve"))]` only: after `nigel init`, `nigel serve` fails
  with a message containing "requires the 'serve' feature". Gated so the default
  build never launches a server from the test suite (it would hang).

## 7. Docs (same commit)

- **CLAUDE.md** — Architecture: new `**Web server:**` bullet describing
  `src/server/` (mod/auth/error/state/routes + static_files), the middleware
  order, the token/cookie flow, rust-embed hosting, and the fact that serve is
  the only async entry point. Commands: `nigel serve`, `nigel serve --port 8080`,
  `nigel serve --no-open`. Project Structure: `src/cli/serve.rs`, the whole
  `src/server/` subtree, `web/dist/index.html`. Key Design Constraints: serve is
  password-exempt in the dispatch pre-flight and migrates only when the DB is
  unencrypted; localhost is not a trust boundary (bind 127.0.0.1 + Host/Origin
  403 + session cookie 401); the token is never persisted and never logged.
  Also update the existing migrations constraint sentence to list `serve` among
  the password-exempt commands.
- **README.md** — Features bullet ("Web UI on localhost via `nigel serve`"),
  a Quick Start line, and a `serve` row in the Feature Flags table (Default:
  Yes). Note that `--no-default-features` now also drops the web server.
- **docs/api.md** — new stub: base URL, camelCase convention, the error-envelope
  shape and full code/status table, the auth flow (`GET /auth?token=` ->
  cookie -> 302), `GET /api/ping`, and the Host/Origin rule. States that every
  API task appends to this file.

## 8. Verification (all must pass before handing back)

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --no-default-features --all-targets -- -D warnings
cargo clippy --no-default-features --features serve --all-targets -- -D warnings
cargo test -- --test-threads=1
cargo test --no-default-features -- --test-threads=1
cargo test --no-default-features --features serve -- --test-threads=1
cargo build --release
```

Manual smoke (recorded in the task notes): `cargo run -- serve --no-open
--port 0`, then against the printed port — `curl -i /api/ping` (401),
`curl -i -H 'Host: evil.com' /api/ping` (403), `curl -i -c jar '/auth?token=...'`
(302 + Set-Cookie), `curl -i -b jar /api/ping` (200), `curl -i -b jar /`
(placeholder HTML), `curl -i -b jar /api/nope` (JSON 404), then Ctrl-C for a
clean exit.

**CI change:** `.github/workflows/ci.yml` currently runs `cargo test` and
`cargo test --no-default-features`. Add a third step
`cargo test --no-default-features --features serve -- --test-threads=1` to cover
the matrix the spec requires. `cargo clippy -- -D warnings` stays as-is (default
features now include serve, so the new code is linted).

## 9. Build order

1. Cargo.toml + `lib.rs` module + `web/dist/index.html` placeholder.
2. `state.rs`, `error.rs` (+ unit test for the envelope shape).
3. `auth.rs` helpers + their unit tests (TDD: validators first).
4. `auth.rs` middleware/handler, `routes/mod.rs`, `static_files.rs`.
5. `mod.rs` router assembly + the six router tests + shutdown test.
6. `cli/serve.rs`, `cli/mod.rs` variant, `main.rs` dispatch deltas,
   `tests/cli_dispatch.rs` cases.
7. Docs + CI + full verification sweep.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented per the approved plan. Deviations, all minor: host_guard uses middleware::from_fn (it needs no state) rather than from_fn_with_state; the /auth handler takes Query<AuthQuery> with an Option<String> field instead of Option<Query<_>>, which is not a valid axum 0.8 extractor; tokio gained the "sync" feature for the oneshot channel the shutdown test uses; the /api 404 message re-prefixes "/api" because nest strips the mount point.
- Verification: cargo fmt --check clean; cargo clippy --all-targets -D warnings clean; cargo test 330+25 pass; cargo test --no-default-features 300+26 pass; cargo test --no-default-features --features serve 323+25 pass; cargo build --release ok. clippy --no-default-features (with and without serve) reports exactly the two pre-existing needless_return lints at dashboard.rs:852 and report/mod.rs:160 (task-34) and nothing new.
- Manual smoke, debug binary on --port 0: no cookie -> 401 unauthorized JSON; Host: evil.com -> 403 forbidden; /auth?token= -> 302 with Set-Cookie nigel_session=...; HttpOnly; SameSite=Strict; Path=/; ping with cookie -> 200 {"ok":true,"version":"1.0.1"}; / -> 200 text/html placeholder; /reports/pnl -> 200 text/html (SPA fallback); /api/nope -> 404 application/json not_found; Origin: http://evil.com -> 403; Origin: http://localhost:5173 -> 200 (vite proxy works). SIGINT printed "Server stopped." and exited cleanly.
- Manual smoke, release binary run from a directory with no web/ present: / -> 200 html with the placeholder marker, /auth -> 302, /api/ping -> 200, SIGINT -> clean stop. Confirms the embed has no filesystem dependency.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds `nigel serve`: an axum HTTP server on 127.0.0.1 that hosts the JSON API and the embedded SPA from the same binary, behind a new default-on `serve` feature. Data endpoints are out of scope (31.4+); this lands the skeleton, the security model, static hosting, and graceful shutdown, with `GET /api/ping` as the only route.

## Changes

- **Cargo**: new default-on `serve` feature gating optional `axum` 0.8 (default-features off; http1/json/query/tokio), `tokio` 1.53, `tower` 0.5 (test-only oneshot; already transitive via axum), `rust-embed` 8.12 (debug-embed + mime-guess), `open` 5.4, `subtle` 2.6. `tower` is a regular optional dep rather than a dev-dep because dev-dependencies cannot be feature-gated and would otherwise compile into the `--no-default-features` run.
- **`src/server/`**: `mod.rs` owns the tokio runtime (the crate's only async entry point; `main` stays sync), router assembly, and graceful shutdown on SIGINT/SIGTERM; `auth.rs` holds the session token, Host/Origin validation, cookie parsing, and the `/auth` handler; `error.rs` defines `ApiError`/`ApiErrorCode` and the `{"error": {code, message, details?}}` envelope with the epic's full status mapping; `state.rs` holds `AppState`; `routes/mod.rs` mounts ping plus a JSON 404 fallback; `static_files.rs` serves `web/dist` with an index.html fallback for SPA routes.
- **Security**: bind loopback only; exact-match Host/Origin check against localhost, 127.0.0.1, and [::1] on any port (403, blocks DNS rebinding, still lets the vite dev proxy through); per-run 32-byte token compared in constant time, traded at `/auth?token=` for an HttpOnly, SameSite=Strict, Path=/ cookie required on every `/api` route (401). The session layer wraps the `/api` router with `.layer` rather than `.route_layer` so it also covers that router's 404 fallback. The token is never persisted and never appears in a response body. `colored::control::set_override(false)` at startup keeps ANSI codes out of HTTP bodies.
- **Dispatch**: `Commands::Serve { port, no_open }` (default port 5731, 0 = ephemeral). Serve joins the `needs_password` exemption list — it has no stdin to prompt on — and therefore runs migrations itself, but only when the database is unencrypted; an encrypted database stays locked until 31.4 unlocks it over HTTP. Built without the feature, the subcommand still parses and fails with a message naming the feature, matching the PDF gate.
- **Assets**: committed placeholder `web/dist/index.html` so `cargo build` works without node; `debug-embed` keeps debug and release builds identical and filesystem-free.

## User impact

New `nigel serve [--port N] [--no-open]`. Every existing command is untouched. `--no-default-features` now also drops the web server; `--features serve` re-enables it.

## Tests

23 new in-crate tests plus 3 CLI-surface tests. Unit coverage for the Host and Origin validators (including `127.0.0.1.evil.com`, `localhost.evil.com`, `[::]`, and unbracketed `::1`), constant-time token compare, cookie parsing, token generation, and the error envelope. Router tests drive the real app via `tower::ServiceExt::oneshot` for all six spec scenarios — 401 without a cookie, 403 on a bad Host (proving middleware order), auth-then-ping 200, index at `/`, SPA deep-link fallback, and JSON 404 on an unknown `/api` path — plus missing-Host, cross-origin, bad/absent token, and a graceful-shutdown test on an ephemeral port. `tests/cli_dispatch.rs` covers `serve --help`, the uninitialized-database error, and (gated to no-serve builds) the feature-gate error.

Green: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and all three feature combos of the test suite (330+25, 300+26, 323+25). `clippy --no-default-features` still reports the two pre-existing needless_return lints tracked as task-34 and nothing new. CI gains a third test step for the `--no-default-features --features serve` combo.

## Docs

New `docs/api.md` (security model, error-envelope code table, `/auth` and `/api/ping`). CLAUDE.md gains a Web server architecture bullet, the serve commands, the `src/server/` and `web/` structure, and four design constraints. README gains a Features bullet, Quick Start lines, a `serve` feature-flag row, and a Configuration note.

## Risks / follow-ups

Axum is built with `default-features = false`; task 31.7 will need to add the `multipart` feature for uploads. The `web/dist` placeholder will be overwritten by the vite build in 31.9, which also owns any ignore rules for built assets.
<!-- SECTION:FINAL_SUMMARY:END -->
