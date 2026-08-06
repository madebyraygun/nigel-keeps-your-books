---
id: TASK-28
title: Strip ANSI codes from TUI-initiated text exports
status: To Do
assignee: []
created_date: '2026-08-05 23:19'
labels:
  - reports
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Text exports triggered from the dashboard carry ANSI color codes (pre-existing; same do_text_export served the old export flow). Strip escape sequences when writing .txt files.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Exported .txt files contain no ANSI escape sequences
<!-- AC:END -->
