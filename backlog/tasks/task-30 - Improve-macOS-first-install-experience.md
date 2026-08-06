---
id: TASK-30
title: Improve macOS first-install experience
status: To Do
assignee: []
created_date: '2026-08-06 04:52'
labels:
  - release
  - docs
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The macOS release asset (nigel-universal-apple-darwin) is a raw signed+notarized universal Mach-O binary with no extension. Double-clicking it in Finder does nothing useful, browsers strip the executable bit, and the README's Install section says 'download a pre-built binary' with no chmod/PATH steps. Constraint: the bare asset name is load-bearing — the in-app updater downloads exactly nigel-universal-apple-darwin from the latest release, so it must remain unchanged; improvements are additive. Candidates: a .tar.gz artifact that preserves the executable bit, a Homebrew formula/tap, and concrete install steps (chmod +x, move onto PATH) in the README.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 README Install section documents the exact macOS steps (chmod +x, move onto PATH, first run)
- [ ] #2 A first-install artifact or channel exists that preserves the executable bit (tarball and/or Homebrew), published alongside the unchanged nigel-universal-apple-darwin asset
- [ ] #3 nigel update continues to work unmodified for existing installs
<!-- AC:END -->
