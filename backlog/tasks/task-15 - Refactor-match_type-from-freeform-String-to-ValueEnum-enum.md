---
id: TASK-15
title: Refactor match_type from freeform String to ValueEnum enum
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - enhancement
  - P2
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/135'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Problem

`--match-type` is a plain `String` across all rule commands (`add`, `update`, `test`). Validation happens at runtime in each caller. This means:

- `--help` doesn't show valid options
- Each caller must independently validate the value
- The `_ => false` wildcard in `categorizer::matches()` silently returns false for unknown types

## Fix

Introduce a `MatchType` enum with `clap::ValueEnum` derive:

```rust
#[derive(Clone, ValueEnum)]
enum MatchType {
    Contains,
    StartsWith,
    Regex,
}
```

Use it in `RulesCommands::Add`, `RulesCommands::Update`, `RulesCommands::Test`, and `categorizer::matches()`. This eliminates runtime validation, provides better `--help` output, and removes the `_ => false` wildcard.

## Context

Identified during PR #132 review (silent-failure-hunter agent).

---
*Migrated from [GitHub issue #135](https://github.com/madebyraygun/nigel-keeps-your-books/issues/135)*
<!-- SECTION:DESCRIPTION:END -->
