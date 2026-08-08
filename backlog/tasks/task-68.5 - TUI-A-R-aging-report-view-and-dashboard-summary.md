---
id: TASK-68.5
title: 'TUI: A/R aging report view and dashboard summary'
status: Done
assignee:
  - '@opus-team'
created_date: '2026-08-08 00:28'
updated_date: '2026-08-08 03:03'
labels:
  - invoicing
  - tui
dependencies: []
parent_task_id: TASK-68
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The aging report joins the interactive report views (ReportView pattern in cli/report/view.rs) so it is browsable like pnl/expenses, and the dashboard home gains an A/R line (outstanding total, oldest bucket) next to the YTD P&L so money owed is visible on launch. Invoice list/show stay in the Invoices screen (68.4); this task is the reporting half.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Aging is available as an interactive report view alongside the existing reports
- [x] #2 Dashboard home shows outstanding A/R when any open invoices exist
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Spec: docs/superpowers/specs/2026-08-08-task-68-5-aging-report-view-design.md. Plan: docs/superpowers/plans/2026-08-08-task-68-5-aging-report-view.md (11 TDD tasks).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan approved by orchestrator. Rulings on open questions:
1. Web endpoints deferred to 68.6 — accepted; the HTTP 404 asymmetry is the epic's staging, not a defect.
2. Aging joins `report all` (nine files) — consistency wins.
3. `invoice aging` delegates to the shared text formatter; output format change accepted — duplication of bucket arithmetic is the worse evil.
4. One dashboard line (A/R Outstanding), not two.
5. A/R line stays non-interactive; dashboard shortcut assignment is owned by 68.4 (lands second, owns the key map).
6. --as-of deferred; if it comes, it rides with TASK-46 (point-in-time balances) which already owns as-of semantics.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
PR #185 merged. A/R aging is a full ReportKind (slug `aging`, DateGranularity::None): interactive view with bucket summary + per-invoice OPEN INVOICES section (oldest first), `nigel report aging` with text/PDF export, ninth report in `report all`. `invoice aging` delegates to the shared formatter — bucket arithmetic single-sourced in ar_aging_detail with ar_aging re-expressed on top. Dashboard home shows `A/R Outstanding $N  (bucket)` only when open invoices exist, best-effort loaded, with tests pinning the shared amount column and the 40-col width at $999,999.99. Review round fixed 6 findings incl. the gapless label and a literal bulk-export index. 672+64 / 498+65 green.
<!-- SECTION:FINAL_SUMMARY:END -->
