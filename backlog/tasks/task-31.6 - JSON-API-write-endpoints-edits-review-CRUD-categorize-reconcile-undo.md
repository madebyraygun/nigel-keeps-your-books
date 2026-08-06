---
id: TASK-31.6
title: 'JSON API: write endpoints (edits, review, CRUD, categorize, reconcile, undo)'
status: In Progress
assignee:
  - '@agent-31.6'
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 20:46'
labels:
  - web
  - backend
dependencies:
  - TASK-31.3
references:
  - src/reviewer.rs
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.6-write-api.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Mutation endpoints wrapping the existing write data layer: transaction category/vendor edits and flag toggle (reviewer.rs), review apply and undo (apply_review/undo_review including rule cleanup), CRUD for accounts, categories, and rules honoring the existing guardrails (delete blocked when transactions exist, soft-delete semantics, blocking_reason surfaced), rule pattern testing against real transactions, running the categorizer, reconcile, and undo of a specific import by id (undo::delete_import already takes an id).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Transaction edit endpoints cover category, vendor, and flag toggle; review apply and undo work including created-rule cleanup
- [ ] #2 CRUD endpoints for accounts, categories, and rules enforce existing blocking and soft-delete semantics with clear error payloads
- [ ] #3 Endpoints exist to test a rule pattern (dry run), run the categorizer, reconcile an account month, and undo a specific import by id
- [ ] #4 All mutations are rejected while the database is locked
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
IMPLEMENTS AFTER 31.5 LANDS (my routes merge into its data_router(); I reuse its
with_conn helper, its testutil module, and its month/date param validators).

## 1. Route table

All routes mount inside data_router() in src/server/routes/mod.rs, so 31.4's
fail-closed locked guard covers every one of them (423 while locked). Files:
transactions.rs, review.rs, reconcile.rs are new; accounts.rs, categories.rs,
rules.rs, imports.rs gain writes beside 31.5's reads.

| Method | Path | Request body | Success | Data layer |
|---|---|---|---|---|
| PATCH | /api/transactions/:id | {categoryId?, vendor?, flag?} | 200 RegisterRow | reviewer::update_transaction_category / _vendor / set_transaction_flag |
| POST | /api/categorize | none | 200 CategorizeResult | categorizer::categorize_transactions |
| GET | /api/review/queue | - | 200 FlaggedTxn[] | reviewer::get_flagged_transactions |
| GET | /api/review/:id | - | 200 RegisterRow | reports::get_register_row (NEW) |
| POST | /api/review/:id/apply | {categoryId, vendor?, createRule?, rulePattern?} | 200 {transactionId, ruleId} | reviewer::apply_review |
| POST | /api/review/:id/undo | {ruleId?} | 200 RegisterRow | reviewer::undo_review |
| POST | /api/accounts | {name, accountType, institution?, lastFour?} | 201 Account | accounts::add_account (returns id) |
| PATCH | /api/accounts/:id | {name} | 200 Account | accounts::rename_account |
| DELETE | /api/accounts/:id | - | 200 {id, deleted:true} | accounts::delete_account |
| POST | /api/categories | {name, categoryType, taxLine?, formLine?} | 201 CategoryRow | categories::add_category (returns id) |
| PATCH | /api/categories/:id | {name?, categoryType?, taxLine?, formLine?} | 200 CategoryRow | categories::get_category + update_category |
| DELETE | /api/categories/:id | - | 200 {id, deleted:true} | categories::delete_category (soft) |
| POST | /api/rules | {pattern, categoryId, matchType?, vendor?, priority?} | 201 RuleRow | rules::add_rule (NEW) |
| PATCH | /api/rules/:id | {pattern?, categoryId?, matchType?, vendor?, priority?} | 200 RuleRow | rules::update_rule (NEW) |
| DELETE | /api/rules/:id | - | 200 {id, deleted:true} | rules::deactivate_rule (NEW, soft) |
| POST | /api/rules/test | {pattern, matchType} | 200 RuleTestResult | rules::test_pattern (NEW) |
| POST | /api/reconcile | {account, month, statementBalance} | 200 ReconcileResult | reconciler::reconcile |
| GET | /api/reconciliations?account= | - | 200 ReconciliationRecord[] | reconciler::list_reconciliations (NEW) |
| DELETE | /api/imports/:id | - | 200 {id, deletedTransactions} | undo::delete_import |

Conventions: 201 for the three resource creations (Location header omitted --
the body carries the row), 200 for action POSTs and for deletes (the SPA parses
a JSON body on every response; 204 would make that a special case). Static
segments beat captures in axum, so /api/rules/test and /api/review/queue never
reach the :id routes. Unknown request fields are ignored (forward compatible),
absent optional fields mean "leave unchanged".

Null semantics, spelled out because it is the one thing a client cannot guess:
`vendor: null` on PATCH transactions/rules **clears** the column; `taxLine:
null` / `formLine: null` on PATCH categories clear theirs; `categoryId: null`
is a 400, not a clear (the register has no uncategorized-by-edit flow -- use
review undo). Implemented with a `double_option` deserializer
(`#[serde(default, deserialize_with = "...")]` on `Option<Option<T>>`, missing
-> None, JSON null -> Some(None)) added as `pub(crate)` in routes/mod.rs.

POST /api/categorize lives in routes/transactions.rs (it is a bulk transaction
mutation); everything else sits in its domain file.

## 2. Structured guardrails: the error refactor (epic contract)

Today every guardrail and every not-found is `NigelError::Other`, which
From<NigelError> maps to internal/500. Routes cannot recover the reason without
string-matching the message. Fix it once, in src/error.rs, keeping every
Display string byte-identical so the CLI and TUI text (and their existing
assertions) do not move:

    pub enum BlockReason { HasTransactions, HasActiveRules }

    pub struct DeleteBlock { pub subject: &'static str,   // "account" | "category"
                             pub reason: BlockReason,
                             pub count: i64 }
    impl DeleteBlock {
        pub fn transactions(subject: &'static str, count: i64) -> Self
        pub fn active_rules(subject: &'static str, count: i64) -> Self
        pub fn reason_code(&self) -> &'static str  // "has_transactions" | "has_active_rules"
    }
    impl Display for DeleteBlock  // "Cannot delete: {subject} has {count} transaction(s)"
                                  // "Cannot delete: {subject} has {count} active rule(s)"

Four new NigelError variants:

| Variant | Display | HTTP |
|---|---|---|
| NotFound(String) | "{0}" | 404 not_found |
| Invalid(String) | "{0}" | 400 bad_request |
| DuplicateName { kind: &'static str, name: String } | "{kind} name already exists: {name}" | 409 conflict, details {reason:"duplicate_name", name} |
| Blocked(DeleteBlock) | "{0}" | 409 conflict, details {reason, count} |

Plus one remap of an existing variant: NoTransactions { account, month } moves
from not_found to 409 conflict with details {reason:"no_transactions", account,
month} -- the spec's reconcile requirement, and 404 was always wrong for it
(the account exists; the month is empty). It has exactly one producer
(reconciler::reconcile), so nothing else shifts.

Reason codes the API can emit in conflict details: has_transactions,
has_active_rules, duplicate_name, already_inactive, no_transactions. 31.16
renders from these, never from the message.

Call sites converted (message text unchanged at every one):

- accounts.rs: add_account -> Invalid (empty name; NEW invalid-type check),
  DuplicateName; rename_account -> Invalid/DuplicateName/NotFound;
  delete_account -> Blocked/NotFound.
- categories.rs: add_category/update_category -> Invalid x2, DuplicateName;
  rename_category/update_category/delete_category -> NotFound;
  delete_category -> Blocked.
- rules.rs: update/delete -> NotFound ("No rule with ID {id}"), Invalid
  ("Nothing to update ..."), already-inactive -> Conflict (see 2a).
- reviewer.rs: get_transaction_by_id -> NotFound.
- reconciler.rs: unchanged (UnknownAccount/NoTransactions already typed).

### 2a. already_inactive

`rules update`/`rules delete` on a deactivated rule currently error with "Rule
{id} is inactive" / "Rule {id} is already inactive" (Other -> 500). Rather than
stretch DeleteBlock, add a fifth variant:

    NigelError::Conflict { code: &'static str, message: String }  // "{message}"

mapped to 409 with details {"reason": code}. deactivate_rule/update_rule use
code "already_inactive". DuplicateName could have been expressed this way too,
but a typed variant keeps the `name` in details for free; both map through the
same arm shape in server/error.rs.

### 2b. Structured blockers exposed to callers

- `accounts::delete_blocker(conn, id) -> Result<Option<DeleteBlock>>` (NEW) --
  extracted from delete_account's inline count check; delete_account calls it.
- `categories::delete_blocker(conn, id) -> Result<Option<DeleteBlock>>` (NEW,
  from blocking_reason's body over usage_count).
- `categories::blocking_reason` stays, reduced to
  `Ok(delete_blocker(conn, id)?.map(|b| b.to_string()))` -- category_manager.rs
  (the only caller, line ~418) is untouched and its status line reads the same.

The routes do not pre-check: they call delete_account/delete_category and let
NigelError::Blocked become the 409, so the guard has exactly one home.

## 3. New / changed data-layer functions

**src/cli/accounts.rs**
- `pub const ACCOUNT_TYPES: &[&str] = &["checking","credit_card","line_of_credit","payroll"];`
  moved here from account_manager.rs (which imports it; no behavior change).
- `add_account(...) -> Result<i64>` (was Result<()>): returns the new id;
  validates non-empty name and membership in ACCOUNT_TYPES.
- `get_account(conn, id) -> Result<Account>` (NEW; NotFound) -- for POST/PATCH
  responses and PATCH's 404.
- `delete_blocker` as above.
- CLI `add()` printer now delegates to `add_account` instead of its own raw
  INSERT, so the CLI gains the duplicate-name and type checks the TUI already
  had.

**src/cli/categories.rs**
- `add_category(...) -> Result<i64>`; `get_category(conn, id) -> Result<CategoryRow>`
  (NEW, is_active = 1, NotFound); `delete_blocker` as above.

**src/cli/rules.rs** (the epic's "rest of the rules data layer"; joins 31.5's
list_rules/RuleRow in the same file)

    pub const MATCH_TYPES: [&str; 3] = ["contains", "starts_with", "regex"];
    pub fn validate_match_type(match_type: &str, pattern: &str) -> Result<()>;
        // unknown type -> Invalid("Invalid match type: {mt}. Must be one of: ...")
        // regex that will not compile -> Invalid("Invalid regex: {e}")
        // both messages lifted verbatim from today's test()

    pub struct NewRule<'a> { pub pattern: &'a str, pub category_id: i64,
                             pub vendor: Option<&'a str>, pub match_type: &'a str,
                             pub priority: i64 }
    pub fn add_rule(conn: &Connection, rule: NewRule<'_>) -> Result<i64>;

    #[derive(Default)]
    pub struct RuleUpdate { pub pattern: Option<String>, pub match_type: Option<String>,
                            pub vendor: Option<Option<String>>, pub category_id: Option<i64>,
                            pub priority: Option<i64> }
    pub fn update_rule(conn: &Connection, id: i64, update: &RuleUpdate) -> Result<()>;
    pub fn deactivate_rule(conn: &Connection, id: i64) -> Result<()>;
    pub fn get_rule(conn: &Connection, id: i64) -> Result<RuleRow>;
    pub fn resolve_category_id(conn: &Connection, name: &str) -> Result<i64>;  // UnknownCategory

    #[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
    pub struct RuleTestMatch { pub description: String, pub count: i64 }
    pub struct RuleTestResult { pub total: i64, pub matches: Vec<RuleTestMatch> }
    pub fn test_pattern(conn: &Connection, pattern: &str, match_type: &str)
        -> Result<RuleTestResult>;

  test_pattern is today's test() with the printing amputated: same
  validate-then-scan-descriptions loop over categorizer::matches, same
  HashMap<description, count> aggregation, same sort (count DESC, then
  description ASC), total = sum of counts. The CLI's test() keeps its exact
  output by formatting the returned struct (including the "No transactions
  match ..." branch when total == 0).

  add_rule/update_rule call validate_match_type and verify the target category
  exists and is active (NotFound "Category not found: id {id}"); update_rule
  keeps the dynamic-SET builder but takes category_id instead of a name, errors
  Invalid when every field is None, and NotFound/Conflict(already_inactive) as
  in 2a. CLI add/update/delete/test become thin wrappers doing name -> id
  resolution and printing; main.rs is unchanged.

- **rules_manager.rs**: the inline `UPDATE rules SET is_active = 0` (~line 242)
  becomes `rules::deactivate_rule(conn, id)`; the match arms keep today's
  Ok/Err status messages.

**src/reviewer.rs**
- `get_transaction_by_id`: Other -> NotFound (same text).
- `set_transaction_flag(conn, id, flagged: bool) -> Result<()>` (NEW) --
  `UPDATE transactions SET is_flagged = ?1 WHERE id = ?2`, NotFound when the
  row is absent. Touches flag_reason not at all, exactly like today's toggle.
- `toggle_transaction_flag` is re-expressed as read-current-then-set (returns
  the new state as before). Behavior is preserved and it now 404s on a missing
  id instead of surfacing a raw rusqlite no-rows error; browser.rs keeps
  calling it unchanged.
- apply_review / undo_review / update_transaction_category / _vendor: unchanged.

**src/reports.rs**
- `get_register_row(conn, id) -> Result<RegisterRow>` (NEW) -- get_register's
  SELECT narrowed to one id, NotFound otherwise. Reusing RegisterRow means the
  PATCH response, GET /api/review/:id, and 31.12's register rows are one type
  on the wire, and no new serde struct is needed.

**src/reconciler.rs**
    #[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
    pub struct ReconciliationRecord { pub id: i64, pub account_id: i64,
        pub account_name: String, pub month: String,
        pub statement_balance: Option<f64>, pub calculated_balance: Option<f64>,
        pub is_reconciled: bool, pub reconciled_at: Option<String>,
        pub notes: Option<String> }
    pub fn list_reconciliations(conn: &Connection, account: Option<&str>)
        -> Result<Vec<ReconciliationRecord>>;
  JOIN accounts, ORDER BY month DESC, id DESC. Balances are Option because the
  columns are nullable REAL. An `account` filter naming no account is
  UnknownAccount -> 404 (checked first, in the same closure), not an empty list.

**src/cli/undo.rs**
- `import_exists(conn, id) -> Result<bool>` (NEW). delete_import stays as-is
  (it happily "deletes" a missing import and reports 0); the route checks
  existence first inside the same connection so DELETE /api/imports/999 is a
  404 rather than a cheerful 0.

## 4. Server plumbing

**src/server/extract.rs (NEW)** -- two extractors that keep the error envelope
total, since a plain axum rejection answers in text/plain and the SPA parses
every failure as an envelope:
- `ApiJson<T>`: FromRequest with Rejection = ApiError; JsonRejection ->
  bad_request carrying the rejection's body_text (safe here -- no endpoint in
  this task carries a secret; POST /api/unlock keeps its bespoke redacting
  handler).
- `ApiPath<T>`: PathRejection -> bad_request("Expected a numeric id in the
  path.").
Both are used by every route below. (Accepted gap: a wrong *method* on a real
path still yields axum's bodyless 405. Fixing that needs a method fallback on
every MethodRouter; noted in docs/api.md, not implemented.)

Handlers stay three lines each: parse/validate, `with_conn(&state, move |conn|
...)` (31.5's spawn_blocking helper), wrap. Multi-statement mutations run
inside `conn.unchecked_transaction()` so a partial PATCH cannot half-apply:
PATCH /api/transactions (category + vendor + flag) and review apply/undo
(already transactional in the data layer).

Month validation for POST /api/reconcile reuses 31.5's YYYY-MM parser; if that
landed private inside routes/reports.rs I promote it to a `pub(crate) fn
parse_year_month` in routes/mod.rs in the same commit rather than writing a
second one. A malformed month is 400 before it reaches reconciler, where today
it would silently become a NoTransactions.

## 5. Error mapping table

| Situation | NigelError / source | Code | Status |
|---|---|---|---|
| malformed JSON body, wrong types | ApiJson rejection | bad_request | 400 |
| non-numeric :id | ApiPath rejection | bad_request | 400 |
| PATCH with no updatable fields | Invalid | bad_request | 400 |
| categoryId: null on PATCH transactions | route check | bad_request | 400 |
| createRule with no rulePattern | route check | bad_request | 400 |
| empty name, bad accountType/categoryType, bad matchType, bad regex | Invalid | bad_request | 400 |
| malformed month on reconcile | route check | bad_request | 400 |
| unknown transaction / account / category / rule / import id | NotFound | not_found | 404 |
| unknown account *name* (reconcile, reconciliations filter) | UnknownAccount | not_found | 404 |
| unknown category name (CLI path only) | UnknownCategory | not_found | 404 |
| duplicate account/category name | DuplicateName | conflict + {reason:"duplicate_name", name} | 409 |
| delete account/category with transactions | Blocked | conflict + {reason:"has_transactions", count} | 409 |
| delete category with active rules | Blocked | conflict + {reason:"has_active_rules", count} | 409 |
| update/delete an inactive rule | Conflict | conflict + {reason:"already_inactive"} | 409 |
| reconcile a month with no transactions | NoTransactions | conflict + {reason:"no_transactions", account, month} | 409 |
| database encrypted, not unlocked | 31.4 guard | locked | 423 |
| SQL/IO failure, JoinError | Db/Io/Other | internal | 500 |

## 6. Docs

- **docs/api.md**: new subsections under Endpoints -- Transactions (incl.
  categorize), Review, Accounts, Categories, Rules (incl. test), Reconcile,
  Imports. Each: method+path, request-field table (name, type, required,
  null-means-clear where it applies), response struct + abbreviated JSON, and
  that route's error rows. Two shared blocks: a conflict-reason-code table
  (the five codes and what each carries in details) and a short "write
  conventions" note (201 on create, PATCH merge semantics, null-clears-column,
  unknown fields ignored, 405 caveat). The existing error table gains the
  detail that `conflict` details always carry `reason`.
- **CLAUDE.md**: Architecture -- the new routes files and the data-layer
  additions (rules add/update/deactivate/get/test_pattern, accounts/categories
  get_* and delete_blocker, reviewer::set_transaction_flag,
  reports::get_register_row, reconciler::list_reconciliations,
  undo::import_exists); Key Design Constraints -- guardrail failures carry a
  structured reason code alongside the human message; the API's flag edit is
  idempotent (`flag: bool`) where the TUI toggles; rule match_type and regex
  are now validated in the data layer, so `nigel rules add --match-type bogus`
  errors instead of silently creating a rule that never matches.
- **README.md**: no change (serve is described; endpoints live in docs/api.md).

## 7. Tests

Data-layer unit tests (feature-independent, so `cargo test
--no-default-features` covers them) in the modules they belong to:
- rules: add_rule returns a usable id and round-trips through get_rule;
  update_rule partial updates touch only named columns; update_rule with no
  fields is Invalid; update/deactivate on a missing id is NotFound and on an
  inactive rule is Conflict(already_inactive); deactivate preserves hit_count
  (replacing today's SQL-only test); add_rule rejects a bad match_type, an
  uncompilable regex, and an unknown/soft-deleted category_id.
- test_pattern parity: seed descriptions and assert its match set equals a
  hand-rolled `categorizer::matches` filter over the same rows for each of
  contains / starts_with / regex, including case-insensitivity for the first
  two and case-sensitivity for regex; duplicate descriptions aggregate into one
  entry with count 2; ordering is count DESC then description ASC; empty result
  has total 0.
- accounts/categories: delete_blocker returns the right variant and count;
  delete_account/delete_category surface Blocked; duplicate names surface
  DuplicateName; get_account/get_category NotFound; existing message
  assertions (already in the suite) must still pass unchanged -- that is the
  proof the CLI/TUI text did not move.
- reviewer: set_transaction_flag is idempotent (setting true twice leaves one
  flagged row and no error) and NotFound on a missing id; toggle still
  alternates.
- reports::get_register_row: matches the corresponding row from get_register;
  NotFound on a missing id.
- reconciler::list_reconciliations: ordering, unfiltered vs account-filtered,
  nullable balances survive as None, unknown account name is UnknownAccount.
- undo::import_exists.

Integration tests (cfg(feature = "serve")) over 31.5's seeded_db/app/testutil,
extended there with post_json/patch_json/delete_json helpers:
1. Happy path per endpoint group: PATCH transactions changes category, vendor
   and flag in one call and returns the updated RegisterRow; categorize
   returns a CategorizeResult whose numbers match a direct call on an
   equivalent DB; each CRUD create -> read-back through 31.5's list endpoint ->
   update -> delete.
2. Guardrail 409s, one test per reason code, asserting status, code, and
   `details.reason`/`details.count`: account with transactions, category with
   transactions, category with active rules, duplicate account name, duplicate
   category name, update+delete of an inactive rule, reconcile of an empty
   month.
3. 404s: PATCH/DELETE with an unknown id on transactions, accounts,
   categories, rules, imports; review apply/undo on an unknown transaction;
   PATCH transactions naming an unknown categoryId; reconcile naming an
   unknown account; /api/reconciliations?account=nope.
4. 400s: malformed JSON, `{}` with nothing to update, categoryId: null,
   createRule without rulePattern, matchType "bogus", an uncompilable regex on
   /api/rules/test, month "2025-13" on reconcile, a non-numeric :id.
5. Review round-trip: queue -> apply with createRule -> the transaction is
   unflagged and categorized and the rule appears in GET /api/rules -> undo
   with that ruleId -> transaction re-flagged, category and vendor cleared, the
   rule gone from the rules table (not merely deactivated -- undo_review
   DELETEs).
6. Idempotent flag: PATCH {flag:true} twice, then {flag:false}, asserting the
   returned isFlagged each time -- the property a toggle endpoint could not
   give.
7. rules/test parity through HTTP: same seeded DB, response equals
   serde_json::to_value(rules::test_pattern(...)).
8. Locked 423: table-driven over every route added here (method + path +
   minimal body) against an encrypted, un-unlocked DB, so a future write route
   mounted outside data_router() fails this test. Runs with 31.4's encrypted-DB
   helper under --test-threads=1 (the password global).
9. Envelope invariant: every failure body in the tests above parses as
   {error:{code,message,...}} -- asserted through a small helper rather than
   ad hoc, which is what justifies ApiJson/ApiPath.

## 8. Verification matrix

1. cargo fmt --check
2. cargo build
3. cargo test
4. cargo test --no-default-features
5. cargo test --no-default-features --features serve
6. cargo clippy --all-targets -- -D warnings
7. cargo clippy --no-default-features --all-targets -- -D warnings
   -- tolerating exactly the 2 known task-34 lints (cli/dashboard.rs:852,
   cli/report/mod.rs:160) and nothing else
8. Manual smoke: cargo run -- serve --no-open against a demo database, then
   curl each write route with the session cookie -- a create/update/delete
   cycle per resource, one 409 per reason code, one 404, one 400 -- confirming
   the envelope shape and that `nigel rules list` / the TUI managers show the
   same rows afterwards.
<!-- SECTION:PLAN:END -->
