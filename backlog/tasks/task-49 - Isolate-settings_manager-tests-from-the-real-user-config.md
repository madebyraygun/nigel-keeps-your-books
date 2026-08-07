---
id: TASK-49
title: Isolate settings_manager tests from the real user config
status: Done
assignee: []
created_date: '2026-08-07 13:57'
updated_date: '2026-08-07 14:38'
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
- [x] #1 settings_manager tests pass regardless of the developer's real settings.json
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed in #182 (commit 18546a7), alongside the guard bug that made this worse
than a stale-config nuisance.

`set_config_dir_for_tests` cleared the override on drop instead of restoring
what it replaced, so a guard falling out of scope handed the real
`~/.config/nigel/settings.json` back to whatever was still running — which then
wrote a tempdir path into it. That is not hypothetical: it happened during the
#182 review and repointed `data_dir` at a directory that had already been
deleted. The setter now returns the previous value and the guard puts it back.

The guard also moved out of `server::testutil` and into `settings` as
`TempConfigDir`, because that is where the override lives and because
`settings_manager` is not behind the `serve` feature and so could not reach the
old one. Both tests now hold it.

Verified by running the full suite against the real `HOME` rather than a
redirected one — 530 lib tests green, and `~/.config/nigel/settings.json`
byte-for-byte unchanged afterwards. Previously those two tests failed against
any config holding non-default values (`update_check: false` was enough).
<!-- SECTION:NOTES:END -->
