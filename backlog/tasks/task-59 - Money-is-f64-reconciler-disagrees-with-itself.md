---
id: TASK-59
title: Money is f64, and the reconciler already disagrees with itself
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - tech-debt
  - correctness
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Amounts are f64 end to end — models.rs, reconciler.rs, throughout reports.rs. In a cash-basis bookkeeping tool this is the type change that would prevent real bugs, and there is already a small symptom: reconciler::is_reconciled is computed from the unrounded discrepancy while the serialized discrepancy is rounded to cents, so a 0.014 difference reports discrepancy 0.01 alongside isReconciled false.

The SPA deliberately declines to recompute the tolerance client-side, which is the right call and contains the damage to that one pairing.

The Intl-vs-Rust rounding divergence fixed in task 31 is the same root cause seen from the display end: two languages rounding the same double differently. A minor-units integer type would remove the class rather than the instances.

This is a conversion touching most of the data layer and should not be bolted onto a feature branch. Filed so the decision is explicit rather than inherited.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A decision is recorded on whether to move to minor-units integers
- [ ] #2 is_reconciled and the serialized discrepancy are computed from the same value
- [ ] #3 If the type change is taken, report figures are unchanged byte-for-byte
<!-- AC:END -->
