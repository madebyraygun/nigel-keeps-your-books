---
id: TASK-21
title: 'Rules import: bulk import categorization rules from CSV'
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels: []
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/168'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Add a `nigel rules import <file>` command to bulk-import categorization rules from a CSV file. This enables users migrating from QuickBooks (or any other bookkeeping tool) to bring their categorization rules into Nigel without manually adding them one by one.

## Detail

QuickBooks Online can export bank rules as `.xls` files, but the format is proprietary, undocumented, and QBO rejects manually-created spreadsheets. QB Desktop has no export at all. Rather than reverse-engineering a proprietary format, the most practical approach is a **simple, documented CSV format** that users can populate by referencing their QB rules screen.

**Current state:**
- Rules can only be added one at a time via `nigel rules add` CLI or during interactive review
- No bulk import or export mechanism exists for rules
- Rules have these fields: pattern, match_type (contains/starts_with/regex), vendor, category, priority

**Requirements:**
- Define a standard CSV format for rules import with columns: `pattern`, `match_type`, `category`, `vendor`, `priority`
- `pattern` and `category` are required; `match_type` defaults to `contains`, `vendor` and `priority` are optional
- Category column should match by name (case-insensitive) against existing categories
- Validate all rows before importing any (fail-fast on unknown categories, invalid match_types, empty patterns)
- Report summary: total imported, skipped duplicates, errors
- Support `--dry-run` flag to preview without importing
- Add corresponding `nigel rules export` command to export current rules as CSV (enables backup/transfer between Nigel instances)
- Add rules import/export to the dashboard TUI (accessible from the Rules Manager screen)

**Migration workflow for QB users:**
1. Open QB > Banking > Rules, manually transcribe rules into the CSV template
2. Run `nigel rules import rules.csv` — done

## Proposed Solution

Add `import` and `export` subcommands to the existing `nigel rules` command. The CSV format is intentionally simple and tool-agnostic — no attempt to parse QB's proprietary `.xls` export. A `--dry-run` flag on import lets users validate before committing. Export enables round-tripping and backup.

## Acceptance Criteria

- [ ] Define CSV format with headers: `pattern,match_type,category,vendor,priority`
- [ ] `nigel rules import <file>` reads CSV, validates all rows, inserts rules
- [ ] `nigel rules import <file> --dry-run` validates and reports without inserting
- [ ] Category matching is case-insensitive by name; error on unknown categories
- [ ] Invalid match_type values are rejected with clear error messages
- [ ] Duplicate detection: skip rules where pattern + match_type + category already exist
- [ ] Summary output: imported count, skipped count, error count
- [ ] `nigel rules export` writes current active rules to CSV (stdout or `--output <file>`)
- [ ] Dashboard TUI integration: import/export options in Rules Manager screen
- [ ] All linting checks pass
- [ ] Update test coverage
- [ ] Create or update documentation, making sure to remove any out of date information
- [ ] **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user

---
*Migrated from [GitHub issue #168](https://github.com/madebyraygun/nigel-keeps-your-books/issues/168)*
<!-- SECTION:DESCRIPTION:END -->
