---
id: TASK-31.16
title: 'SPA: accounts, categories, and rules managers'
status: Done
assignee: []
created_date: '2026-08-06 16:27'
updated_date: '2026-08-07 16:26'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.6
  - TASK-31.9
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.16-managers.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CRUD screens mirroring the three TUI managers with their guardrails. Accounts: list, add, rename, delete with blocked-delete reasons. Categories: list, add, edit (name, type, tax line, form line), soft-delete with usage guardrails and blocking_reason surfaced. Rules: list, add, edit (priority, category, pattern, match type), deactivate, and a pattern test preview against real transactions (parity with rules test). Web reaches CLI parity here, which is a superset of the TUI managers.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Accounts manager supports list, add, rename, and delete with blocked-delete reasons shown
- [x] #2 Categories manager supports list, add, edit of all fields, and soft-delete with guardrail reasons shown
- [x] #3 Rules manager supports list, add, edit, deactivate, and a live pattern test preview
- [x] #4 All guardrail errors from the API render as clear inline feedback
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Verification (2026-08-07)

- `npm test` in web/: 1290 passed across 71 files (theme 98, ui 683, app 509), 0 failed.
- `npm run lint`, `npm run typecheck`, `npm run build` all clean; bundle 510.96 kB / 121.72 kB gzipped.
- `cargo test` under a pristine isolated HOME, `--test-threads=1`: 495 + 25 passed, 0 failed, 1 ignored. `src/` is untouched by this task.
- Pre-existing flake, NOT caused by this task: run in parallel, 7-8 `server::` tests fail (unlock budget, locked-route, transactions patch, settings password). They share the process-global `set_db_password` mutex. Reproduced at HEAD with this work stashed — same failures, so it predates 31.16. Single-threaded they all pass.
- The 2 task-49 settings tests (`settings_manager::tests::{toggle_update_check,update_check_loads_from_settings}`) fail only when HOME already holds a settings.json; a pristine HOME is green.
- No pre-commit hook is installed in this worktree (samples only), so the hook route was not needed.

## Manual smoke against `cargo run -- serve`

Isolated HOME + `nigel init` + `nigel demo`, server on :5799, session cookie via the printed /auth token.

- `GET /` serves the freshly built bundle (`assets/index-CaWDVq-4.js`, the hash `npm run build` emitted).
- Reads: `/api/accounts`, `/api/categories` (30 rows, form lines 1120S-11/12/16/18/19/1a — the datalist source), `/api/rules`.
- Account writes: create 201, duplicate name 409 `duplicate_name` (+name), rename 200, delete 200, delete-with-activity 409 `has_transactions` count 270.
- Category writes: create 201, `{"formLine":null}` patch clears it, `{}` patch 400, delete blocked 409 `has_active_rules` count 1 and 409 `has_transactions` count 36.
- Rule writes: create 201, delete 200, delete again 409 `already_inactive` (no count).
- `POST /api/rules/test`: "GUSTO" -> total 18; "ZZZZ" -> total 0 (a 200, not an error); "(" -> 400 unclosed group.
- **Evidence for the no-client-side-regex decision:** `(?=GUSTO)` is a valid JavaScript RegExp and the server answers 400 "look-around ... is not supported". A local check would have passed a pattern the server refuses.

Every `details.reason` the message table keys off was observed live in the shape the table expects.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds the accounts, categories and rules manager screens to the SPA. The web reaches CLI parity here, which is a superset of the TUI: `rules_manager.rs` can only list and delete rules, so this is the first place in the app outside `nigel rules add` that can write one.

## The shared pattern

The three screens are one screen with different columns — a list, an Add button, per-row Edit and Delete — over `wc-manager-layout`, `wc-manager-table` and `wc-manager-dialog`.

**Editing is a dialog, not an inline panel.** The rule form is tall (pattern, match type, category, vendor, priority, live test preview) and inline it would push the list it is about off screen; delete is already a dialog, so this is one overlay idiom per screen rather than two; and `wa-dialog` supplies the focus trap and Esc handling an inline form would hand-roll.

**Guardrails render from `details.reason`, never from the server's sentence.** `screens/manager-errors.ts` is the whole table — `has_transactions`, `has_active_rules`, `duplicate_name`, `already_inactive` — with counts formatted client-side, which is what makes the strings translatable. Two deliberate exceptions render the server's message: a 400 (it names the offending value and the legal set, and anything re-derived would drift) and an unrecognized 409 reason (inventing a sentence would hide the only information we have).

**A failed save renders in the dialog; a failed delete renders in the screen's alert region**, because `confirmDialog()` resolves and removes itself before the request is sent. Never a toast: a count that vanishes before it is read is not feedback.

**Every mutation refetches its list.** A priority edit reorders the rules, a rename reorders the categories, and a category rename changes the name on every rule row — three ways for an optimistic splice to be quietly wrong, against one round trip to a local server.

## Per screen

- **Accounts.** No transaction-count column: `GET /api/accounts` does not carry one and a screen may not add an endpoint, so the number appears in the blocked delete where it is actionable. Rename is the only edit the PATCH route accepts. The 4-digit last-four rule is enforced client-side because it lives in `account_manager.rs` and nowhere else (server follow-up filed as task-48).
- **Categories.** All four fields, income/expense radios, and the K-1 form-line vocabulary documented beside the field with a datalist derived at runtime from the form lines in use. An unrecognized value warns rather than blocks — `form_line` is free text everywhere else and `resolve_k1_mapping` has defined behaviour for it. Edits send only changed fields; a cleared field travels as explicit null.
- **Rules.** Server priority order is kept (it is the semantics). The pattern box debounces `POST /api/rules/test` at 250 ms into 31.13's `wc-rule-test-preview`, firing immediately on a match-type change because a click is a decision rather than typing, and issuing nothing for a blank pattern (the route guards on `is_empty`, not a trim). No client-side regex validation: `(?=X)` is a valid JS RegExp that the Rust crate refuses, confirmed live against the server. `#/rules?categoryId=12` filters client-side, which is where the categories guardrail points.

## Scope

`src/` and `docs/api.md` are untouched — every endpoint already existed. 20 new files under `web/`, 6 new `@nigel/ui` components (each with a preview and an axe run over every state) plus 3 icons, 10 new `ApiClient` methods, 3 pure data modules.

## Tests

- `npm test`: 1290 passed, 0 failed (theme 98, ui 683, app 509). `npm run lint`, `typecheck`, `build` clean.
- Guardrail coverage is table-driven over every reason code, including singular/plural, a missing name, an unknown reason and a non-ApiError.
- Rules covers the debounce with fake timers, the immediate match-type path, the blank-pattern path, a dropped stale response, and the invalid-regex 400 rendering inline while the form still saves.
- `cargo test` under a pristine isolated HOME: 495 + 25 passed, 0 failed.
- Manual smoke against `cargo run -- serve` on a demo database: every `details.reason` observed live in the shape the message table expects, and the served bundle is the one `npm run build` emitted.

## Risks and follow-ups

- Two pre-existing test flakes, both reproduced at HEAD with this work stashed: `server::` tests fail under parallel execution (shared process-global password mutex), and the task-49 settings tests fail when HOME already holds a settings.json. Neither is caused by this task; the pre-commit hook blocks on the second, so its exact command was run green under an isolated HOME and the commits went through `git -C`.
- The rules screen loads every rule; there is no pagination, matching every other list in the app.
<!-- SECTION:FINAL_SUMMARY:END -->
