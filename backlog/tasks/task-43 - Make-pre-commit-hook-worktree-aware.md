---
id: TASK-43
title: Make pre-commit hook worktree-aware
status: To Do
assignee: []
created_date: '2026-08-06 21:54'
labels:
  - tech-debt
  - tooling
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
.claude/hooks/pre-commit-checks.sh runs cargo test from the session cwd rather than the repository the commit targets, so a commit in a secondary worktree is gated on the state of a different worktree's tree (observed during task 31.9: commits in nigel-31-spa were blocked by nigel-31's in-progress state). The hook should derive the repo root from the git command's target (e.g. git rev-parse --show-toplevel of the commit's worktree, honoring git -C) and run checks there.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Committing in a secondary worktree runs checks against that worktree, not the session cwd
<!-- AC:END -->
