---
id: TASK-41
title: Add env override for settings path to enable isolated end-to-end tests
status: To Do
assignee: []
created_date: '2026-08-06 20:50'
labels:
  - tech-debt
  - testing
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
settings::get_data_dir() and the config dir are hardcoded to ~/.config/nigel with no injection point, so tests cannot exercise CLI wrappers that open their own connection (e.g. cli/report/text.rs fetchers) without touching the developer's real settings.json. An env override (e.g. NIGEL_CONFIG_DIR) honored by settings.rs would enable true byte-for-byte CLI-vs-API parity tests (noted during task 31.8 planning) and safer integration tests generally.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An env variable overrides the settings/config directory location
- [ ] #2 At least one test uses it to exercise a CLI wrapper end to end against a temp config
<!-- AC:END -->
