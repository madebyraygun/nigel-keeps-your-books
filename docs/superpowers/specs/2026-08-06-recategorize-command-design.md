# `nigel recategorize` — bulk category reassignment

## Problem

There is no non-interactive way to change the category of an already-categorized
transaction. `categorize` only touches uncategorized rows; `review` and the register
browser are TUIs. Bulk cleanups (e.g. draining a category at year-end, moving
tangible-property purchases to a Section 179 category) currently require driving a
TUI by hand, once per transaction.

## Command

```
nigel recategorize [IDS...] --category <NAME> [FILTERS] [--dry-run] [--yes]
```

Two selection modes, mutually exclusive (supplying both IDs and any filter is an error):

- **Explicit IDs**: positional transaction IDs. Any unknown ID is a hard error and
  nothing is written. Writes immediately, no confirmation.
- **Filters** (all optional, ANDed together):
  - `--from-category <NAME>` — current category, exact name match
  - `--uncategorized` — rows with no category (mutually exclusive with `--from-category`)
  - `--year <YYYY>` / `--month <YYYY-MM>` / `--from <YYYY-MM-DD>` / `--to <YYYY-MM-DD>`
    (same date semantics as reports; `--year`/`--month` exclusive with `--from`/`--to`)
  - `--pattern <PAT>` with `--match-type contains|starts_with|regex` (default contains),
    matched against description, same semantics as rules
  - `--account <NAME>` — account name, exact match
  - `--min-amount <N>` / `--max-amount <N>` — compared against the absolute amount
  - Filter mode with no filters at all is an error (prevents accidental
    whole-database moves).

`--category` is required: the target category, exact name match against active
categories. Unknown name → error listing available categories (existing
`CategoryNotFound`-style message).

## Behavior

- Every run prints the matched transactions as a table (ID, date, description, amount,
  current category → target) plus a count and total, then:
  - `--dry-run`: stop. Nothing written. (Valid in both selection modes.)
  - ID mode: apply immediately.
  - Filter mode: require `--yes`, or an interactive `y/N` prompt when stdin is a TTY.
    Without a TTY and without `--yes`, error out with a hint to pass `--yes`.
- Zero matches in filter mode: print "No transactions matched." and exit 0.
- Applying mirrors an in-app review (`reviewer.rs`): set `category_id`, clear
  `is_flagged` and `flag_reason`. Vendor is untouched. All rows update inside a single
  SQL transaction — all or nothing.
- Rows already in the target category are skipped from the update (reported as
  "already in <category>" in the output) so the summary count reflects real changes.

## Implementation

- `src/cli/mod.rs`: new top-level `Recategorize` variant (clap), styled after
  `Categorize`/`Rules`.
- `src/cli/recategorize.rs`: arg struct, validation, table output, confirmation, run().
- `src/reviewer.rs` (data layer, keeps `&Connection`-in/plain-structs-out convention):
  - `find_transactions_for_recategorize(conn, &RecategorizeFilter) -> Vec<TxnRow>` —
    builds a parameterized WHERE clause; regex matching filters in Rust (SQLite has no
    regex function loaded), consistent with how the categorizer applies regex rules.
  - `recategorize_transactions(conn, &[i64], category_id) -> Result<usize>` — single
    transaction, `UPDATE ... SET category_id=?, is_flagged=0, flag_reason=NULL`.

## Testing

- Unit tests (reviewer.rs): filter matching (each filter + combinations), batch update
  clears flags, skip-already-in-target, unknown ID aborts whole batch.
- Integration tests (tests/cli_dispatch.rs pattern): drive the real binary —
  ID mode end-to-end, filter mode with `--yes`, `--dry-run` writes nothing,
  no-TTY-no-`--yes` fails fast, works against an encrypted DB via `NIGEL_DB_PASSWORD`.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.

## Out of scope

- Setting vendor, creating rules, or touching the rules engine.
- Combining IDs with filters in one invocation.
- Undo (use `nigel backup` before bulk moves; documented in README).
