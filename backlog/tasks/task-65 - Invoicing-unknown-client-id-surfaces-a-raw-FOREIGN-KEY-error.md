---
id: TASK-65
title: 'Invoicing: unknown client id surfaces a raw FOREIGN KEY error'
status: To Do
assignee: []
created_date: '2026-08-07 23:09'
labels:
  - invoicing
  - bug
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nigel invoice new --client 99 with a nonexistent client id fails with "Error: Database error: FOREIGN KEY constraint failed" instead of a friendly not-found message naming the id (compare accounts/categories, which resolve and report by name). The data layer should check the client exists before inserting, and answer with NigelError::NotFound.

Found during pre-merge testing of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 invoice new with an unknown --client reports the id in a not-found error and creates nothing
<!-- AC:END -->
