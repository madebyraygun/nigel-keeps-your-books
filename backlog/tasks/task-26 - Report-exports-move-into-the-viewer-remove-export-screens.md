---
id: TASK-26
title: Report exports move into the viewer; remove export screens
status: Done
assignee:
  - '@claude'
created_date: '2026-08-05 22:19'
updated_date: '2026-08-05 23:33'
labels:
  - reports
  - ux
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the dashboard there is no way to set a time period when exporting — e.g. no way to export the 2025 K-1 prep, only 2026. Direction chosen: option (b) — the report viewer already pages periods (Left/Right, m toggles month/year) and ReportView::date_params() was built to pass the viewed period to exports. Add export to the viewer so what you see is what you export, and delete the standalone export flow (ReportPickerMode::Export, EXPORT_TYPES, ExportFormatPicker screen). Rename the dashboard menu entry 'View a Report' to 'View Reports'. 'Export all' remains available from the CLI (nigel report all) and becomes a low-priority secondary function of the View Reports screen.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 From the View Reports screen, any report can be exported (PDF or text) for the exact period currently displayed, including past years (e.g. 2025 K-1 prep)
- [x] #2 The standalone export picker and export-format screens are removed; the dashboard menu entry is renamed 'View Reports'
- [x] #3 Export All is reachable as a secondary function of the View Reports screen
- [x] #4 CLI report flags (--mode export, --format, --output, report all) are unchanged
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Per docs/superpowers/plans/2026-08-05-viewer-export.md: 1. Viewer export keys (e=PDF, t=text) using date_params + existing do_export helpers 2. Remove export picker/format screens, rename picker View Reports, append Export All entry 3. Docs + changelog
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Exports moved into the viewers: report viewer exports the exact displayed period via e (PDF) / t (text); the dashboard-hosted register browser exports the browsed set via x (PDF — e is inline-edit there) / t, gated off during search/edit input; standalone CLI viewers unaffected. Standalone export picker and format screens deleted; single View Reports picker (renamed) with an Export All Reports entry (Enter=PDF, t=text). CLI flags untouched. Manual pty verification both feature configs; 310+23 / 303+23 tests green. Follow-ups filed: task-27 period-stamped filenames (high), task-28 ANSI stripping, task-29 footer-hint restore.
<!-- SECTION:FINAL_SUMMARY:END -->
