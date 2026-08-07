# nigel web UI

The single-page app `nigel serve` embeds and serves. An npm workspace with
three packages, built in order by the root `build` script.

| Package | Path | Purpose |
|---|---|---|
| `@nigel/theme` | `packages/theme` | Design tokens as Lit `css` modules composed into one `CSSResult`, plus a generated plain stylesheet. |
| `@nigel/ui` | `packages/ui` | `wc-*` Lit components and the preview harness. Depends on `lit` only — no signals, no API types. |
| `@nigel/app` | `apps/app` | Composition: screens, store, api client. Depends on the other two. |

Stack: lit 3, Web Awesome 3, `@lit-labs/signals` 0.2, vite 6, vitest 2,
TypeScript 5.7. Node 20.19+ (22 recommended). No turbo — the root scripts
chain the three builds explicitly.

## Commands

Run from this directory.

```bash
npm ci                 # install (uses the committed lockfile)
npm run dev            # vite dev server on :5173
npm run preview        # component preview harness on :9090
npm test               # vitest across all three packages
npm run lint           # eslint across all three packages
npm run typecheck      # tsc --noEmit across all three packages
npm run build          # theme -> ui -> app, output to web/dist
```

## Dev loop

The dev server proxies to a running backend, so both halves stay live.

```bash
# terminal 1 — the backend
cargo run -- serve --no-open
#   prints: http://127.0.0.1:5731/auth?token=<hex>

# terminal 2 — the frontend
cd web && npm run dev
#   vite on http://localhost:5173
```

Then open the token URL **on the vite origin**, not the backend's:

```
http://localhost:5173/auth?token=<hex>
```

Why that works: vite proxies `/auth` and `/api` to `127.0.0.1:5731`. The
backend's Host check accepts `localhost` on any port, which is what makes a
dev proxy possible at all. The `Set-Cookie` for `nigel_session` carries no
`Domain`, so the browser stores it host-only for `localhost:5173` — the origin
you are actually on — and the `302` to `/` is relative, so you stay there.
Every later `/api` call is same-origin from the browser's point of view, which
is why the client sends `credentials: 'same-origin'`.

Restarting `nigel serve` mints a new token and invalidates the cookie. The next
API call returns 401, `appUnauthorized` flips, and the shell shows a banner
telling you to reopen the newly printed URL — the page cannot mint a token
itself.

## Component-first workflow (mandatory)

Every visual change ships through `@nigel/ui`:

1. The component lives in `packages/ui/src/components/` as `wc-foo.ts`.
2. A co-located `wc-foo.preview.ts` covers the visible states.
3. `wc-foo.test.ts` calls `describePreviewA11y(preview)`, which runs axe over
   every state the preview declares — add a state and its a11y test comes with
   it.
4. Only then does `apps/app` consume it. There are no bespoke component
   implementations under `apps/app/src/components/` other than the root
   container.

Use Web Awesome `<wa-*>` primitives unless behavior demands custom. Wrappers
read theme tokens and expose them as cascading variables; they never inline a
brand value. Pure logic, state and service work is exempt.

## The api seam

`apps/app/src/api/` is the only module that talks to the server. This is the
hinge the Tauri and multiuser plans swing on, so it has rules:

- `types.ts` mirrors the serde structs by hand, camelCase, one interface per
  response struct named exactly as the Rust one. Keep it in step with
  `docs/api.md` in the same commit as the endpoint.
- `ApiClient` gets **one method per endpoint**, named after it. Routes with
  more than one parameter take a single typed options object, so adding a
  parameter stays non-breaking.
- There is no generic `request()` on the interface, and there must not be. It
  would let screens build URLs, which a Tauri or remote client cannot honour.
- Cross-cutting transport state (`appLocked`, `appUnauthorized`) lives here,
  not in components.

A guard test fails the build if `fetch(`, `XMLHttpRequest`, `EventSource`,
`WebSocket` or `navigator.sendBeacon` appears anywhere under `src` outside
`src/api` — tests included. Use `src/__mocks__/fake-api-client.ts` instead.

One transport wrinkle lives behind that seam. `uploadImport(file)` sends a
`FormData` body, and `request()` omits its usual JSON content type when it sees
one: `multipart/form-data` needs a boundary parameter that only the browser can
generate, and naming the header by hand produces a body the server cannot
parse. The method takes a `File` and no progress callback — `fetch` cannot
report upload progress, and putting a callback on the interface would oblige a
Tauri or remote client to invent progress it does not have. A client that *can*
measure it (an XHR one, which is why `src/api` is exempt from the guard above)
adds it as an optional options object without disturbing this one.

## Screens

`apps/app/src/screens/registry.ts` is the single description of a screen:
title, nav label, icon, whether it appears in the sidebar, and how it renders.
The sidebar, the header and the content area all read it, and it is typed as
`Record<ScreenId, ScreenDef>` so a missing entry is a compile error.

Routing is `location.hash`, format `#/<screen>?<params>`. Navigation writes the
hash and nothing else; the `hashchange` listener is the only thing that updates
route state. Unknown hashes fall back to the dashboard.

A screen that only draws markup is a `render(ctx)` function. A screen that holds
state is a custom element declared in the same file, with the render function
reduced to one tag — `nigel-settings-screen` is the worked example. Screen
elements live in `screens/`; `apps/app/src/components/` stays the root container
alone, and every visual primitive still comes from `@nigel/ui`.

`render` is handed a `ScreenContext` (`screens/context.ts`): the api client, the
route's query parameters, and `navigate`. Screens take their client from there
rather than importing a singleton, which is what lets a test drive a whole
screen with `FakeApiClient` and what will let a Tauri client take the same
place. Keep it small — anything added to it is added for every screen at once.

## Importing

`#/import` is one screen with three panels that appear as the decision is made:
choose a file, preview what it would do, confirm. Nothing is written until the
confirm, and the server takes a snapshot before it writes.

**The upload is lazy.** Choosing a file sends nothing. Preview uploads and then
dry-runs, in one action, for two reasons: a file picked and thought better of
never reaches the server's spool, and an upload before an account is chosen is
an upload that cannot be used for anything. The `uploadId` is then cached
against that file, so fixing a column mapping and previewing again costs one
request rather than re-sending the bytes.

**An expired upload re-uploads itself, once.** The spool clears after an hour,
which a preview left open over lunch will find. The file is still in the
browser, so the honest response is to send it again rather than to make someone
re-choose it; the retry is capped at one, after which the dropzone says the
upload expired.

**A duplicate file blocks the confirm.** The server would answer `200` with
`duplicateFile: true` and zero counts — the checksum is checked before anything
is parsed — so a confirm button there would offer a no-op. The screen shows the
warning and offers no button.

**Format and mapping cannot both be sent.** The API refuses the pair with a
`400`. `importRequestBody` in `screens/import-data.ts` derives both from the
single format field, so there is no branch that sets them together; likewise
`saveProfile` is dropped unless there is a mapping to save under it, which is
the other pairing the API refuses.

Failures go where their cause is, not into a toast that vanishes while the form
is still on screen — `routeImportError` is the whole table. `423` and `401` go
nowhere at all: the shell gates both before this screen is constructed, so
handling them here would be a second, worse telling of a story already on
screen.

## The register

`#/register` is the biggest screen, and the one whose behaviour is pinned to the
TUI's (`src/browser.rs`).

**Search is client-side.** `/api/reports/register` has no search parameter and
gains none. `rowMatches` in `screens/register-data.ts` is the TUI's
`recompute_search_matches`, field for field: case-insensitive substring over
description, vendor and category name, with a missing vendor or category
treated as empty so it can never match. Date, amount and account are not
searched. One deliberate difference: the web filters the table down to matching
rows and reports "N of M", where the terminal keeps every row and jumps between
matches. The footer total stays the whole result set's `RegisterReport.total`
either way, so it never silently becomes the total of a search.

**Where it opens.** With no date filter the register lands on the last row dated
on or before today — `scroll_to_today`, relying on the date-ascending order the
endpoint returns. A dated register stays at the top, because a March register
has no today to find. `#/register?id=185` beats both, which is how the dashboard
and the review screen link to a transaction.

**Filters are the route.** Account and period changes call `navigate`, so a
filtered register is a link and the back button walks the filters. The search
box is the exception: it is read from `?q=` on load but not written back per
keystroke, which would put one history entry between the user and the previous
screen for every character typed.

**Edits are optimistic with rollback.** `buildPatch` sends only what changed —
an empty `PATCH` is a `400` by design — and the row is replaced by the one the
server answers with rather than by the optimistic copy. A failure puts the
original row back and toasts. Flags are sent as a state, never a toggle, so a
retry cannot land the opposite of what was asked for.

`wc-register-table` keeps rows cheap: a row that is not being edited renders
text, one `wc-money` and one icon button, and no `wa-*` component at all. A test
asserts that, because an unfiltered register is thousands of rows and every
custom element in a row is paid for thousands of times. `wc-period-nav` is
shared with the report screens — driven by the `granularity` the server reports,
with `allowAll` the flag that lets the register offer an unfiltered view reports
do not have.

## The review flow

`#/review` steps through flagged transactions one at a time, the web
counterpart of `src/reviewer.rs`. `#/review?id=185` re-reviews a single
transaction, which is `nigel review --id`; the router has no path segments, so
the id is a query parameter like every other deep link.

**Back undoes, it does not merely re-show.** Every applied decision is pushed
onto a stack of `{ transactionId, ruleId }`, and Back pops one and calls
`POST /api/review/:id/undo` with that `ruleId`, which re-flags the transaction
and deletes the rule outright. A skip pushes `null` instead — the same
`Option<ReviewDecision>` the TUI stacks — so stepping back over a skipped
transaction issues no request at all, rather than clearing a category some
earlier session set. An undo that fails puts its decision back on the stack, so
the stack can never claim a decision the server still holds. The summary counts
are derived from the stack rather than tallied, which is why a Back corrects
them for free.

**Two departures from the TUI, both forced by the browser.** Tab does not skip:
on the web it is the key that moves between the form's controls, and rebinding
it would strand anyone not using a mouse — Skip is a button, and the keys the
screen really binds are Enter to apply and Esc to go back. And there is no
match-type choice, because `reviewer::apply_review` writes `contains` and the
apply route carries no field for anything else; the form says so in words
rather than offering a control the server could not honour.

**A missing transaction is not a wedged queue.** Another tab, or an undone
import, can take a transaction out from under an open review. A `404` on apply
advances with a toast and records a skip, so a later Back does not try to undo
a decision that was never made.

The rule pattern is prefilled with the first two words of the description,
which is the TUI's default — a bank line ends in a transaction id that will
never repeat, so the whole description would match nothing ever again. Typing
in it drives a debounced `POST /api/rules/test` whose answer lands in
`wc-rule-test-preview`; a rejected pattern renders inline and still leaves the
decision applicable, because a bad preview is not a bad decision.

`wc-category-picker` is the searchable chart-of-accounts combobox, sharing
`categoryLabel` with the register's inline editor. It is hand-built because
this Web Awesome build ships no searchable select, and the ARIA wiring
(`aria-activedescendant` into the option list) needs a real input underneath.
Options are grouped income then expense, the order `/api/categories` returns.

## Reports

`#/reports` is a directory of the eight; `#/reports?report=pnl&year=2025` is one
of them. The report is a query parameter rather than a path segment because the
router has none — the same reason `#/review?id=185` looks the way it does — and
every parameter that reaches the API is in the hash, so a report view is a URL
you can paste to someone.

One screen serves all eight. The frame never changes (period control, export
links, a body) and only the body differs, so `screens/reports-data.ts` holds the
catalog — one `ReportDef` per slug, describing its title, its icon and the date
parameters it accepts — and the landing page and the detail view both read it.
`REPORTS[slug].supports` is not decoration: the server answers `400` for a
parameter its route does not take, so sending `year` to `/api/reports/balance`
is an error rather than a no-op, and dropping the unsupported ones is what lets
one screen speak eight vocabularies.

**The period control is driven by the server.** `wc-period-nav` takes its
granularity from the `granularity` field on the report envelope, not from a
table in the client: `monthAndYear` gets paging and the month/year toggle,
`yearOnly` gets year paging alone, and `none` renders nothing at all. It runs
with `allowAll` off — "all transactions" belongs to the register browser, and a
report is always a report of some period. With no period in the route the screen
opens on the current year, which is what the TUI's own date navigation is seeded
with (`TableReportView::new` takes today, and every builder passes
`year.unwrap_or(current_year)`).

Period and account changes navigate; they never mutate state directly. The route
is the only thing that triggers a load, so the back button walks periods.

**The bodies** are composed from `@nigel/ui`, out of pure mappers:

| Report | What renders it |
|---|---|
| pnl, expenses, tax, cashflow, flagged | `wc-report-table` from a mapper |
| cashflow | plus `wc-bar-chart`, reusing the dashboard's `cashflowBuckets` |
| balance | `wc-balance-list` and a `wc-stat-card` for YTD net income |
| register | `wc-register-table` in `readonly` mode, with an account filter |
| k1 | `wc-panel`s, `wc-report-table`s and `wc-notice-bar`s, composed |

The K-1 worksheet is composed rather than given a component of its own. Every
block of it is already a primitive, nothing about the stacking is reusable
anywhere else, and a `wc-k1-worksheet` would have to take `K1PrepReport` as a
property — which would drag API types into `@nigel/ui`, the one thing the
package boundary forbids. It renders in `format_k1`'s order, including the
**auto-mapped note** (income with no form line falls back to gross receipts) and
the **needs-mapping section** (expenses with no form line, excluded from every
total above and listed so they can be mapped).

The register view is read only, and says so with a link to `#/register`. Editing
in two places would mean two places to keep honest about the same row.

### Exports

Both formats are plain `<a download>` links, from `wc-export-links`. The browser
streams the file, the session cookie rides along on a same-origin navigation,
and `Content-Disposition` names it — the same filename the CLI would write.

The href comes from `client.exportUrl(slug, format, params)`, never from a
string in the screen. A download link is as much a hardcoded address as a
`fetch` is, and a Tauri or remote client has no `/api` to serve it, so the api
seam owns it like every other endpoint. The guard test enforces this: a quoted
`/api/` literal anywhere under `src` outside `src/api` fails the build. (Naming
an endpoint in a doc comment is fine — the rule skips comment lines, because
documenting the seam is not routing around it.)

**PDF is offered only when the server can produce it.** PDF export is a
compile-time cargo feature, and `/api/status` reports it as `pdfExport`. A
download link cannot inspect what comes back, so without that flag a no-pdf
build would save a `501` error envelope as `pnl.pdf` — a failure dressed as a
success. When it is off, the PDF control is disabled with the reason in text
beside it and text export carries on. The status call already happens at boot,
so the check costs nothing.

Known edge, stated rather than hidden: a rare export failure that survives all
of the above (an account deleted in another tab, a server bug) is saved by the
browser as a small file containing the error envelope, because a link cannot
read a status code.

### Figure parity with the CLI

The reports have to show the numbers `nigel report` prints, and
`screens/reports-parity.test.ts` proves it against the CLI's own bytes: for each
report it renders the screen from a captured API response and compares every
money figure on the page with every money figure in the captured text export.
The comparison is on absolute values, because `wc-money` always renders the sign
while the text report prints magnitudes and lets colour carry direction — a
deliberate difference (colour cannot be the only cue), not a difference in the
figures.

Both sides come from one seeded database. Regenerate them after changing a
report's shape:

```bash
cargo test --features serve capture_web_report_fixtures -- --ignored --nocapture
```

That is an ignored test in `src/server/fixture_capture.rs` rather than a script
because the seeded database, the router and a session already exist as test-only
code — and because a script driving `nigel serve` would have to run `nigel init
--data-dir`, which rewrites the developer's real `~/.config/nigel/settings.json`
and repoints their books. It writes `web/apps/app/src/__fixtures__/reports/`:
a `.json` (what the browser would receive), a `.txt` (what the CLI would export)
and a `manifest.json` per report, plus a `needs-mapping-k1` pair from a second
database carrying an unmapped category, so the K-1's mapping states have real
captured data behind them.

### Printing

A printed report is the artifact someone keeps, so `@media print` in
`@nigel/theme`'s `print.ts` gives the page over to the report: shell chrome
hidden, black on white, 1.5cm margins, table headings repeating across page
breaks, and rows and panels kept from splitting.

The recolouring works by redefining the tokens at `:root`, not by restyling
components — custom properties inherit through shadow boundaries, which is the
only thing that reaches inside every `wc-*` element at once. Hiding the shell
needs the other route through the boundary, which is why `wc-app-shell` exposes
`sidebar`, `header`, `banner` and `content` as parts.

`packages/theme/__tests__/print.test.ts` asserts the rules that carry the
behaviour, and the build test proves they reach `dist/css/nigel.css`. What a
printer actually does still needs eyes, so before changing this sheet, run
through:

- [ ] `npm run dev`, open each of the eight reports, print preview
- [ ] No sidebar, header, banner, period control or export buttons
- [ ] Black on white in both light and dark mode
- [ ] A multi-page register repeats its column headings
- [ ] Nothing clipped at the right edge, at A4 and at Letter

## The managers

`#/accounts`, `#/categories` and `#/rules` are one screen three times over: a
list, an Add button, per-row Edit and Delete. They share `wc-manager-layout`,
`wc-manager-table` and `wc-manager-dialog`, and differ only in their columns and
their form. Together they are a superset of the TUI: `rules_manager.rs` can list
and delete rules, and nothing outside `nigel rules add` can write one.

**Editing happens in a dialog, not an inline panel.** The rule form is tall —
pattern, match type, category, vendor, priority, and a live preview of what the
pattern matches — and inline it would push the list it is about off the screen.
Delete is already a dialog, so this is one overlay idiom per screen rather than
two, and `wa-dialog` brings the focus trap and the Esc handling with it.

**A guardrail is rendered from its reason code, never from the server's
sentence.** `screens/manager-errors.ts` is the whole table: `has_transactions`,
`has_active_rules`, `duplicate_name` and `already_inactive` each map to a string
written here, with the count formatted here. `docs/api.md` puts it plainly —
"a client can explain the block in its own words instead of parsing ours" — and
those strings are then the only thing a translation would have to touch. Two
deliberate exceptions: a `400` renders the server's message, because
`Invalid regex: unclosed group` names the offending value and anything we
re-derived would drift from the server's actual rules; and an unrecognized 409
reason falls back to the message rather than to an invented sentence.

**Where the message lands depends on what was refused.** A failed save renders
inside the dialog, beside the field that caused it. A failed *delete* renders in
the layout's own alert region, because `confirmDialog()` resolves and removes
itself before the request is even sent — there is no dialog left to render into,
and a toast would take the count away before it had been read.

**Every mutation refetches the list.** No optimistic splicing: a priority edit
reorders the rules, a rename or a type change reorders the categories, and a
category rename changes the name shown on every rule row. The lists are
unpaginated single-table queries against a local server, so the refetch buys
three ways of being quietly wrong for one round trip.

Per screen:

- **Accounts.** No transaction-count column — `GET /api/accounts` does not carry
  one, and a screen may not add an endpoint; the number appears in the blocked
  delete instead, where it is actionable. Rename is the only edit, because that
  is all `PATCH /api/accounts/:id` accepts. The four-digit rule on Last four is
  enforced here and only here: `account_manager.rs` has it, the route does not.
- **Categories.** All four fields, income/expense as radios, and the K-1 form
  line documented next to the field with a datalist built at runtime from the
  form lines the chart of accounts already uses. A value the worksheet will not
  recognize gets a warning, never a block — `form_line` is free text everywhere
  else, and `resolve_k1_mapping` has defined behaviour for a value it does not
  know. An edit sends only what changed, since an all-omitted `PATCH` is a 400.
- **Rules.** Priority order is the semantics, so the list is never re-sorted.
  The pattern box drives `POST /api/rules/test` through the same 250 ms debounce
  the review screen uses, into the same `wc-rule-test-preview`; a match-type
  change fires immediately, because a click is a decision rather than typing.
  There is no client-side regex validation: JavaScript's `RegExp` and the Rust
  `regex` crate accept different languages, so a local check would be wrong in
  both directions. `#/rules?categoryId=12` filters the list client-side, which is
  where the categories screen's "3 active rules assign this category" points.

## Boot sequence

`nigel-app` fetches `/api/status` and nothing else until it knows where it
stands. `AppStore.boot` derives one of four phases from that answer:

| Phase | What renders |
|---|---|
| `starting` | A spinner. Status has not answered yet. |
| `locked` | The unlock gate, and nothing else — no shell, no sidebar. |
| `failed` | The retry banner. |
| `ready` | The app. |

The `locked` phase replaces the shell rather than disabling it, and that is the
point: with no shell there is no screen element, so nothing exists that *could*
fetch data before the password arrives. The phase is derived rather than stored,
so a `423` from any later call sends the app back to the gate on its own.

The unlock backoff is served by the server, which holds the response back before
answering — so the gate counts down *during* the request rather than locking the
form afterwards. A client-side cooldown on top would charge the same penalty
twice.

Switching data directory goes through `AppStore.switchDataDir`, which reloads
the page on success. The reload is injectable
(`initializeAppStore(client, { reload })`) because jsdom cannot implement
`location.reload`, and because "did it reload?" is exactly what a test needs to
assert.

## How the build reaches the binary

`npm run build` writes to `web/dist`, which `rust-embed` bakes into the binary.

`web/dist` is **generated and gitignored**. The committed fallback is
`web/placeholder/index.html`, and `build.rs` copies it into `web/dist` when no
`index.html` is there, so `cargo build` works on a machine without node — it
just serves a "SPA not built" page.

`build.rs` also emits `cargo:rerun-if-changed` for `web/dist`. This is load
bearing: `rust-embed`'s `debug-embed` decides *when* assets are baked in, not
when cargo reconsiders them, and its proc macro cannot emit the key itself.
Without the build script a fresh `npm run build` followed by `cargo run` would
serve the previously embedded bytes.

One caveat: `rerun-if-changed` on a directory tracks that directory's own
mtime, which covers files appearing and disappearing but not edits nested under
`assets/`. Vite rewrites `index.html` on every build, so that file is tracked
by name as well.
