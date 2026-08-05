---
id: TASK-24
title: Fix clippy collapsible_match failures blocking CI
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-05 19:27'
updated_date: '2026-08-05 19:33'
labels:
  - ci
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CI runs cargo clippy -- -D warnings; four pre-existing collapsible_match lints now fail every build (see actions run 31032592649): src/cli/goodbye.rs:148, src/cli/password_manager.rs:344, src/cli/reconcile_manager.rs:305, src/cli/splash.rs:254. These predate current work and gate CI for all open PRs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 cargo clippy -- -D warnings exits clean on macOS and Linux CI
- [ ] #2 The four collapsible_match sites are collapsed into their outer match arms (no allow attributes)
<!-- AC:END -->
