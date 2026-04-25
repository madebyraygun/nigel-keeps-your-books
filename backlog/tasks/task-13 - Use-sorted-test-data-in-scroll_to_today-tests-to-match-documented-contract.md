---
id: TASK-13
title: Use sorted test data in scroll_to_today tests to match documented contract
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels:
  - P2
  - cleanup
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/121'
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Context

The `scroll_to_today` function in `src/browser.rs` documents that it relies on rows being sorted by date ASC. The test `test_scroll_to_today_clamps_selected` populates rows with cycling dates and manually sets one row's date to today, which means the data is not sorted by date.

The test still passes because `rposition` finds the last date <= today regardless of sort order, but the unsorted data is inconsistent with the documented contract.

## Fix

Update the test to use a cleanly sorted dataset.

Follow-up from PR #109 review.

---
*Migrated from [GitHub issue #121](https://github.com/madebyraygun/nigel-keeps-your-books/issues/121)*
<!-- SECTION:DESCRIPTION:END -->
