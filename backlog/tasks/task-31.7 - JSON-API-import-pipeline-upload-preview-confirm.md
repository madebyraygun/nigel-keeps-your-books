---
id: TASK-31.7
title: 'JSON API: import pipeline (upload, preview, confirm)'
status: To Do
assignee: []
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 18:22'
labels:
  - web
  - backend
dependencies:
  - TASK-31.3
references:
  - src/importer.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.7-import-api.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser import flow on top of import_file(): multipart upload to a temp file; a preview step runs import_file with dry_run=true and returns detected format, sample rows, and imported/skipped/malformed counts without mutating anything; a confirm step mirrors the import_manager.rs sequence — pre-import snapshot, import, auto-categorize — and returns full results. Supports explicit format keys including saved csv_profiles, inline generic CSV column mappings, and saving a new profile.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Upload plus preview returns detected format, sample rows, and counts with no database mutation
- [ ] #2 Confirm performs snapshot, import, and auto-categorization, returning imported/skipped/malformed/flagged counts
- [ ] #3 Duplicate files (checksum) and duplicate rows are reported the same way the CLI reports them
- [ ] #4 Generic CSV column mapping and save-profile are supported
- [ ] #5 Upload size limits are enforced and temp files are cleaned up
<!-- AC:END -->
