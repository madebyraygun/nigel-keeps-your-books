# Epic 31 — Web UI (localhost): Architecture (task-31)

**Goal:** `nigel serve` runs an axum HTTP server on 127.0.0.1 serving a JSON API
over the existing data layer plus an embedded Lit SPA, from the same single
binary. The TUI stays the default; serve is additive. This spec fixes the
decisions shared by all subtasks (31.1–31.17); per-task specs reference it.

## Crate layout (backend)

- `src/lib.rs` exposes the existing modules as a library (task 31.1); `main.rs`
  keeps clap dispatch, the ratatui panic hook, and all interactive prompting.
- The server lives in `src/server/`: `mod.rs` (router assembly + startup),
  `auth.rs` (session token, Host/Origin validation), `error.rs` (`ApiError` →
  HTTP mapping), `state.rs` (`AppState`), `routes/` (one file per domain:
  status, reports, lists, transactions, review, accounts, categories, rules,
  imports, exports, reconcile, settings).
- New dependencies are optional, behind a **default-on `serve` feature**
  (matching the pdf/gusto pattern): `axum`, `tokio` (rt-multi-thread, signal),
  `tower-http` (only if needed for limits), `rust-embed`, `open`. Built without
  the feature, `nigel serve` exits with a clear error. `cargo test
  --no-default-features` must stay green throughout the epic.
- rusqlite is sync and `Connection` is per-request: every handler runs its data
  work inside `tokio::task::spawn_blocking`, opening a fresh connection via
  `db::get_connection` (WAL + busy_timeout already handle concurrency). No pool.
- `AppState`: `db_path: PathBuf`, `session_token: String`, feature flags. The
  DB password stays where it is today — the process-global
  `db::set_db_password` mutex — set by the unlock endpoint (31.4).
- Serve-mode startup calls `colored::control::set_override(false)` so shared
  text formatters never emit ANSI codes into HTTP responses.

## HTTP conventions

- All endpoints under `/api`. JSON bodies both ways.
- **Casing:** every API-visible struct gets `#[serde(rename_all = "camelCase")]`
  (task 31.2). This is the single documented choice.
- **Error envelope:** `{"error": {"code": "<snake_case>", "message": "<human>"}}`
  with optional `"details"`. Codes and status mapping:
  - `bad_request` 400 (bad params; lone `from`/`to` — the pair rule is enforced
    exactly as in the CLI)
  - `unauthorized` 401 (missing/invalid session)
  - `forbidden` 403 (Host/Origin check failed)
  - `not_found` 404 (unknown account/category/rule/transaction/import)
  - `conflict` 409 (guardrail blocks: delete with transactions, duplicate name —
    structured `details` carries a reason code, e.g. `has_transactions`, plus
    counts, alongside the existing human message)
  - `locked` 423 (encrypted DB not yet unlocked; body also distinguishable from
    401 so the SPA routes to the unlock screen)
  - `internal` 500
  - `feature_disabled` 501 (e.g. PDF export in a no-pdf build)
- Date params mirror CLI semantics: `year` (int), `month` (`YYYY-MM`), `from` /
  `to` (`YYYY-MM-DD`, must be a pair), `account` (name). Invalid `month` strings
  are a 400, not the CLI's silent `(None, None)`.
- Endpoint inventory lives in `docs/api.md`; every API task updates it in the
  same commit.

## Security model (31.3)

Localhost is not a trust boundary. Defense layers, in middleware order:

1. **Bind** 127.0.0.1 only. `--port` (default 5731), `--port 0` for ephemeral,
   `--no-open` to suppress browser launch.
2. **Host/Origin validation:** the request Host — and Origin when present —
   must have host `localhost`, `127.0.0.1`, or `[::1]` (any port, so the vite
   dev proxy works). Anything else is 403. This blocks DNS rebinding.
3. **Session token:** 32 random bytes (hex) generated per server start. The
   printed/opened URL is `http://127.0.0.1:<port>/auth?token=<t>`; `/auth`
   validates the token (constant-time compare), sets a `nigel_session` cookie
   (HttpOnly, SameSite=Strict, Path=/), and redirects to `/`. Every `/api`
   route except the auth redirect requires the cookie; failures are 401. Static
   assets are served without the cookie (they contain no data).
4. The token and password are never logged.

## Locked-state flow (31.4)

For an encrypted DB the server starts **locked**: `GET /api/status` and
`POST /api/unlock` are the only working endpoints; everything else returns 423.
Unlock validates via `db::validate_password` and stores via `set_db_password`.
Failed attempts get attempts-remaining feedback with exponential backoff
(mirroring the TUI's 3-attempt posture). Unencrypted DBs skip the flow —
`status` reports `locked: false` from the start.

## SPA architecture (31.9–31.17)

The frontend borrows **structure and code** from boxcraft-studio
(`~/Dev/boxcraft/main`) — component library first, token library, Web Awesome
primitives before custom work.

### Workspace layout

`web/` is an npm workspace (no turbo; three packages, plain root scripts that
build in order):

| Package | Purpose (mirrors boxcraft) |
|---|---|
| `@nigel/theme` (`web/packages/theme`) | Design tokens as Lit `css` composed into one `CSSResult` **and** a generated plain `.css`. Token names shadow Web Awesome's `--wa-*` namespace (`--wa-color-bg/surface/text/brand/...`, `--wa-font-*`, `--wa-space-*`, `--wa-radius-*`) plus nigel-specific `--nc-*` tokens. Dark mode via `prefers-color-scheme` + `.dark-mode`/`.light-mode` class overrides. Brand identity derives from nigel's existing pastel-gradient look (`src/effects.rs` palette) with a mono font for money columns. `::part()` overrides for `wa-*` primitives live in the document-level sheet. |
| `@nigel/ui` (`web/packages/ui`) | All visual primitives as `wc-*` Lit components, plus the preview harness (`import.meta.glob` manifest, port 9090) and per-state axe tests. Depends on `lit` only — no signals, no API types. |
| `@nigel/app` (`web/apps/app`) | Composition only: screens, stores, services, api client. Depends on `@nigel/ui`, `@nigel/theme`, `@awesome.me/webawesome`, `@lit-labs/signals`. |

Pinned stack: lit ^3.2, @awesome.me/webawesome ^3.1, @lit-labs/signals ^0.2,
vite ^6, vitest ^2, typescript ^5.7 (with `experimentalDecorators` +
`useDefineForClassFields: false`).

### Component-First workflow (MANDATORY — carried over from boxcraft CLAUDE.md)

Every visual change ships through `@nigel/ui`:

1. Component lives in `web/packages/ui/src/components/` as `wc-foo.ts`.
2. Co-located `wc-foo.preview.ts` covers the visible states (default, hover,
   disabled, loading, empty, dense — whichever apply).
3. `wc-foo.test.ts` runs `axe.run()` against each preview state with zero
   violations.
4. Only then is it consumed: `apps/app` imports from `@nigel/ui`. **No bespoke
   component implementations in `web/apps/app/src/components/`.**

Component selection: use Web Awesome `<wa-*>` primitives unless behavior
demands custom. A `wc-*` wrapper reads `@nigel/theme` tokens and exposes them
as cascading variables — never duplicates brand values inline. Pure
logic/state/service work is exempt.

### Conventions carried over verbatim from boxcraft

- `wc-` tag prefix; canonical file skeleton (types → `@customElement` →
  `static styles` → `@property` → handlers → `render()` →
  `HTMLElementTagNameMap` augmentation); `reflect: true` only where CSS keys
  off it; variants as `:host([variant])` attribute CSS.
- Cherry-picked Web Awesome side-effect imports
  (`@awesome.me/webawesome/dist/components/<x>/<x>.js`) — no autoloader, no WA
  stylesheet (theming rides the `--wa-*` custom properties).
- The `dispatchWcToast` typed-event bus terminating in the app shell's single
  `aria-live` region (popover top-layer trick included).
- Icon system: `WcIconBase` abstract class, `--wc-icon-size`-style sizing
  (nigel: `--nc-icon-size`).
- State: module-scope writable signals exposed through a `ReadonlySignal`
  interface with action methods (boxcraft `project-store` pattern);
  per-feature free-floating signals for async flags. Components mix in
  `SignalWatcher(LitElement)` in `apps/app` only.
- Vitest: node environment by default, jsdom by glob for component tests;
  `test-setup.ts` shims (`attachInternals`/ElementInternals, dialog methods) —
  required for Web Awesome under jsdom.
- Guard tests: a dependency-manifest test (every `@nigel/*` specifier declared
  in package.json) and an api-seam test (no `fetch(` outside the api client
  module).
- `"development"` export condition + vite alias so `apps/app` compiles
  `@nigel/ui` source directly (HMR without a watch build).

### Deliberate departures from boxcraft

- **Routing:** boxcraft has no router and syncs screen identity in three
  places. Nigel uses a single screen registry
  (`Map<ScreenId, { title, icon, render }>`) driving both the sidebar and the
  content area, with `ScreenId` reflected to `location.hash` (deep-linkable,
  back-button works).
- **No `window.confirm`:** confirmations use a `wa-dialog`-based `wc-confirm`.
- Boxcraft's stale `packages/ui/src/tokens/index.ts` duplicate-palette mistake
  is not ported: tokens live in `@nigel/theme` only.

### API client seam (the Tauri/multiuser hinge)

`web/apps/app/src/api/` is the **only** module that talks to the server:

- `types.ts` — hand-written TS mirrors of the serde structs (camelCase),
  kept in sync via `docs/api.md`.
- `client.ts` — `interface ApiClient` with one typed method per endpoint;
  `FetchApiClient` implements it (base URL, credentials, error-envelope
  parsing). A later Tauri `InvokeApiClient` or remote-server client implements
  the same interface without touching components.
- Transport-level cross-cutting state lives here as signals: `appLocked`
  (423 seen → route to unlock), `appUnauthorized` (401 → show reauth message),
  plus per-call error normalization to a typed `ApiError`.

### Embedding & workflows

- `vite build` outputs to `web/dist/`; `rust-embed` embeds `web/dist` into the
  binary with an index.html fallback for SPA routes. A committed placeholder
  `web/dist/index.html` ("SPA not built — run npm run build in web/") keeps
  `cargo build` working without node.
- Dev loop: `nigel serve --no-open` on 5731 + `vite` on 5173 proxying `/api`
  and `/auth` with `changeOrigin: true`; open the printed `/auth?token=` URL on
  the vite origin. Documented in CLAUDE.md + web/README.md.
- CI: npm ci + build `web/` before `cargo build`/`cargo test`; `npm test` and
  `npm run lint` join the pipeline (31.9).

## Delivery model

Work lands as one commit series per subtask on the epic branch `nigel-31`,
merged to main as a single epic PR when acceptance criteria for task-31 hold.
Every subtask keeps the full workspace green: `cargo test`, `cargo test
--no-default-features`, `cargo clippy`, `cargo fmt --check`, and (once web/
exists) `npm test` + `npm run lint`. Documentation policy applies per subtask:
CLAUDE.md / README.md / docs/api.md updates ship with the change, not after.

## Cross-task interface contracts

Ownership of shared seams, to prevent collisions:

- 31.1 moves `DateGranularity` out of `cli/report/view.rs` into `reports.rs`
  (pub, with a `granularity(report) -> DateGranularity` mapping) — later tasks
  must not re-declare it.
- 31.2 owns all serde derives; API tasks add derives only for structs they
  introduce.
- 31.5 adds `list_imports(&Connection)` (import history query — today only
  `get_last_import` exists) and extracts `list_rules(&Connection) ->
  Vec<RuleRow>` with a **public** `RuleRow` (the current one is private in
  `rules_manager.rs`).
- 31.6 extracts the rest of the rules data layer (`add/update/delete/test` as
  `&Connection` functions returning data — `rules::test` currently prints and
  discards its result set) and converts `categories::blocking_reason` /
  accounts' inline guard into structured reason codes (keeping the human
  message).
- 31.8 owns nothing new in the data layer: PDF renderers (`pdf.rs`) already
  return `Vec<u8>`; text formatters are reused with color disabled.
- The SPA screen tasks (31.10–31.17) never add endpoints; if a screen needs
  something the API lacks, the fix goes into the owning API task first.
