---
id: TASK-31.12
title: 'SPA: register browser with inline editing'
status: Done
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-07 13:55'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.5
  - TASK-31.6
  - TASK-31.9
references:
  - src/browser.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.12-register.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Web version of browser.rs, the richest screen in the app: paginated or virtualized transaction table backed by the register endpoint, account filter, date-range navigation with scroll-to-today on load, incremental text search, row selection, inline category and vendor editing, and flag toggling. Reuse boxcraft-app table and form patterns.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Full register table with account filter, date range, and incremental search
- [x] #2 Inline category and vendor edits and flag toggles persist via the API and update in place
- [x] #3 Initial load lands at the current date, matching the TUI scroll-to-today behavior
- [x] #4 Basic keyboard navigation works (arrows, enter to edit, escape to cancel)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Implemented per the approved plan and the coordinator's eight rulings. Two commits on nigel-31-spa: the three @nigel/ui components, then the api methods, the screen and the docs.

- Ruling 5 resolved by inspection before building on it: this Web Awesome build's `wa-select` has no search (no `withSearch` in its declaration, and there is no combobox component in the package), so the approved fallback applies. The category editor is a hand-built combobox — a native input carrying `role="combobox"`/`aria-activedescendant` plus a `role="listbox"`, because the ARIA wiring needs an input the component owns rather than one inside another component's shadow root. Vendor stayed on `wa-input`, whose label is hidden through `::part(form-control-label)` since the column header already names the field.

- Search filters rather than jumps (ruling 4), so the two "how many rows" numbers had to be kept apart deliberately: the footer Net is always `RegisterReport.total` for the whole result set, and the match count lives beside it as "N of M rows shown". A filtered total would be a different number wearing the same label.

- One deviation from the plan worth flagging, decided while wiring the route: account and period changes navigate, but the search box does not. Writing `?q=` on every keystroke would put one history entry per character between the user and the previous screen. `?q=` is still read on load, so a search is still a shareable link — it just is not a back-button trail. Documented in web/README.md.

- `wc-period-nav` landed as the shared component (ruling 3) with pure helpers — `stepPeriod`, `periodLabel`, `periodToParams`, `paramsToPeriod` — exported alongside it. `periodToParams` sends a month as `month=YYYY-MM` alone rather than with a redundant `year`, because the API reads `year` as the winner when both are present. Nothing in the component knows about the register, and `readonly` on the table (31.15's heads-up) has nothing to fight: the table's editing surface is already gated on `editingId`, so a readonly prop is an additive guard on activation and the flag button.

- `scroll_to_today` parity is exact, including the two edge cases: the *last* row dated today when several share it, and the top of the list when every transaction is in the future. `todayIso` builds the local date by hand rather than through `toISOString`, which is UTC and would open the register on the wrong row for most of the evening west of Greenwich. A dated register deliberately does not scroll to today, matching `cli/browse.rs`, which only scrolls when no date filter is in play.

- The TUI's vendor prompt starts empty and writes NULL on Enter, which silently wipes an existing vendor while the prompt calls it a skip. The web prefills instead, so leaving the field alone is a no-op and clearing it is an explicit clear. `buildPatch` sends only changed fields and returns null when nothing changed — verified live that an empty PATCH is a 400 and that `categoryId: null` is refused, which is exactly why neither is ever sent.

- Row cost is asserted, not just intended: a test fails if any `wa-*` element exists in the table's DOM while no row is being edited.

- Verification, all from the worktree. Web: `npm run lint`, `npm run typecheck`, `npm test` (670 tests: 79 theme + 362 ui + 229 app; 136 of them new — 83 ui, 53 app), `npm run build` — all clean.

- Cargo, no source touched (docs only): `cargo test` is green at 495 + 25 with `--test-threads=1` and an isolated HOME. Two failures appear otherwise and are NOT from this task: `cli::settings_manager::tests::{toggle_update_check,update_check_loads_from_settings}` read the real `~/.config/nigel/settings.json`, which currently holds `"update_check": false` and a `/tmp` data_dir left behind by an earlier test run. 31.10 added a `#[cfg(test)]` config-dir override and a `TempConfig` guard for exactly this hazard; these two tests do not use it. Both pass under a clean HOME. Worth a follow-up on the settings_manager tests, not on this branch. A further five encryption/unlock tests fail only when run in parallel, which is why the matrix is single-threaded.

- Manual smoke against a real server: `nigel init` + `nigel demo` in an isolated HOME, then `serve --port 5798 --no-open`. `/api/reports/register` answers the `{granularity, report:{rows,total}}` envelope with 270 rows and every field name matching the hand-written TS mirror (`categoryId`, `accountName`, `isFlagged`). Filters checked: `month=2025-03` 15 rows, `year=2025` 150, `account=BofA Checking` 270, `from`+`to` a valid empty result, `from` alone 400, `month=2025-13` 400. PATCH checked: category+vendor together answers the full row (which is what the screen swaps in), `vendor: null` clears, `flag: true` twice stays flagged once and `false` unflags, `{}` is 400 with "Nothing to update", `categoryId: null` is 400. The freshly built bundle is embedded and served — `/` returns the SPA and its asset contains `wc-register-table`. The server stopped cleanly on signal.

- Not done: a real-browser visual pass. No browser tooling is available in this session, so the rendered check is limited to jsdom (every component test plus axe over all 18 preview states) and to confirming the built bundle is what the binary serves. The preview harness at `npm run preview` is the place to eyeball the components in both themes.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds the transaction register to the web UI — the web counterpart of `browser.rs`, and the screen the rest of the SPA links into.

## Screen

`#/register` (`nigel-register-screen`) loads the register, the accounts and the chart of accounts in parallel, and gives them an account filter, a period pager, incremental search, row selection, inline category and vendor editing, and flag toggling.

Behaviour is pinned to the TUI's, with the parity rules kept as pure functions in `screens/register-data.ts` so both halves of the app can be held to them:

- **`rowMatches`** is `recompute_search_matches`, field for field: case-insensitive substring over description, vendor and category name, with a missing vendor or category treated as empty so it can never match. Date, amount and account are not searched. The query is not trimmed, because in the terminal a trailing space is part of what you typed.
- **`indexOfToday`** is `scroll_to_today`: the last row dated on or before today, the *last* of several rows sharing today's date, and the top of the list when everything is in the future. The date is built from local fields rather than `toISOString`, which is UTC and would open the register on the wrong row for most of the evening.
- **`buildPatch`** sends only what changed. An empty `PATCH` is a `400` by design and `categoryId: null` is refused outright, so neither is ever sent.

Search filters the table and reports "N of M rows shown" where the terminal keeps every row and jumps between matches — the divergence the spec asked for. The footer Net stays `RegisterReport.total` for the whole result set either way, so it never quietly becomes the total of a search.

Edits are optimistic and the row is replaced by the one the server answers with, not by the optimistic copy: the response is the authority on the row, including fields this screen never touched. A failure puts the original row back and toasts. Flags are sent as a state rather than a toggle, so a retry after a lost response cannot land the opposite of what was asked for.

Account and period changes navigate, so a filtered register is a link and the back button walks the filters. `?q=` seeds the search on load but is not written back per keystroke, which would put one history entry per character between the user and the previous screen. `#/register?id=185` opens on a transaction, which is how the review screen and the flagged report will link in.

## Components

Three new `@nigel/ui` components, each with a preview and an axe suite over every state:

- **`wc-register-table`** — `role="grid"` with a roving tabindex, so selection follows focus and a screen reader announces the same model the keyboard drives. Arrows, Home/End, PgUp/PgDn (a measured screenful, falling back to the TUI's page size), Enter to edit, Esc to cancel, `f` to flag, `/` to reach the search box. Rows stay cheap: one that is not being edited renders text, one `wc-money` and one icon button, and a test fails if any `wa-*` element appears in the table while nothing is being edited — an unfiltered register is thousands of rows.
- **`wc-register-toolbar`** — account select, period nav, and a search box whose row count is a live region.
- **`wc-period-nav`** — the granularity-driven pager, shared with the report screens (31.15). Driven by the `granularity` the server reports, with `allowAll` for the register's unfiltered default; `stepPeriod`/`periodLabel`/`periodToParams`/`paramsToPeriod` ship beside it as pure, tested helpers.

The category editor is a hand-built combobox rather than `wa-select`: this Web Awesome build has no searchable select and no combobox component, and the `aria-activedescendant` wiring needs an input the component owns.

## API

Four methods on `ApiClient`, `FetchApiClient` and `FakeApiClient`: `getRegister`, `getAccounts`, `getCategories`, `patchTransaction`, with `RegisterRow`, `RegisterReport`, `Account`, `CategoryRow` and `TransactionPatch` mirrored by hand in `types.ts`. The fake really applies a patch to its fixture and answers with the updated row, because the screen swaps the response in — a fake that echoed the request back would never catch that.

`docs/api.md`'s route table showed report responses without their `{granularity, report}` wrapper, which is what the routes actually return; the table now names the envelope.

## Tests

136 new tests — 83 across the three components (keyboard navigation at the boundaries, edit commit and cancel payloads, flag state both ways, scroll-to-row, the no-`wa-*` row contract, period arithmetic including year rollover) and 53 on the screen and its helpers (the search predicate as a table of promises, scroll-to-today across four fixture shapes, PATCH payloads including the no-op commit that must send nothing, rollback with a toast, deep links, and refetching only when the request itself changed).

Web: lint, typecheck, 670 tests, build — all clean. No Rust source touched; `cargo test` is green at 495 + 25 single-threaded under a clean HOME.

## Follow-ups

- `cli::settings_manager::tests::{toggle_update_check,update_check_loads_from_settings}` read the real `~/.config/nigel/settings.json` and fail on a machine whose settings have `update_check: false`. 31.10 added a `TempConfig` guard for this hazard; these two tests do not use it. Pre-existing, unrelated to this change.
- No virtualization, per the spec. A register of tens of thousands of rows will render slowly; that is a follow-up if it turns out to matter in practice.
<!-- SECTION:FINAL_SUMMARY:END -->
