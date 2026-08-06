---
id: TASK-31.15
title: 'SPA: report views'
status: To Do
assignee: []
created_date: '2026-08-06 16:27'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.5
  - TASK-31.8
  - TASK-31.9
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Screens for all reports — pnl, expenses, tax, cashflow, balance, flagged, register (read view), and k1 — with date controls matching the TUI report navigation: previous/next period paging and month/year granularity toggle per each report's declared DateGranularity. Export buttons (PDF and text) call the download endpoints. Include a print-friendly stylesheet. The K-1 view surfaces the needs-mapping section and auto-mapped note like the existing report.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All reports render with figures matching the CLI text output for the same period
- [ ] #2 Date navigation parity: previous/next paging and month/year toggle where the report supports it
- [ ] #3 Export as PDF and text buttons work per report; pages print cleanly
- [ ] #4 K-1 view shows needs-mapping and auto-mapped notes
<!-- AC:END -->
