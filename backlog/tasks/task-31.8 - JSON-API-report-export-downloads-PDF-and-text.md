---
id: TASK-31.8
title: 'JSON API: report export downloads (PDF and text)'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - backend
dependencies:
  - TASK-31.3
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.8-export-api.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wire the existing export machinery (cli/export.rs, pdf.rs, report/text.rs) to download endpoints so every report can be exported from the browser with the same content as CLI exports. Respect the pdf feature gate: text export always available, PDF degrades gracefully when the feature is off.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every report downloads as PDF and text with content matching the CLI export output
- [ ] #2 Responses set correct content-type and filename headers
- [ ] #3 Builds without the pdf feature keep text export and return a clear error for PDF requests
<!-- AC:END -->
