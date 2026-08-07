---
id: TASK-45
title: Split officer compensation from employee wages on the K-1 prep worksheet
status: To Do
assignee: []
created_date: '2026-08-06 22:35'
labels:
  - enhancement
  - tax
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Form 1120-S separates compensation of officers (line 7, detailed on Form 1125-E when total receipts ≥ $500k) from other salaries and wages (line 8). The stock chart of accounts maps all payroll to a single Payroll — Wages category with `form_line 1120S-8`, so the K-1 prep worksheet labels officer compensation as line 8 and cannot produce the line 7/8 split tax software asks for. Today the workaround is summing per-employee gross pay from the Gusto export by hand.

## Detail

The category-level fix is simple: a "Payroll — Officer Wages" expense category with `form_line 1120S-7` (Payroll — Wages stays `1120S-8`). The real design question is the importer: the Gusto XLSX importer books each payroll run as aggregate totals, not per person, so an aggregate wage transaction can't be split by category without knowing which employees are officers and apportioning each run's gross.

Options to weigh:

- **Officer list in metadata + importer split** — store officer names (or Gusto employee ids) in the `metadata` table, have the Gusto importer emit two wage transactions per run (officer / non-officer portions) using the per-employee detail already present in the export. Most accurate; touches the importer.
- **Worksheet-time split** — leave transactions aggregated; the K-1 report reads the officer list and apportions the Payroll — Wages total using a configured ratio or per-year officer gross entered via CLI. Less invasive, but introduces report-time state.
- **Documented manual step** — categories only; a year-end `nigel recategorize` note in docs. Cheapest, least accurate when officer/non-officer runs are mixed within one payroll.

Relevant code: `src/importer.rs` (Gusto importer), `src/db.rs` (DEFAULT_CATEGORIES, metadata table, migration for the new category on existing databases), `src/reports.rs` (K-1 prep worksheet), `docs/`.

Note: when every employee on payroll is an officer (the current situation), line 8 is simply zero and the whole wage total belongs on line 7 — the feature matters most in years with non-officer employees.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A stock or migratable category maps officer compensation to form line 1120S-7, distinct from Payroll — Wages (1120S-8)
- [ ] #2 The K-1 prep worksheet shows officer compensation and other wages as separate line items with correct form-line labels
- [ ] #3 A documented path exists (importer-driven or manual) to get officer wages into the new category from a Gusto import
- [ ] #4 Existing databases pick up the new category/mapping via schema migration
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation, making sure to remove any out of date information
- [ ] #7 All linting checks pass
- [ ] #8 **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user
<!-- AC:END -->
