---
id: TASK-33.4
title: Desktop unlock and optional OS keychain
status: To Do
assignee: []
created_date: '2026-08-06 16:29'
labels:
  - tauri
  - frontend
dependencies:
  - TASK-33.2
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Unlock screen parity for encrypted databases in the desktop shell, plus an opt-in remember-password backed by the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service). Never plaintext on disk; a clear toggle removes the stored secret. CLI and TUI behavior unchanged.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Encrypted databases prompt with the same unlock UX as web mode
- [ ] #2 Opt-in keychain remembering works on all three platforms and can be turned off, removing the stored secret
- [ ] #3 The password is never written to disk outside the OS keychain
<!-- AC:END -->
