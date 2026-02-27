# Issues

## Open

### #3 - Multiple data files

Add the ability to support multiple data files.

---

## Closed

### #1 — Bring back K-1 prep report

Added K-1 preparation worksheet for 1120-S filings. Includes `reports::get_k1_prep()` data layer, `nigel report k1` terminal display, `nigel export k1` PDF export, and inclusion in `nigel export all`. Categories with `form_line` values are grouped into income summary, deduction lines, Schedule K items, and Line 19 other deductions (with 50% meals limitation). Validation warnings for uncategorized transactions and officer comp vs. distributions.

---

### #2 — Claude skill: pre-import CSV reviewer with rule suggestions

Created `.claude/skills/csv-rule-reviewer/SKILL.md` — a Claude skill that analyzes a bank CSV before import, compares transaction descriptions against existing rules, clusters unmatched descriptions by vendor, and suggests ready-to-run `nigel rules add` commands.
