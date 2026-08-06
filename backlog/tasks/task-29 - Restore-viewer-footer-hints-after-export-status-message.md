---
id: TASK-29
title: Restore viewer footer hints after export status message
status: To Do
assignee: []
created_date: '2026-08-05 23:19'
labels:
  - reports
  - ux
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After an export in the report viewer, the Exported... status message occupies the footer row until the next period change clears it; scrolling doesn't restore the key hints. Clear status_message on the next keypress in the viewer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Footer hints return on the next keypress after an export message
<!-- AC:END -->
