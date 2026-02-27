# Claude Skills

Nigel includes two Claude skills that extend Claude Code with bookkeeping-specific workflows. Skills are triggered automatically when relevant — just describe what you need and Claude will use the right skill.

## Importer Engineer

**Skill:** `.claude/skills/importer-engineer/SKILL.md`

Generates a complete Nigel importer from a bank data file. Point Claude at a CSV or XLSX statement and it will analyze the format, write the parser, add tests, and verify everything compiles.

### When to use

- You have a bank statement file from an unsupported institution
- You want to add support for a new CSV/XLSX format

### Example

```
Here's my Chase checking statement (attached: chase_jan2025.csv). Create an importer for it.
```

### What it does

1. Analyzes the file — identifies headers, date format, amount convention, rows to skip
2. Reads existing importer patterns in `src/importer.rs`
3. Generates a new `ImporterKind` variant with `detect()` and `parse()` functions
4. Adds tests with sanitized sample data from the real file
5. Runs `cargo test` to verify

## CSV Rule Reviewer

**Skill:** `.claude/skills/csv-rule-reviewer/SKILL.md`

Analyzes a bank CSV before import and suggests categorization rules for transactions that would otherwise be flagged as uncategorized. Saves time on manual review by pre-generating `nigel rules add` commands.

### When to use

- Before importing a new statement, to reduce the number of flagged transactions
- When onboarding a new account with many unfamiliar transaction descriptions

### Example

```
Review this BofA statement before I import it (attached: bofa_march2025.csv).
```

### What it does

1. Parses the CSV and extracts unique transaction descriptions
2. Loads existing rules from the database via `nigel rules list`
3. Simulates matching — identifies which descriptions are already covered and which are not
4. Clusters unmatched descriptions by vendor/prefix
5. Suggests `nigel rules add` commands with appropriate categories, vendors, and match types
6. Reports coverage stats and flags descriptions that need manual review

### Example output

```
Coverage: 23 of 35 unique descriptions matched by existing rules

Suggested rules:

nigel rules add "NETFLIX" --category "Software & Subscriptions" --vendor "Netflix" --match-type contains
nigel rules add "WHOLEFDS" --category "Meals" --vendor "Whole Foods" --match-type contains
nigel rules add "TRANSFER TO SAV" --category "Transfer" --vendor "Internal Transfer" --match-type starts_with

Remaining unmatched (3) — review manually:
  - CHECKCARD 0215 MISC PURCHASE
  - POS DEBIT 1847234
  - ACH WITHDRAWAL REF#9182
```
