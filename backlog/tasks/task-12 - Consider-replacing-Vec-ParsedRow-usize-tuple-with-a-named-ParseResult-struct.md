---
id: TASK-12
title: >-
  Consider replacing (Vec<ParsedRow>, usize) tuple with a named ParseResult
  struct
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - P2
  - cleanup
  - importers
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/118'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Context

PR #108 changed the parse function return type to `(Vec<ParsedRow>, usize)` where the usize is the malformed row count. This works but is less self-documenting than a named struct. If more fields (e.g., warnings, line numbers of skipped rows) are added later, a named struct would be clearer.

## Proposal

```rust
struct ParseResult {
    rows: Vec<ParsedRow>,
    malformed: usize,
}
```

Follow-up from PR #108 review.

---
*Migrated from [GitHub issue #118](https://github.com/madebyraygun/nigel-keeps-your-books/issues/118)*
<!-- SECTION:DESCRIPTION:END -->
