---
id: TASK-53
title: delete_account writes three statements with no transaction
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - bug
  - data-integrity
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
cli::accounts::delete_account deletes the account's reconciliations, nulls account_id on its imports, and then deletes the account, as three autocommitted statements.

Under the CLI this was a single-threaded call. DELETE /api/accounts/:id now runs it on a multi-threaded server against a WAL database other requests hold connections to. If a concurrent import inserts transactions into that account between delete_blocker's count and the final DELETE, the foreign key rejects the delete — but the reconciliation history has already been committed and destroyed and the imports rows have already been nulled. The caller gets a 500 saying the delete failed; the account is still there and its reconciliation history is not.

The three statements are byte-identical on main, so this is not a regression — but HTTP concurrency is what makes it reachable. delete_category is a single UPDATE and is fine.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The three statements run in one transaction and roll back together
- [ ] #2 A blocked delete leaves reconciliations and imports untouched
- [ ] #3 A test covers a delete that fails at the final statement
<!-- AC:END -->
