---
id: TASK-16
title: >-
  Make categorizer::matches return Result<bool> instead of silently returning
  false
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - enhancement
  - P2
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/136'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Problem

`categorizer::matches()` has two silent failure modes:

1. `_ => false` for unknown match types — silently reports "no match" instead of erroring
2. `.unwrap_or(false)` for regex compilation — swallows regex syntax errors

If a rule is stored with an invalid `match_type` (e.g., typo `"start_with"` vs `"starts_with"`) or an invalid regex pattern, the rule silently produces zero matches with no indication of why.

## Fix

Change signature from `fn matches(...) -> bool` to `fn matches(...) -> Result<bool>` and propagate errors for unknown match types and regex compilation failures. Update callers in `categorizer::categorize_transactions()` and `cli::rules::test()`.

## Context

Identified during PR #132 review (silent-failure-hunter agent). Currently mitigated by caller pre-validation in `rules::test()`, but the function itself encodes a "failure looks like no match" contract.

---
*Migrated from [GitHub issue #136](https://github.com/madebyraygun/nigel-keeps-your-books/issues/136)*
<!-- SECTION:DESCRIPTION:END -->
