---
# nigel-snfs
title: K-1 prep report plugin
status: todo
type: feature
priority: normal
created_at: 2026-02-27T00:56:30Z
updated_at: 2026-02-27T00:56:30Z
blocked_by:
    - nigel-to27
---

Implement the K-1 prep report as a Nigel plugin, per the plan in docs/plans/raygun-bookkeeper-k1-addendum.md.

## Summary
Generates a worksheet that maps categorized transaction data to Schedule K / K-1 line items (Form 1120-S for S-Corps). Not a tax form generator — a data organization tool for handing to a CPA or feeding into TurboTax Business.

## Key Components

### DB Schema (plugin migration)
- `entity_config` table — entity name, type (s_corp/partnership), EIN, tax year, fiscal year end
- `shareholders` table — name, ownership %, officer status, annual compensation, capital

### New Categories
- Charitable Contributions (K Line 12a, separately stated)
- Section 179 Equipment (K Line 11)
- Officer Compensation (Line 7, should match Gusto W-2 totals)

### Report Outputs (`nigel report k1-prep --year YYYY`)
1. Form 1120-S income summary (gross receipts → deductions → ordinary business income)
2. Line 19 other deductions detail
3. Schedule K summary (ordinary income + separately stated items)
4. Per-shareholder K-1 worksheets (allocated by ownership %)
5. Validation checks (uncategorized txns, meals at 50%, distribution proportionality, reasonable comp warning, Gusto W-2 match)

### Key Business Rules
- Officer comp (Gusto W-2 wages) deducted before ordinary income on Line 7
- Payroll taxes → Line 12, employee benefits → Line 18
- Meals stored at full amount, report applies 50% limitation
- S-Corp allocations must be proportional to ownership — no special allocations
- Flag aggressive comp-to-distribution ratios

## Depends On
- Plugin system (nigel-to27) must be implemented first
