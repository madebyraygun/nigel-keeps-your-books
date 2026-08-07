---
id: TASK-60
title: Closed-set string fields should be enums
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - tech-debt
  - types
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three shapes where a closed set is carried as an open type:

match_type ('contains' | 'starts_with' | 'regex') is a String validated at runtime by validate_match_type against a MATCH_TYPES array — while the TypeScript side already declares it as a union. category_type ('income' | 'expense') is a String that drives SQL ordering, where an unexpected value silently sorts as an expense. Rust enums with serde(rename_all) would delete both validators, move the error to deserialization, and make the TS unions derivable rather than remembered.

HistoryQuery in routes/reconcile.rs is the only Deserialize struct in the server without rename_all = camelCase. Its single field is `account`, identical in both conventions, so nothing breaks today — but the moment a second word lands (fromMonth, accountId) axum's Query yields None for the mismatched name, which is a silently ignored filter rather than a 400.

ReportKind::All is a CLI-only variant living in the enum the API shares. ParamSpec::for_kind(ReportKind::All) compiles, falls through to the catch-all arm, and yields a YearOnly spec for a route that does not exist.

Explicitly not worth doing: AccountId / CategoryId newtypes. The two never appear in the same request struct and ids arrive through path extractors, so there is no site where one could be passed for the other.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 match_type and category_type are enums validated at deserialization
- [ ] #2 HistoryQuery carries rename_all so a second field cannot be silently ignored
- [ ] #3 ReportKind::All cannot reach the API's parameter table
<!-- AC:END -->
