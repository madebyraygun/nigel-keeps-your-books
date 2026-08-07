---
id: TASK-56
title: Three test assertions that cannot fail
status: To Do
assignee: []
created_date: '2026-08-07 14:12'
labels:
  - testing
  - tech-debt
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while reviewing task 31. Each passes for a reason unrelated to what it claims to check:

1. web/packages/ui/src/components/wc-unlock-card.test.ts — the password-leak test asserts el.outerHTML does not contain the password. outerHTML on a custom element does not serialize the shadow root, and submit() clears the input first, so the value is never in the string under test either way. Start rendering the password into the DOM — an actual credential leak — and this still passes. This is the one component in the tree that handles a password.

2. web/apps/app/src/screens/rules.test.ts — the stale-answer test asserts result?.total is not 99. When result is null, null?.total is undefined, which satisfies it. Replace the guard with code that drops every answer and it passes. This is the only test in the suite that overlaps two in-flight requests.

3. web/apps/app/src/screens/reports-parity.test.ts — moneyTokens strips the sign from both sides and sorts before comparing, so it is blind to a sign error and to any label-to-figure misassignment: swapping two categories' amounts leaves an identical multiset. It also matches only currency tokens, so the percent column is uncovered.

The same shape was found and fixed in uploads.rs is_valid_id during task 31. The author is demonstrably aware of the hazard — exports.rs and the _guarded_probe route are deliberate defenses against it — which makes these look like oversights rather than a pattern.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The unlock-card test fails if the password is rendered anywhere reachable
- [ ] #2 The rules concurrency test asserts the surviving answer's actual value
- [ ] #3 The parity test compares signs on the rows the CLI prints signed, and covers the percent column
<!-- AC:END -->
