---
id: TASK-26
title: Report exports move into the viewer; remove export screens
status: To Do
assignee: []
created_date: '2026-08-05 22:19'
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
- [ ] #1 From the View Reports screen, any report can be exported (PDF or text) for the exact period currently displayed, including past years (e.g. 2025 K-1 prep)
- [ ] #2 The standalone export picker and export-format screens are removed; the dashboard menu entry is renamed 'View Reports'
- [ ] #3 Export All is reachable as a secondary function of the View Reports screen
- [ ] #4 CLI report flags (--mode export, --format, --output, report all) are unchanged
<!-- AC:END -->
