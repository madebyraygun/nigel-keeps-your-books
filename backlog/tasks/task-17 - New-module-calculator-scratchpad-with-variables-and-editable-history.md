---
id: TASK-17
title: 'New module: calculator scratchpad with variables and editable history'
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - enhancement
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/158'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Add a calculator scratchpad module (`nigel calc`) that supports arithmetic with percentages, named variables, and editable history. Accessible from both the dashboard TUI and as a CLI subcommand.

## Detail

Requirements:

* **Arithmetic engine** — supports addition, subtraction, multiplication, division, parentheses, and percentage operations (e.g., `1500 * 12%`, `500 + 10%`, `25% of 8000`)
* **Variables** — users can assign results to named variables and reference them in subsequent expressions (e.g., `revenue = 50000`, `tax = revenue * 0.21`, `net = revenue - tax`)
* **Editable history** — scrollable list of previous expressions; users can navigate to a previous line, edit it, and re-evaluate; all subsequent dependent calculations update accordingly
* **Dashboard integration** — accessible from the dashboard command menu as an inline TUI screen, following the same patterns as other dashboard screens (account manager, rules manager, etc.)
* **CLI subcommand** — `nigel calc` launches the calculator directly from the command line
* **Ephemeral** — calculation history resets each session (not persisted to DB)
* **Money formatting** — results displayed with two decimal places using the project's existing `fmt` helpers
* **Error handling** — invalid expressions show inline error messages without crashing (e.g., division by zero, undefined variables, syntax errors)

Relevant existing patterns:
* Dashboard inline screens: `cli/account_manager.rs`, `cli/rules_manager.rs`, `cli/snake.rs`
* CLI subcommand registration: `cli/mod.rs` (Clap derive `Commands` enum), `main.rs` (dispatch)
* TUI helpers: `tui.rs` (styles, shared rendering)
* Number formatting: `fmt.rs`

## Proposed solution

Add a new `cli/calc.rs` module implementing the calculator as a ratatui TUI screen. Add a simple expression parser that handles arithmetic, percentages, and variable assignment/lookup. Register it as both a dashboard menu option and a `nigel calc` CLI subcommand. History is stored in-memory as a `Vec` of expression/result pairs with support for cursor-based editing and re-evaluation.

## Acceptance Criteria

- [ ] `nigel calc` launches the calculator scratchpad from the CLI
- [ ] Calculator accessible from the dashboard command menu
- [ ] Supports basic arithmetic: `+`, `-`, `*`, `/`, parentheses
- [ ] Supports percentage operations: `1500 * 12%`, `500 + 10%`
- [ ] Supports variable assignment and reference: `x = 100`, `y = x * 2`
- [ ] Editable history: navigate to previous lines, edit, and re-evaluate
- [ ] Dependent calculations update when an earlier line is edited
- [ ] Invalid expressions show inline errors (division by zero, undefined vars, syntax errors)
- [ ] Results formatted with two decimal places
- [ ] History is ephemeral (not saved to DB)
- [ ] All linting checks pass
- [ ] Update test coverage
- [ ] Create or update documentation, making sure to remove any out of date information
- [ ] **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user

---
*Migrated from [GitHub issue #158](https://github.com/madebyraygun/nigel-keeps-your-books/issues/158)*
<!-- SECTION:DESCRIPTION:END -->
