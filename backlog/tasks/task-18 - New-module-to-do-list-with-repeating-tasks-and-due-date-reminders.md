---
id: TASK-18
title: 'New module: to-do list with repeating tasks and due-date reminders'
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - enhancement
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/160'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Add a to-do list module for tracking recurring bookkeeping and business tasks. Displays as a dashboard widget showing upcoming items and surfaces reminders on login for tasks that are due soon.

## Detail

Requirements:

* **To-do CRUD** — create, edit, complete, and delete to-do items; each item has a title, optional description, due date, and optional recurrence rule
* **Recurrence patterns** — support simple presets (daily, weekly, monthly, quarterly, annually) and calendar-aware options (e.g., 15th of each month, last business day of quarter, first Monday of month)
* **Auto-regeneration** — when a recurring to-do is completed, automatically create the next occurrence based on the recurrence rule
* **Dashboard widget** — display upcoming to-dos on the dashboard home screen alongside existing widgets (P&L, account balances, bar chart); show items due soon with visual priority (overdue items highlighted)
* **Login reminders** — on dashboard launch, display a notification/summary of items due within the reminder window before showing the main dashboard
* **Configurable reminder window** — user-configurable number of days for the "due soon" threshold, stored in DB metadata; default 7 days
* **TUI management screen** — inline TUI screen accessible from the dashboard for managing to-dos (list, add, edit, complete, delete), following existing dashboard screen patterns
* **CLI subcommand** — `nigel todos` to list upcoming to-dos from the command line
* **Persistence** — to-do items stored in a new `todos` table in the SQLite database, added via the schema migration system

Relevant existing patterns:
* Dashboard widgets: `cli/dashboard.rs` (home screen layout with multiple data panels)
* Inline TUI screens: `cli/account_manager.rs`, `cli/category_manager.rs`, `cli/rules_manager.rs`
* CLI subcommand registration: `cli/mod.rs` (Clap `Commands` enum), `main.rs` (dispatch)
* Schema migrations: `migrations.rs` (`MIGRATIONS` array, sequential `up()` functions)
* Per-database settings: `db.rs` (`get_metadata`/`set_metadata` for key-value config)
* Date handling: `chrono` crate already in dependencies

## Proposed solution

Add a `todos` table via a new schema migration with columns for title, description, due_date, recurrence_rule, is_completed, completed_at, and created_at. Add a `cli/todos.rs` module for the data layer and CLI subcommand, and a `cli/todo_manager.rs` for the TUI management screen. Add a dashboard widget panel showing upcoming items. On dashboard startup, check for due-soon items and display a summary before rendering the home screen. Store the reminder window setting in DB metadata as `todo_reminder_days`.

## Acceptance Criteria

- [ ] `todos` table added via schema migration
- [ ] CRUD operations: create, edit, complete, delete to-do items
- [ ] Recurrence presets: daily, weekly, monthly, quarterly, annually
- [ ] Calendar-aware recurrence: specific day of month, last business day of quarter, first Monday of month
- [ ] Completing a recurring to-do auto-creates the next occurrence
- [ ] Dashboard widget shows upcoming to-dos with overdue highlighting
- [ ] Login reminder displays due-soon items on dashboard launch
- [ ] Reminder window is user-configurable (default 7 days), stored in DB metadata
- [ ] TUI management screen accessible from dashboard menu
- [ ] `nigel todos` CLI subcommand lists upcoming items
- [ ] All linting checks pass
- [ ] Update test coverage
- [ ] Create or update documentation, making sure to remove any out of date information
- [ ] **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user

---
*Migrated from [GitHub issue #160](https://github.com/madebyraygun/nigel-keeps-your-books/issues/160)*
<!-- SECTION:DESCRIPTION:END -->
