---
id: TASK-31.11
title: 'SPA: dashboard screen'
status: Done
assignee:
  - '@agent-31.11'
created_date: '2026-08-06 16:26'
updated_date: '2026-08-07 13:23'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.5
  - TASK-31.9
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.11-dashboard.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Web home mirroring the TUI dashboard (cli/dashboard.rs): YTD P&L summary, account balances, monthly income/expense bar chart (get_cashflow data), flagged-transaction count with a call to action into review, quick navigation to the major screens, and the update-available notice when the server-side check finds a newer release.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Shows YTD income, expenses, and net; account balances; and a twelve-month income/expense chart from the API
- [x] #2 Flagged count badge links into the review flow; quick links reach all major screens
- [x] #3 A manual refresh action reloads dashboard data
- [x] #4 Update-available notice appears when the server reports a newer release
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Server: split cli/update.rs into check_with_cooldown() (data) + update_notice() (text); check_and_notify() is now a wrapper. AppState gained an update_available slot filled by a one-shot background task started in serve() only, so build_router/serve_with_shutdown tests stay offline. StatusResponse gained updateAvailable.
- UI: added wc-stat-card, wc-balance-list, wc-bar-chart, wc-notice-bar (each with .preview.ts + describePreviewA11y) and wc-icon-refresh / wc-icon-download.
- App: four ApiClient methods (getPnl/getBalance/getCashflow/getFlagged) with a generic query() helper; dashboard-store.ts fetches them in parallel into independent slots; dashboard-data.ts ports the TUI bucket math; screens/dashboard.ts is nigel-dashboard-screen taking .client from ScreenContext.
- Dropped getApiClient() per the coordinator ruling; the screen takes its client from ScreenContext instead.

Manual smoke (isolated HOME, demo data, nothing touched under the real ~/.config/nigel):
- nigel init --data-dir <tmp> + nigel demo, then nigel serve --no-open --port 5799.
- GET /api/status returned the new field: updateAvailable null, alongside pdfExport true and companyName "Acme Consulting LLC".
- settings.json in the isolated home gained last_update_check 2026-08-07T06:20:26, which is the background check having run and stamped the cooldown.
- The four dashboard endpoints answered with demo data: pnl?year=2026 income 72545.00 / expenses -28998.78 / net 43546.22; balance one account at 79101.64; cashflow 18 months (2025-03..2026-08, so the client tail-12 window spans a year boundary and captions "2025 - 26" — the real case); flagged 84.
- SPA shell served at / with the current bundle (assets/index-Gl50IbOJ.js, 257482 bytes, matching the vite build); an unknown path falls back to index.html.
- Opt-out path: set update_check false and cleared last_update_check, restarted; last_update_check stayed null, so check_with_cooldown returns before stamping, and status still serialized updateAvailable.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds the web dashboard, mirroring the TUI home screen: year-to-date P&L, account
balances, twelve months of income against expenses, the flagged count, and the
update-available notice.

Server (the only src/ change)
- cli/update.rs split: check_with_cooldown() is the data-only check (opt-out,
  24h cooldown, timestamp write, then check_for_update) and update_notice()
  formats the sentence. check_and_notify() is now a wrapper over the two, so
  main.rs and cli/dashboard.rs are untouched and behave identically.
- AppState gained an update_available slot; serve() fills it from a one-shot
  spawn_blocking task. Started from serve() alone, so build_router and
  serve_with_shutdown — and therefore the whole test suite — stay off the
  network. One shot rather than a ticker because the cooldown means a repeating
  task in a foreground server would only ever find its own timestamp.
- StatusResponse gained updateAvailable (the version, or null), documented in
  docs/api.md including that it is null until the background check answers.

@nigel/ui
- wc-stat-card, wc-balance-list, wc-bar-chart, wc-notice-bar, each with a
  co-located preview and describePreviewA11y over every state; plus
  wc-icon-refresh and wc-icon-download.
- wc-bar-chart draws in CSS rather than pulling in a chart library. Its
  arithmetic is an exported barHeights() so the scaling, including the all-zero
  case that would otherwise divide by zero, is unit-tested without a DOM. Bars
  are a picture, so the chart names itself role="img" and repeats the figures in
  a visually-hidden table — a screen reader gets numbers, not rectangles.

@nigel/app
- Four ApiClient methods: getPnl, getBalance, getCashflow, getFlagged, with a
  generic query() helper that omits unset parameters (the server 400s on a
  parameter a route does not support rather than ignoring it). No getRegister —
  31.12 owns that.
- dashboard-store.ts fetches the four in parallel into independent
  {data, loading, error} slots, so a failed balance query shows an inline retry
  on that panel and leaves the P&L and chart standing; retry re-runs only the
  endpoint that failed, the header refresh re-runs all four.
- dashboard-data.ts ports the TUI's bucket math: the last twelve months *that
  have data*, no calendar zero-fill. The server emits only months with
  transactions, and inventing the gaps here would make the web and the terminal
  disagree about the same books. That is also why the window can cross a year
  and why the caption reads "2025 - 26".
- screens/dashboard.ts is nigel-dashboard-screen, taking its client from
  ScreenContext.

Notable test change: nigel-app's unlock test asserted its entire call log, which
now carries the dashboard's four fetches. It asserts the first three as a prefix
instead — what the test is about is that nothing runs before the unlock, and
pinning the exact tail would break for any screen that ever loads anything.

Tests: 534 web (79 theme + 279 ui + 176 app), 520 cargo test, 364 cargo test
--no-default-features. npm lint/typecheck/build clean; cargo fmt and clippy
clean. cargo clippy --no-default-features still reports exactly the two
pre-existing task-34 needless_return lints and nothing new. Manually smoked
against demo data on an isolated HOME: the field, the cooldown stamp, all four
endpoints, the served bundle, and the update_check opt-out.
<!-- SECTION:FINAL_SUMMARY:END -->
