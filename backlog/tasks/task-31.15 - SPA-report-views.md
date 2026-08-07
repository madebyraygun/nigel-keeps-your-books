---
id: TASK-31.15
title: 'SPA: report views'
status: Done
assignee: []
created_date: '2026-08-06 16:27'
updated_date: '2026-08-07 15:43'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.5
  - TASK-31.8
  - TASK-31.9
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.15-reports.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Screens for all reports — pnl, expenses, tax, cashflow, balance, flagged, register (read view), and k1 — with date controls matching the TUI report navigation: previous/next period paging and month/year granularity toggle per each report's declared DateGranularity. Export buttons (PDF and text) call the download endpoints. Include a print-friendly stylesheet. The K-1 view surfaces the needs-mapping section and auto-mapped note like the existing report.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All reports render with figures matching the CLI text output for the same period
- [x] #2 Date navigation parity: previous/next paging and month/year toggle where the report supports it
- [ ] #3 Export as PDF and text buttons work per report; pages print cleanly
- [x] #4 K-1 view shows needs-mapping and auto-mapped notes
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Verification

Web: npm run lint, npm run typecheck, npm test (98 theme + 573 ui + 418 app =
1089 passing), npm run build. Confirmed the print rules reach the shipped
bundle, not just the source: @media print and wc-app-shell::part(sidebar) are
both present in web/dist/assets/index-*.css after a production build.

Cargo, all eight commands, under an isolated HOME (see below):
- cargo fmt --check: clean
- cargo clippy --all-targets -D warnings: clean
- cargo test: 495 + 25 passed, 1 ignored (the fixture capture)
- cargo test --no-default-features: 338 + 26 passed
- cargo test --no-default-features --features serve: 489 + 25 passed, 1 ignored
- cargo clippy --no-default-features --all-targets -D warnings: exactly the two
  known task-34 needless_return lints (cli/dashboard.rs:852,
  cli/report/mod.rs:174) and nothing new

## The parity test was checked for teeth

reports-parity.test.ts passed on its first run, which is not by itself
evidence. Deleting the Total Income row from pnlTable failed it (5 figures
against the CLI's 6); restoring it passed again. So the comparison is really
comparing.

## HOME isolation (task-49)

The two cli::settings_manager tests fail under the developer's real HOME and
pass under an isolated one. Confirmed both directions here rather than assumed:
with the ambient HOME, cargo test --no-default-features fails exactly
toggle_update_check and update_check_loads_from_settings (336 passed, 2
failed); with HOME/XDG_CONFIG_HOME pointed at a scratch directory (and
RUSTUP_HOME/CARGO_HOME left pointing at the real toolchain, or rustup cannot
find a toolchain at all), the same command passes 338/338.

The Claude pre-commit hook runs that command with the ambient HOME, so it
blocked the first commit for those two unrelated tests. Took the sanctioned
route: ran the hook's exact command green under the isolated HOME first, then
committed with git -C.

## Print stylesheet: what is and is not verified

Verified by machine: the rules exist and are the right ones
(packages/theme/__tests__/print.test.ts asserts the @page margin, the :root
token overrides, each ::part hide, the repeating table headings and
break-inside), the sheet composes last so dark mode cannot survive onto paper,
and the whole thing survives the vite CSS pipeline into web/dist.

NOT verified: what a printer actually does. This session has no browser, and
the workspace has no headless browser dependency, so no print preview was ever
rendered. AC #3 is therefore left unchecked. Its export half is proven at every
level available here (URL building against FetchApiClient, the component's
hrefs and disabled state, the screen passing current params, and 31.8's
server-side tests of the bytes and headers), but "pages print cleanly" needs a
human with a print dialog. The checklist to run is in web/README.md under
Reports > Printing: eight reports, both themes, A4 and Letter, and a
multi-page register for the repeating headings.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
All eight reports in the browser — pnl, expenses, tax, cashflow, balance,
flagged, register and the K-1 worksheet — with period navigation, text and PDF
downloads, and a print layout. Six commits on nigel-31-spa.

## Shape

`#/reports` lists the eight; `#/reports?report=pnl&year=2025` renders one. A
query parameter rather than a path segment, because the router has none — the
same reason `#/review?id=185` looks the way it does — and every parameter that
reaches the API lives in the hash, so a report view is a URL you can paste to
someone.

One screen serves all eight. The frame never changes and only the body differs,
so `reports-data.ts` holds a catalog (one entry per report: title, icon, and the
date parameters that route accepts) plus pure table mappers shaped deliberately
like `cli/report/text.rs`. The catalog's `supports` flags gate what reaches the
request: the server answers 400 for a parameter its route does not take, so
`year` on `/api/reports/balance` is an error rather than a no-op.

The period control is driven by the `granularity` field on the response
envelope, not a table in the client — the server is the authority on which of
`year` and `month` a route accepts, and says so on every answer. `allowAll`
stays off, since an unfiltered view belongs to the register browser, and with no
period in the route the screen opens on the current year, which is what
`TableReportView::new` seeds the TUI's own date navigation with.

## Components

Three new in `@nigel/ui`, each with a preview and an axe suite:
`wc-report-table` (columns and rows as data — eight bespoke tables would have
been eight places for a column to drift), `wc-export-links`, `wc-link-grid`.
`wc-register-table` gains `readonly`; `wc-app-shell` exposes its furniture as
parts so the print sheet can reach it.

The K-1 worksheet is composed from panels, tables and notice bars rather than
given a component of its own: every block is already a primitive, and a
`wc-k1-worksheet` would have to take `K1PrepReport` as a property, dragging API
types into `@nigel/ui`. It renders in `format_k1`'s order, including the
auto-mapped note and the needs-mapping section (AC #4).

## Exports

Plain `<a download>` links whose href comes from `client.exportUrl(...)`, never
a string in a screen — a download link is as much a hardcoded address as a
`fetch`, and a Tauri client has no `/api` to serve it. The api-seam guard grew a
rule to enforce that, skipping comment lines so documenting the seam still
works.

PDF is offered only when `/api/status` reports `pdfExport`. A link cannot
inspect its response, so without that check a build without the pdf feature
would save a 501 error envelope as `pnl.pdf` — a failure dressed as a success.

## Figure parity, with teeth

The load-bearing test renders each report from a captured API response and
compares every money figure on the page against every money figure in the
CLI's own text export, on absolute values (`wc-money` always renders the sign;
the text report prints magnitudes and lets colour carry direction). Both sides
come from one seeded database, captured by an `#[ignore]`d test in
`src/server/fixture_capture.rs` — a script would have had to run `nigel init
--data-dir` and repoint the developer's real books.

It passed first try, so it was mutation-checked: deleting the Total Income row
from `pnlTable` fails it, restoring it passes.

## Tests and risk

1089 web tests pass (98 theme, 573 ui, 418 app); the full eight-command cargo
matrix is green with exactly the two known task-34 lints. The one gap is
stated rather than papered over: "pages print cleanly" is unverified, because
this session has no browser. The print rules are asserted and confirmed present
in the built bundle, but AC #3 is left unchecked and a manual checklist is in
web/README.md for whoever has a print dialog.

Follow-up worth knowing about: the api-seam URL rule will bite any later screen
that hardcodes an endpoint, which is the intent.
<!-- SECTION:FINAL_SUMMARY:END -->
