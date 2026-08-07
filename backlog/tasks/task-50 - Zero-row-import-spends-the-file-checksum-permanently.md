---
id: TASK-50
title: A zero-row import spends the file's checksum permanently
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - bug
  - importer
  - data-loss
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
importer::import_file writes the imports row — filename, checksum, record_count — before parsing any transaction and without regard to parsed_rows being empty. A statement imported under the wrong format or a mis-keyed column mapping therefore records its checksum with record_count 0, and every later attempt at the same file short-circuits on the duplicate-checksum branch. The rows can never be imported from that file again without deleting the imports row by hand.

Reproduced with the CLI alone on a clean database, no server involved:

    $ nigel import stmt.csv --account Books
    0 imported, 0 skipped (duplicates)
    $ nigel import stmt.csv --account Books
    This file has already been imported (duplicate checksum).
    $ nigel status   # Transactions: 0

Pre-existing on main; the code is byte-identical there. The web import wizard now refuses to submit a zero-row preview (task 31), which removes the easiest way to reach it, but the CLI path and any future caller still can. The fix is to not write the imports row when parsed_rows is empty, and to report the malformed count as the reason instead.

The escape hatch today is DELETE /api/imports/:id, which the user has no reason to look for because the CLI told them the import succeeded. nigel undo only reverses the most recent import.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A zero-row import writes no imports row and no checksum
- [ ] #2 The same file can be re-imported after the format or mapping is corrected
- [ ] #3 The CLI reports why nothing was imported rather than reporting success
- [ ] #4 A regression test covers import-nothing then import-again on one file
<!-- AC:END -->
