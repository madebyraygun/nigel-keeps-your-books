---
id: TASK-57
title: End-to-end parity test between the real binary and the captured fixtures
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - testing
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Both report-figure parity sides descend from one call in one process: fixture_capture.rs drives the axum router in-process and the SPA test compares against what it wrote. Nothing anywhere compares a figure the shipped CLI prints against a figure the API serves.

A test that runs the real release binary's text export against the seeded database and diffs it byte-for-byte against the captured fixture would close several gaps at once:

- the Intl-vs-Rust rounding split fixed in task 31 (found by hand, not by a test)
- fixture staleness, since a stale fixture would stop matching the binary
- get_balance / ytd_net_income, which has no test anywhere and a fixture that is permanently $0.00

The manual equivalent was run during review — all eight text exports came back byte-identical over the wire — so the property holds today and is worth pinning before it stops holding.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test runs the built binary's text export and diffs it against the fixture
- [ ] #2 The test covers all eight reports including balance
- [ ] #3 A drifted fixture fails the test rather than being silently compared against itself
<!-- AC:END -->
