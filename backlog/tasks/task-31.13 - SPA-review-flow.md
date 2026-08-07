---
id: TASK-31.13
title: 'SPA: review flow'
status: Done
assignee:
  - '@agent-31.13'
created_date: '2026-08-06 16:26'
updated_date: '2026-08-07 14:23'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.6
  - TASK-31.9
references:
  - src/reviewer.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.13-review.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Web equivalent of the interactive review: step through flagged transactions one at a time, pick a category (vendor optional), optionally create a categorization rule from a pattern, with back navigation that undoes the previous decision including any created rule (undo_review semantics, parity with TUI Esc) and skip that leaves the transaction flagged (parity with Tab). Progress indicator and completion summary. Support re-reviewing a single transaction by id, matching review --id.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Sequential review of flagged transactions with category picker and optional rule creation
- [x] #2 Back undoes the previous decision including any created rule; skip leaves the transaction flagged
- [x] #3 Progress indicator during review and a summary at completion
- [x] #4 A single transaction can be re-reviewed by id
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## 0. Scope and what I inherit

Client-only. **No `src/` changes, no `docs/api.md` changes** — 31.6 landed every
endpoint this screen needs and 31.7 owns `src/` this cycle. Everything below is
`web/`, plus the doc updates the project policy requires.

Inherited, not re-created:

- **`ScreenContext`** (31.12): `ScreenDef.render(ctx)` where
  `ScreenContext = { client: ApiClient; params: URLSearchParams; navigate(screen, params?) }`.
  My `renderReview(ctx)` takes it. If 31.12 lands `getApiClient()` from 31.11
  instead, I adopt whichever one carries `params` — I need `params` and will not
  introduce a third mechanism.
- **`wc-panel`** (31.10) for the completion summary and the empty state frame.
- **`CategoryRow`** and **`getCategories()`** (31.12) — consumed, never redefined.
- **`RegisterRow`** (31.12) — it is exactly what `GET /api/review/:id` and
  `POST /api/review/:id/undo` return.
- **`CategoryOption { id, name, categoryType }`** (31.12, exported from
  `wc-register-table`) — my picker takes the same option type.
- Landed: `wc-money`, `wc-spinner`, `wc-empty-state`, `dispatchNcToast`,
  `confirmDialog`, `wc-icon-review`, `wc-icon-flag`, the `FakeApiClient`
  seed/error/call-log conventions, the preview + `describePreviewA11y` harness.
- **Route form is `#/review?id=185`, not `#review/<id>`.** The spec's path
  segment does not parse — `parseHash('#/review/185')` falls back to the
  dashboard. Query param matches 31.12's convention and needs no router change.
  Bare `#/review` is the queue entry the 31.11 dashboard links to.

## 1. Ground truth I verified against the code (not the spec)

Read `src/reviewer.rs`, `src/cli/review.rs`, `src/server/routes/review.rs`,
`src/server/routes/rules.rs`, `src/cli/rules.rs`.

1. **There is no match type to choose.** `reviewer::apply_review` hardcodes
   `match_type = 'contains'`, and `ApplyRequest` has no `matchType` field. The
   TUI has no match-type prompt either. So the form does **not** get a
   match-type select (spec asks for one — see discrepancy 1); it states in
   plain words that the rule matches when the description *contains* the
   pattern, and the live preview sends `matchType: 'contains'` so what you see
   is what gets saved.
2. **The TUI prefills the pattern with the first two words of the
   description**, not the whole description (`review.rs`, `ConfirmRule` →
   `InputRulePattern`). I match the TUI, not the spec's "the description".
3. **`createRule` with a blank `rulePattern` is a `400`**, deliberately, not a
   silent rule-less success. The form blocks it client-side *and* renders the
   server's message if it arrives anyway.
4. **`apply` 404s only when the transaction row is gone**, not when another tab
   already categorised it — `apply` re-reads the row but never checks
   `is_flagged`. So the "queue changed under me" case is narrower than the spec
   implies; I handle 404 generically and do not pretend to detect the other.
5. **`undo` deletes the rule outright** and returns the restored `RegisterRow`
   (re-flagged, category and vendor cleared). I use that row to refresh the
   card in place rather than re-fetching.
6. **`GET /api/categories` already returns income first, then expense, by
   name** (`ORDER BY CASE category_type WHEN 'income' THEN 0 ELSE 1 END, name`).
   The picker renders in API order with a group heading per type — the
   "grouping" the spec asks for is free.
7. **The TUI's decision stack is `Vec<Option<ReviewDecision>>`** — `None` for a
   skip. I mirror that shape exactly, because it is what makes Back over a skip
   a no-op instead of a stray undo.

## 2. New `@nigel/ui` components

Component-first per `web/README.md`: each is `wc-foo.ts` +
`wc-foo.preview.ts` + `wc-foo.test.ts` ending in `describePreviewA11y(preview)`,
exported from `packages/ui/src/components/index.ts`, before `apps/app` touches it.

### `wc-review-card` (group: Data)

The transaction under review. Presentation only, no events.

- Props: `date` (string), `description` (string), `amount` (number),
  `accountName` (string), `currency = 'USD'`, `locale?`,
  `currentCategory: string | null`, `currentVendor: string | null`.
- Description and amount are the visual anchors (amount via `wc-money`,
  `variant="signed"`); date and account are secondary. Semantic `<dl>` so a
  screen reader gets label/value pairs rather than four loose strings.
- `currentCategory` / `currentVendor` render a "currently" line — only ever
  populated in single-id re-review mode, where `GET /api/review/:id` hands back
  a row that may already be categorised.
- Preview states: `default`, `income` (positive amount), `long-description`
  (wrapping), `re-review` (current category and vendor present).

### `wc-review-progress` (group: Data)

- Props: `current` (1-based), `total`, `reviewed`, `skipped`.
- Renders "n of m" plus a bar, and a quiet "· 3 reviewed · 1 skipped" tail.
- Implemented as a `role="progressbar"` element with
  `aria-valuenow/valuemin/valuemax` and `aria-valuetext="3 of 12"`, over a
  token-styled track. No Web Awesome dependency.
- Preview states: `start`, `mid`, `with-skips`, `single` (total 1), `complete`.

**Why not `wc-progress-dots`, and why not `wa-progress-bar`.** Dots are a
count-me widget: a review queue after a fresh import is routinely 50–200
transactions, and 200 dots is noise, not progress. The TUI itself uses a
*labelled* `LineGauge` reading `3/12`, so a labelled bar is the parity choice.
Against `wa-progress-bar`: I need the count label, the skipped tail and the bar
bound together as one accessible unit, WA's internals are outside our token and
axe control, and 31.10 has an open spike on how WA form elements behave under
jsdom — a dozen lines of ARIA removes that scheduling dependency entirely.
Swapping the inner track for `wa-progress-bar` later is a one-element change
behind the same props if the spike comes back clean and you prefer it.

### `wc-review-form` (group: Data)

The decision form. Dumb and controlled — it holds only its own field state and
emits; the screen owns every network call.

- Props: `.categories: CategoryOption[]`, `busy` (bool), `error: string | null`
  (server message rendered inline), `descriptionForPattern` (string, drives the
  prefill), `canGoBack` (bool).
- Category picker: a filter `<input>` plus a `role="listbox"` of
  `role="option"` rows, grouped income-then-expense with a group heading and
  the `Name (inc)` / `Name (exp)` suffix 31.12 established. Type to filter,
  Up/Down to move, Enter to select — the TUI's exact interaction. See open
  decision 2 for the `wa-select` alternative.
- Optional vendor `<input>`, empty by default. Empty means "no vendor" and the
  field is omitted from the request. (31.12's prefilled-input rule does not
  apply: a flagged transaction has no vendor to preserve.)
- "Create a rule for future matches" checkbox. Checking it reveals the pattern
  input, prefilled with the **first two words** of the description, and the
  fixed-match-type note. Below it, a named slot `rule-test` where the screen
  places the preview panel.
- Apply is a real `<button type="submit">` inside a `<form>`, so Enter submits
  natively and the browser's own validation and focus order apply. Skip and
  Back are sibling buttons; Back is disabled when `canGoBack` is false.
- Events: `nc-review-apply` `{ categoryId, vendor, createRule, rulePattern }`,
  `nc-review-skip`, `nc-review-back`, `nc-rule-pattern-change` `{ pattern }`
  (fired on every keystroke — the screen debounces, because the screen is what
  owns the API seam).
- `reset()` method — the screen calls it after every advance and after a Back.
- Preview states: `default`, `filtering` (query typed, list narrowed),
  `rule-open` (checkbox on, pattern prefilled, slot filled),
  `busy`, `with-error`, `first-transaction` (Back disabled).

### `wc-rule-test-preview` (group: Data)

The "this pattern would also match…" panel. Separate from the form because
31.16 (managers) wants the identical panel next to its rule editor, and because
it keeps `RuleTestResult` out of the form's prop surface.

- Props: `busy` (bool), `.result: RuleTestResult | null`, `error: string | null`.
- Renders `Matches 3 transactions` and the description/count list, busiest
  first; `total: 0` is a plain "Nothing matches yet", not an error state;
  `error` renders the server's message (the invalid-regex 400 path).
- `aria-live="polite"` — the panel updates while focus stays in the pattern
  input, and a silent update is a change the keyboard user never learns about.
- Preview states: `idle`, `busy`, `matches`, `no-matches`, `error`.

Nothing else new. The completion summary is `wc-panel` (31.10) with the counts
and two anchors in its `actions` slot; the empty queue is the landed
`wc-empty-state` with `icon="wc-icon-review"`. If `wc-panel` has not landed when
I start, the summary uses `wc-empty-state` and I switch it after.

## 3. API seam

### `apps/app/src/api/types.ts`

```ts
FlaggedTxn          { id, date, description, amount, accountName }
ReviewApplyRequest  { categoryId: number; vendor?: string; createRule?: boolean;
                      rulePattern?: string }
ReviewApplyResponse { transactionId: number; ruleId: number | null }
ReviewUndoRequest   { ruleId?: number }
RuleMatchType       = 'contains' | 'starts_with' | 'regex'
RuleTestRequest     { pattern: string; matchType?: RuleMatchType }
RuleTestMatch       { description: string; count: number }
RuleTestResult      { total: number; matches: RuleTestMatch[] }
```

`FlaggedTxn` keeps the wire name: the README rule is "one interface per
response struct named exactly as the Rust one", and `reviewer::FlaggedTxn` and
`reports::FlaggedTransaction` are genuinely two different structs. 31.11 mirrors
the second. The names are one letter apart on purpose; I add a doc comment on
each saying which endpoint it belongs to so nobody later "tidies" them together.

`RegisterRow` and `CategoryRow` are 31.12's. I add neither.

### `apps/app/src/api/client.ts` — five methods, no more

```ts
getReviewQueue(): Promise<FlaggedTxn[]>;
getReviewTransaction(id: number): Promise<RegisterRow>;
applyReview(id: number, input: ReviewApplyRequest): Promise<ReviewApplyResponse>;
undoReview(id: number, input: ReviewUndoRequest): Promise<RegisterRow>;
testRule(input: RuleTestRequest): Promise<RuleTestResult>;
```

Added to `interface ApiClient`, `FetchApiClient` and `FakeApiClient` in
lockstep. `request()` stays private. **`testRule` is mine** — no sibling plan
claims it; 31.16 will consume it unchanged.

`FakeApiClient` additions follow the established shape: mutable public fixtures
(`reviewQueue`, `reviewRows: Map<number, RegisterRow>`, `ruleTest`), one
`<method>Error` field per method for injection, and call-log strings matching
31.12's format — `getReviewQueue`, `getReviewTransaction:185`,
`applyReview:42:{"categoryId":12,"createRule":true,"rulePattern":"ADOBE CC"}`,
`undoReview:42:{"ruleId":7}`, `testRule:{"pattern":"ADOBE","matchType":"contains"}`.
The fake's `applyReview` mutates its own seeded row (clears the flag, sets the
category) and its `undoReview` puts it back, so a round-trip test asserts real
state and not just a call log.

## 4. The screen

`screens/review.ts` keeps `renderReview(ctx)` and returns
`html`<nigel-review-screen .client=${ctx.client} .params=${ctx.params} .navigate=${ctx.navigate}>`.
`screens/review-screen.ts` holds `nigel-review-screen`, a
`SignalWatcher(LitElement)` — sibling file, per 31.12's precedent, because this
screen is big. The registry entry already exists; I only swap the render body.

### State

```ts
type ReviewItem = { id, date, description, amount, accountName,
                    category: string | null, vendor: string | null };
type Decision   = { transactionId: number; ruleId: number | null } | null;  // null = skipped

phase: 'loading' | 'empty' | 'reviewing' | 'summary' | 'error'
queue: ReviewItem[]
index: number
history: Decision[]
categories: CategoryRow[]
busy: boolean
formError: string | null
ruleTest / ruleTestBusy / ruleTestError
```

`history` is the TUI's `Vec<Option<ReviewDecision>>`, one for one. The summary
counts are **derived** from it — `reviewed` = non-null entries, `skipped` =
nulls, `rulesCreated` = non-null with a `ruleId` — so a Back that pops an entry
corrects the counters for free instead of needing its own bookkeeping.

### Load (on `firstUpdated`, and again when `params.get('id')` changes)

- `id` present and numeric → **single mode**: `getReviewTransaction(id)` only,
  queue of one. A 404 → `phase: 'error'` with a `wc-empty-state` reading
  "Transaction 185 isn't there any more" and a link to the register.
- otherwise → **queue mode**: `getReviewQueue()`. Empty → `phase: 'empty'`.
- Either way, `getCategories()` in parallel.
- Fetching happens in a lifecycle hook, never in `render()`. 31.10's boot gate
  guarantees the element is not constructed while the app is locked, so no
  locked check here.

### Transitions

| Trigger | Effect |
|---|---|
| `nc-review-apply` | `busy`; `applyReview(currentId, detail)`; on success push `{ transactionId, ruleId }`, advance |
| apply → `ApiError` 404 | `dispatchNcToast` "That transaction is gone — moving on", push **null**, advance |
| apply → `ApiError` 400 | `formError = e.message`, **no advance** (blank pattern, unknown category) |
| apply → anything else | `formError` + a `danger` toast, no advance |
| `nc-review-skip` | push **null**, advance. **No network call at all** — the flag is untouched because nothing is sent |
| `nc-review-back` (index > 0) | pop; `index--`; if the popped entry is non-null → `undoReview(txnId, { ruleId })` and replace that queue item with the returned `RegisterRow`; if null → no call. Reset the form either way |
| back → undo fails | push the entry back, restore `index`, `danger` toast. The stack never desynchronises from the server |
| advance past the end | `phase: 'summary'` |

`advance()` is `index++` then `form.reset()` then the end check — the same
`advance` / `reset_to_pick_category` pairing the TUI uses.

### Rule-test preview

`nc-rule-pattern-change` → debounce 250 ms (one trailing call) → `testRule({
pattern, matchType: 'contains' })`. A blank or whitespace pattern cancels the
timer and clears the panel without calling. In-flight results are dropped if
the pattern has changed since (a sequence token, not `AbortController` — the
client seam owns `fetch`, not the screen). `ApiError` 400 → `ruleTestError`
rendered inline in the panel; the form stays submittable, because a bad
preview is not a bad decision.

### Keyboard

- **Enter applies** — native form submit, no key handler.
- **Esc goes back** — a `keydown` listener on the host, ignored when a
  `wc-confirm` is open.
- **Tab does not skip.** This is a deliberate divergence, flagged as
  discrepancy 3. Tab is the browser's focus key; rebinding it inside a form of
  five controls would strand keyboard and screen-reader users. Skip is a
  labelled button that sits next in the focus order after Apply.
- A quiet hint line reads `Enter apply · Esc back`, and the buttons carry the
  rest.

### Summary and empty

Summary is a `wc-panel` headed "Review complete" with a `<dl>` of reviewed /
skipped / rules created, and two anchors in the `actions` slot —
`#/register` and `#/dashboard`, plain links per 31.11's "no duplicate nav grid"
rule. Single mode says "Transaction reviewed", matching the TUI's own wording.
Empty queue is `wc-empty-state` with `wc-icon-review` and a register link.

No cross-store invalidation: the 31.11 dashboard fetches its flagged count on
connect, so navigating back to it is already fresh.

## 5. Tests

`packages/ui` — component tests beside each new component, each closing with
`describePreviewA11y(preview)`, so every preview state above is also an axe
case. Behavioural coverage: `wc-review-form` filter narrows the listbox and
Up/Down/Enter selects; checking create-rule prefills the pattern with the first
two words and reveals the slot; submit emits `nc-review-apply` with the exact
detail; `reset()` clears every field. `wc-review-progress` computes the right
`aria-valuenow` and renders "n of m". `wc-rule-test-preview` renders total 0 as
a message rather than an error.

`apps/app/src/screens/review-screen.test.ts`, mounting `nigel-app` with a
`FakeApiClient` and driving `location.hash` (the established pattern):

1. Queue mode renders the first flagged transaction and "1 of 3".
2. Apply sends `applyReview` with the form's values and advances to the second.
3. **Apply → Back round-trip**: Back calls `undoReview` with *the ruleId the
   apply response returned*, re-presents transaction one with a cleared form,
   and the fake's row is flagged and uncategorised again.
4. Back over a **skip** issues no `undoReview` at all (asserted on the call log).
5. Skip advances and the call log contains **no mutation call**; the fake's row
   is still flagged.
6. Mixed run of applies, skips and one rule → summary shows the right
   reviewed / skipped / rules-created counts.
7. Single-id mode (`#/review?id=185`) calls `getReviewTransaction` and **never**
   `getReviewQueue`, shows "1 of 1", and lands on the summary after one apply.
8. Single-id 404 renders the not-found state.
9. **Queue 404 on apply** (`applyError` = an `ApiError` with status 404)
   advances, dispatches a toast caught on a window listener, and records a
   skip — so a following Back does not try to undo it.
10. 400 on apply renders the message inline and does not advance.
11. **Debounce**: three `nc-rule-pattern-change` events inside the window
    produce exactly one `testRule` call, carrying the last pattern
    (`vi.useFakeTimers`).
12. Invalid regex (`testRuleError` = 400) renders inline in the panel and the
    form still applies successfully afterwards.
13. Empty queue renders the empty state and no card.
14. A failing `undoReview` leaves `index` and the history where they were — no
    phantom advance.

`registry.test.ts`: the review entry still renders under the new signature and
stays `inNav`.

`vite.config.ts`: if 31.10 has not yet added `['**/screens/**', 'jsdom']` to
`environmentMatchGlobs`, I add it — my screen test cannot run under node.

Every test uses `FakeApiClient`; the api-seam guard forbids `fetch` anywhere
outside `src/api`, tests included.

## 6. Docs

- `web/README.md` — the new components in the library list, and the review
  route's `?id=` param.
- `CLAUDE.md` — a `cli/review` counterpart line under the SPA section noting the
  review screen and its endpoints.
- `docs/api.md` untouched. No endpoint changes.

## 7. Order

1. `wc-review-card` + preview + axe.
2. `wc-review-progress` + preview + axe.
3. `wc-rule-test-preview` + preview + axe.
4. `wc-review-form` + preview + axe (the biggest; the listbox a11y is the risk).
5. Types, then the five client methods, then the `FakeApiClient` fixtures.
6. `nigel-review-screen`: load → present → apply → advance → summary.
7. Back / undo, then skip, then the 404 and 400 paths.
8. Rule-test debounce and the preview wiring.
9. Screen tests, registry test, docs.

## 8. Verification

`npm --prefix web install` once, then `npm --prefix web test`,
`npm --prefix web run lint`, `npm --prefix web run typecheck`,
`npm --prefix web run build` — all four clean, output pasted into the notes.
`cargo` is not run and `src/` is not touched: 31.7 owns it this cycle.

## 9. Discrepancies and open decisions

1. **No match-type select, contrary to the spec.** `apply` has no `matchType`
   field and `apply_review` hardcodes `contains`, so a select would either be
   inert or would make the preview lie about what gets saved. The form states
   the contains behaviour in words. *Decision needed only if you want the
   alternative*: add `matchType` to `ApplyRequest` and to
   `reviewer::apply_review` — small, but it is a `src/` change while 31.7 holds
   `src/`, and it changes `docs/api.md`. My default is to ship without it and
   let 31.16 raise it if the managers screen needs it.
2. **Category picker: custom listbox, not `wa-select`.** 31.12 flags WA 3's
   searchable-select attribute as unconfirmed and 31.10 has an open jsdom spike
   on WA form elements. A filter input plus `role="listbox"` is TUI-faithful,
   jsdom-safe and fully under our axe control. If 31.12 lands a working
   searchable `wa-select` first, I will use theirs instead and say so.
3. **Tab does not skip.** The spec asks for Tab parity with the TUI. On the web
   Tab is the focus key and a form with five controls needs it. Esc-for-back
   and Enter-for-apply both carry over unchanged. Say the word if you want a
   modifier binding (`Alt+S`) instead of relying on the button.
4. **Rule pattern prefills with the first two words**, which is what the TUI
   does, not the whole description as the spec says. The field is editable and
   the live preview shows the consequence immediately.
5. **`#/review?id=185`, not `#review/185`** — the router does not parse path
   segments. Flagging because the spec spells the other form.
6. **Blocked on 31.12's `ScreenContext`.** 31.12 calls this "the one item I want
   sequenced explicitly". Single-id mode needs `ctx.params`. If it lands
   differently I adopt whatever carries params; I will not add a third seam.
7. **`wc-panel` dependency** — the summary uses 31.10's panel. If 31.10 slips,
   the summary ships on `wc-empty-state` and switches later.
8. Not asked for, not built: deep-linking from the summary into a specific
   register row. 31.12 left `#/register?id=` unresolved; my links are plain
   `#/register` and `#/dashboard`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation

Built on the landed 31.10/31.11/31.12 seams: ScreenContext from screens/context.ts, wc-panel, and 31.12 getCategories/CategoryRow/RegisterRow. No src/ or docs/api.md changes — 31.6 landed every endpoint this needed.

- Followed the landed screen convention (screens/review.ts holds the element and renderReview, screens/review-data.ts holds the pure logic) rather than the review.ts + review-screen.ts split the plan described. All three sibling screens use the former; consistency with what shipped beat consistency with the plan text.
- Ruling 2 (reuse the register combobox rather than invent a second) was taken further than "copy the pattern": extracted wc-category-picker as a shared component, and lifted CategoryOption + categoryLabel into category-option.ts so there is one label implementation. wc-register-table keeps its own inline editor — its tests query its shadow DOM directly, so swapping it to the new child element would have broken six landed tests for no user-visible gain. Folding it in is a clean follow-up.
- axe caught two real ARIA bugs in the picker during development: li role="group" is not an allowed role inside a listbox, and a listbox with zero options violates aria-required-children. Fixed by moving to div-based listbox/group/option and by treating an empty filter result as not-expanded rather than as an empty listbox.
- Found and closed a gap the plan missed: once the queue completed there was no way back to correct the last decision, because the form (and its Back button) was gone. The summary now carries a "Back to the last transaction" action wired to the same undo path.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds the SPA review flow: step through flagged transactions one at a time, with back navigation that genuinely undoes.

## What changed

**Screen** — `screens/review.ts` (`nigel-review-screen`) is the web `reviewer.rs`. It enters at `#/review` for the queue or `#/review?id=185` for a single re-review (`nigel review --id`; the router has no path segments, so the id is a query parameter). Pure logic lives in `screens/review-data.ts`.

**Back undoes rather than re-shows.** Each applied decision is pushed onto a stack of `{ transactionId, ruleId }`; Back pops one and calls `POST /api/review/:id/undo` with that rule id, re-flagging the transaction and deleting the rule outright. A skip pushes `null` — the same `Option<ReviewDecision>` the TUI stacks — so stepping back over a skip issues no request rather than clearing a category an earlier session set. A failed undo pushes its decision back, so the stack can never claim a decision the server still holds. Summary counts are derived from the stack, so a Back corrects them for free.

**Components** (`@nigel/ui`, each with a preview and an axe suite): `wc-review-card`, `wc-review-progress`, `wc-review-form`, `wc-rule-test-preview`, and `wc-category-picker` — the searchable combobox lifted out of the register so both editors share one implementation and one `categoryLabel` (`category-option.ts`).

**API seam**: `FlaggedTxn`, the review request/response types and `RuleTestResult` in `types.ts`; five methods (`getReviewQueue`, `getReviewTransaction`, `applyReview`, `undoReview`, `testRule`) added to `ApiClient`, `FetchApiClient` and `FakeApiClient` in lockstep. The fake really moves rows between flagged and categorized, so the round trip is tested rather than just logged.

## Deliberate departures from the TUI

- **Tab does not skip.** On the web Tab is the key that moves between the form controls; rebinding it would strand anyone not using a mouse. Skip is a button; Enter applies and Esc goes back.
- **No match-type control.** `reviewer::apply_review` writes `contains` and the apply route carries no field for anything else, so a select would be inert or would make the live preview lie about what gets saved. The form says so in words. 31.16 owns full match-type editing.
- **Pattern prefills with the first two words** of the description, which is what the TUI does — a bank line ends in a transaction id that will never repeat.

## Robustness

A 404 on apply (another tab, or an undone import) advances with a toast and records a skip, so a later Back does not undo a decision that was never made. A 400 renders inline without advancing. The rule-test call is debounced at 250ms with a sequence token dropping stale responses; an invalid regex renders in the panel and still leaves the decision applicable.

## Tests

712 web tests pass (445 `@nigel/ui`, 267 `@nigel/app`), 65 of them new: 27 screen tests (apply, back round-trip carrying the right ruleId, back-over-skip issuing no undo, skip mutating nothing, summary counts, single-id mode, 404 advance, 400 inline, undo-failure stack integrity, debounce, empty queue) plus 11 pure-logic and component suites. Lint, typecheck and build are clean. No Rust changed; `cargo test` passes 495/495 single-threaded.

## Follow-ups

- `wc-register-table` still has its own inline combobox; folding it onto `wc-category-picker` is a clean follow-up its tests would need rewriting for.
- Unrelated to this work: `cargo test` fails 8 server tests when run in parallel and passes all 495 with `--test-threads=1`. The server tests share process-global state (the db password lives in a process-wide `Mutex`), so they are not parallel-safe. Pre-existing on this branch and worth its own task.
<!-- SECTION:FINAL_SUMMARY:END -->
