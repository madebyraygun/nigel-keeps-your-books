---
name: csv-rule-reviewer
description: Analyze a CSV before import and suggest categorization rules for unmatched transactions
---

# CSV Rule Reviewer

Analyze a bank CSV file before importing and suggest `nigel rules add` commands for transactions that would otherwise be flagged as uncategorized.

## Inputs

The user provides a bank CSV file path. They may also specify the importer format or account name.

## Workflow

### 1. Read the CSV

Read the provided file and identify:
- **Header row** — column names, position in file (may have preamble rows)
- **Date column** — which column contains the transaction date
- **Description column** — which column contains the description/memo
- **Amount column** — single signed column or separate debit/credit columns
- Skip preamble rows, summary rows, and blank lines

### 2. Extract unique descriptions

- Parse all data rows and collect the description values
- Normalize: uppercase, trim whitespace, collapse multiple spaces
- Deduplicate and count occurrences of each unique description

### 3. Read existing rules

Run `nigel rules list` to get current rules, or read the rules from the database directly:
```bash
nigel rules list
```
Note the pattern, match type, category, and vendor for each rule.

### 4. Simulate matching

For each unique description from the CSV:
- Check if any existing rule matches using the rule's match type:
  - `contains` — description contains the pattern (case-insensitive)
  - `starts_with` — description starts with the pattern (case-insensitive)
  - `regex` — description matches the regex pattern
- Track which descriptions are **matched** and which are **unmatched**

### 5. Group unmatched descriptions

Cluster unmatched descriptions by common substrings:
- Extract likely vendor names (first 2-3 words, common prefixes)
- Group descriptions that share the same vendor/prefix
- Sort groups by total occurrence count (most frequent first)

### 6. Read existing categories

Query the database for valid category names:
```bash
sqlite3 ~/Documents/nigel/nigel.db "SELECT name, category_type, description FROM categories WHERE is_active = 1 ORDER BY category_type, name"
```
Use the settings data dir if it differs from the default.

### 7. Suggest rules

For each cluster of unmatched descriptions, suggest a `nigel rules add` command:
- **Pattern**: the common substring that matches all descriptions in the cluster
- **Category**: best-fit from existing categories based on the description context
- **Vendor**: normalized vendor name (clean, title-cased)
- **Match type**: `contains` for most patterns, `starts_with` if the pattern is a prefix
- **Priority**: 0 (default) unless a more specific rule is needed

Format each suggestion as a ready-to-run command:
```bash
nigel rules add "PATTERN" --category "Category Name" --vendor "Vendor Name" --match-type contains
```

### 8. Output

Present a summary:
1. **Coverage stats** — X of Y unique descriptions already covered by rules
2. **Suggested rules** — grouped by category, each with the command to run
3. **Remaining unmatched** — descriptions that couldn't be confidently categorized (suggest manual review)

The user can copy/paste commands directly or approve them interactively.

## Tips

- Prefer broader patterns that catch multiple descriptions over narrow exact matches
- When unsure about a category, suggest the most likely option with a comment noting uncertainty
- For transfers between accounts (e.g., "TRANSFER TO...", "Online Banking Transfer"), suggest the "Transfer" category
- For payroll-related descriptions, check if they match Gusto patterns before suggesting
- Watch for common bank-specific prefixes (e.g., "DEBIT CARD PURCHASE", "ACH DEBIT") that can be stripped to find the actual vendor name
