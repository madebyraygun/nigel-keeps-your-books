---
id: TASK-49
title: Isolate settings_manager tests from the real user config
status: To Do
assignee: []
created_date: '2026-08-07 13:57'
labels:
  - tech-debt
  - testing
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
cli::settings_manager::tests::{toggle_update_check,update_check_loads_from_settings} read the real ~/.config/nigel/settings.json and fail when it holds non-default values (observed with update_check:false and a stale /tmp data_dir). Task 31.10 added a TempConfig test guard (settings::set_config_dir_for_tests) for exactly this hazard; these two tests predate it and should use it. Noted during task 31.12 verification.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 settings_manager tests pass regardless of the developer's real settings.json
<!-- AC:END -->
