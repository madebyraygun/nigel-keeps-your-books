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
