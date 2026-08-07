---
id: TASK-63
title: Encrypted-database integration test for invoice commands
status: To Do
assignee: []
created_date: '2026-08-07 21:53'
labels:
  - invoicing
  - testing
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/pull/172'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Invoice and client commands unlock encrypted databases through the shared prompt_password_if_needed path, which since PR #178 also reads NIGEL_DB_PASSWORD. That combination is untested: the existing encrypted-db integration tests cover recategorize and core commands, but no test drives an invoice command (e.g. invoice list, invoice new, client add) against an encrypted database via NIGEL_DB_PASSWORD. Same follow-up PR #180 recorded for itself after #178 landed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An integration test runs at least one invoice/client command against an encrypted database unlocked via NIGEL_DB_PASSWORD
- [ ] #2 A wrong NIGEL_DB_PASSWORD on an invoice command fails with the documented error rather than hanging on a prompt
<!-- AC:END -->
