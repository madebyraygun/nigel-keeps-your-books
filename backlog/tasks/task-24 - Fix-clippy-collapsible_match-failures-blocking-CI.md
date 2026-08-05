---
id: TASK-24
title: Fix clippy collapsible_match failures blocking CI
status: Done
assignee:
  - '@claude'
created_date: '2026-08-05 19:27'
updated_date: '2026-08-05 19:41'
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

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Collapsed the four flagged match-arm ifs into match guards (goodbye.rs, password_manager.rs, reconcile_manager.rs, splash.rs). No behavior change — guard failures fall through to existing catch-all arms. cargo clippy -D warnings clean, full suite + fmt green on both CI platforms. Merged as PR #174.
<!-- SECTION:FINAL_SUMMARY:END -->
