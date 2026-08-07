---
id: TASK-61
title: Document the CLI validation changes, and settle the rules is_active asymmetry
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - docs
  - question
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The task 31 refactor extracted data-layer functions and, in doing so, pulled validation into CLI paths that previously had none. Output is byte-identical across both binaries for every report and list — that half of the parity claim holds — but these five commands now refuse input they used to accept:

- accounts add with a duplicate name (there is no UNIQUE constraint on accounts.name)
- accounts add with an account type outside the four known ones
- rules add and rules update with an unknown --match-type
- rules add with a regex that does not compile
- rules add naming a deactivated category

Every one is an improvement — main was writing dead rules and off-list account types into the database — but they are breaking changes for anyone scripting an idempotent accounts add, and an existing database holding an off-list account_type can no longer be recreated. They should be called out in the changelog rather than shipped under a behaviour-unchanged banner.

rules update also reordered its error precedence: the category is now resolved before rule existence is checked, so `rules update 999 --category Nope` reports the unknown category where it used to report the missing rule.

Open question worth settling: resolve_category_id now filters on is_active = 1, but list_rules joins categories with no such filter. So a rule pointing at a deactivated category still lists and still fires — it just cannot be created or re-pointed. Is that intended, or should the categorizer skip rules whose category has been deactivated?
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The five validation changes are recorded in the changelog as breaking
- [ ] #2 The rules update error precedence is either restored or documented
- [ ] #3 A decision is recorded on whether rules pointing at deactivated categories should still fire
<!-- AC:END -->
