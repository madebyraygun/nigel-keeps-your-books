---
id: TASK-19
title: Log failed password attempts and surface in settings
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - enhancement
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/162'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Track incorrect password attempts (timestamp, count) and display them in a visible location so the user is aware of any unauthorized access attempts. Builds on #161 which adds inline password input to the splash screen.

## Detail

**Requirements:**
- Log each failed password attempt with a timestamp
- Store attempt records persistently (likely in the `metadata` table or a dedicated table)
- Surface failed attempt history in the Settings screen or a dedicated security section
- Show at minimum: date/time of last failed attempt, total failed attempts since last successful login
- Clear or reset the counter on successful authentication

**Relevant code:**
- `src/db.rs` — `prompt_password_if_needed()` handles password retries; `metadata` table for key-value storage
- `src/cli/settings_manager.rs` — Settings TUI screen where attempt history could be displayed
- `src/cli/password_manager.rs` — password management sub-screen within settings
- `src/migrations.rs` — if a new table is needed for attempt logging

## Proposed Solution

Store failed attempt data in the database (metadata table or new table) and display a summary in the Settings screen alongside existing password management options. On successful login, show a notification if there have been failed attempts since the last successful login.

## Acceptance Criteria

- [ ] Failed password attempts are logged with timestamps
- [ ] Attempt data persists across sessions
- [ ] Settings screen (or appropriate location) displays failed attempt summary
- [ ] User is notified on login if failed attempts occurred since last successful login
- [ ] Counter resets or acknowledges on successful authentication
- [ ] All linting checks pass
- [ ] Update test coverage
- [ ] Create or update documentation, making sure to remove any out of date information
- [ ] **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user

---
*Migrated from [GitHub issue #162](https://github.com/madebyraygun/nigel-keeps-your-books/issues/162)*
<!-- SECTION:DESCRIPTION:END -->
