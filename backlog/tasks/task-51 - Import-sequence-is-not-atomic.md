---
id: TASK-51
title: The import sequence is not atomic
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - bug
  - importer
  - data-integrity
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
POST /api/imports/confirm and the CLI import both run snapshot, then import_file, then categorize_transactions on one connection with no SQL transaction around any of it. import_file autocommits the imports row and then each INSERT individually; categorize_transactions autocommits each UPDATE. Compare apply_review, undo_review, delete_import and the transactions PATCH, all of which take conn.unchecked_transaction().

A failure partway through — a disk full, a SQLITE_BUSY past the busy_timeout now that the server is multi-threaded, a panic on a stored regex — leaves committed transactions, some categorized and some not, and answers 500. Nothing records the partial state; the only trace is the snapshot path, which travelled in the response that failed.

The retry then compounds it: the upload is deliberately kept so the same uploadId can be resubmitted, import_file finds the checksum already present, and the caller is told the file was already imported and nothing was added — while the rows from the failed run are sitting in the database.

Pre-existing on main. Wrapping the import and categorize steps in one transaction would make the failure roll back to the state the snapshot promises.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A failure during import or categorize leaves the database as it was before the import
- [ ] #2 A partial import cannot be misreported as a duplicate on retry
- [ ] #3 A test covers a failure injected between the import and the categorize step
<!-- AC:END -->
