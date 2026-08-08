# Task 68.5 — A/R aging report view and dashboard summary

Parent: TASK-68 (Invoicing management surface). Sibling: 68.4 owns the Clients
and Invoices manager screens; this task owns the *reporting* half — aging as a
browsable report, and A/R visible on the dashboard home screen.

## The problem

`nigel invoice aging` prints five bucket totals with `println!` and nothing
else. There is no way to see *which* invoices make up a bucket, no way to reach
aging from the dashboard, no export of any kind, and the number never appears
on the home screen — so money owed is invisible unless you remember to ask a
specific subcommand for it.

## Decision: aging becomes a full `ReportKind`

`ReportKind` is the report vocabulary — one variant per report, `as_str()` the
slug, `granularity()` the single mapping to date navigation. Aging is a report.
Making it a bespoke dashboard-only view would mean a special case in
`enter_report_view_with_date`'s index table, no `nigel report aging`, and a
second definition of "what an aging report is" living next to the first.

Adding the variant is also what makes the work *finite*: `ReportCommands::kind`,
`output_args`, `report::dispatch_text` and `export::dispatch_pdf` are exhaustive
matches, so the compiler names every surface that must answer.

### In scope

| Surface | Why |
| --- | --- |
| `ReportKind::Aging`, slug `aging`, `DateGranularity::None` | the vocabulary entry |
| `ReportCommands::Aging { output }` → `nigel report aging` | clap parity with every other report |
| `view::build_aging` — interactive `TableReportView` | AC #1 |
| `text::format_aging` / `text::aging(today)` | **mandatory**: `dispatch_text` is exhaustive, and non-TTY `nigel report aging \| cat` routes through it |
| `pdf::render_aging` + `export::aging` | **mandatory-ish**: `dispatch_pdf` is exhaustive and `--mode export` defaults to PDF; erroring there would be a hole the user can walk into |
| `report all` includes aging | `balance` and `flagged` are equally date-less and are already in the bulk export |
| Dashboard report picker (view + export) entries | AC #1 — "alongside the existing reports" |
| Dashboard home A/R line | AC #2 |
| `nigel invoice aging` delegates to `text::aging` | one definition of the report |

### Out of scope — deliberately

- **`GET /api/reports/aging` and `/api/exports/aging`.** They belong to 68.6
  ("Web UI: invoicing endpoints and screens at full parity"), together with the
  SPA catalog entry in `reports-data.ts`, the `wc-` components, and the
  `fixture_capture.rs` capture row. Nothing breaks by deferring: `ParamSpec::for_kind`
  has a `_ => spec` catch-all, so the new variant compiles against the server
  untouched, and no test enumerates "every `ReportKind` has a route".
  The result is a knowingly asymmetric build for one task: CLI/TUI have aging,
  the web does not. That asymmetry is the epic's shape (CLI → TUI → web), not an
  accident.
- **An `--as-of` date parameter.** Aging is as-of-today by definition here;
  see open questions.
- **Bucket boundary changes.** `days = today − (due_date ?? issue_date)`;
  `≤0` current, `≤30`, `≤60`, `≤90`, else `90+`; only `sent`/`partial`/`overdue`;
  rows owing `≤ 0` skipped. Unchanged, and the existing test pins it.
- Clients/Invoices manager screens (68.4).

## Data layer

`invoicing::invoices::ar_aging` returns `Vec<AgingBucket>` — five labels and five
totals. A five-row table with a scroll hint is not a browsable report, so the
view needs the invoices behind the buckets.

Add to `src/invoicing/invoices.rs`:

```rust
pub struct AgingBucket { pub label: &'static str, pub count: usize, pub total: f64 }

pub struct AgingInvoice {
    pub number: i64,
    pub client: String,
    pub due_date: String,     // COALESCE(due_date, issue_date) — the date the bucket used
    pub days_past_due: i64,   // ≤ 0 means not yet due
    pub bucket: &'static str,
    pub total: f64,
    pub paid: f64,
    pub balance: f64,
}

pub struct AgingReport {
    pub as_of: String,
    pub buckets: Vec<AgingBucket>,
    pub invoices: Vec<AgingInvoice>,   // oldest first (days_past_due DESC)
    pub outstanding: f64,
}

pub fn ar_aging_detail(conn: &Connection, today: &str) -> Result<AgingReport>;
```

`ar_aging` is re-expressed as `ar_aging_detail(conn, today).map(|r| r.buckets)`,
so the bucketing arithmetic exists once and the existing `ar_aging` test keeps
passing untouched. `count` on `AgingBucket` is additive; nothing reads the struct
positionally today.

Derive `Serialize` with `#[serde(rename_all = "camelCase")]` on all three, matching
the `reports.rs` convention. Not needed by this task — needed by 68.6, and free now.

## The view

`DateGranularity::None`, so it follows `build_balance` / `build_flagged` exactly:
construct with `TableReportView::new(...)` and **do not** call `.with_date(...)`.
The consequences are all the ones we want, and all already implemented:

- `TableReportView::new` seeds `granularity: DateGranularity::None`.
- `handle_key`'s Left/Right arms are guarded by `granularity != None` and `m` by
  `granularity == MonthAndYear`, so all three keys are inert. No `Reload` can be
  emitted, so no dashboard reload wiring is needed.
- `period_label()` returns `""` and the footer's `nav_hint` is `""`.
- `date_params()` returns `(None, None)`, so the dashboard export path passes no
  date to `do_export` / `do_text_export` — correct, because aging has none.

The as-of date rides in the title instead, the way `build_flagged` puts its count
there: `format!("A/R Aging — as of {}", data.as_of)`.

### Wireframe

Five columns: `["Invoice", "Client", "Due", "Days", "Balance"]`, widths
`[Fill(1), Length(22), Length(12), Length(6), Length(14)]`. Bucket rows put their
label in column 0 and their amount in column 4, leaving the middle blank — the
same shape `build_k1` uses for section lines.

```
 A/R Aging — as of 2026-08-07
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 Invoice        Client                  Due           Days      Balance

 SUMMARY
   Current (2)                                                $4,200.00
   1-30 (1)                                                   $1,500.00
   31-60 (0)                                                      $0.00
   61-90 (1)                                                  $3,200.00
   90+ (0)                                                        $0.00
   Total Outstanding (4)                                      $8,900.00

 OPEN INVOICES
 #1244          Initech                 2026-05-30      69     $3,200.00
 #1249          Globex                  2026-07-20      18     $1,500.00
 #1251          Acme Co                 2026-08-31      —      $4,200.00

 ↑/↓=scroll  q/Esc=close  line 1/14
```

- `SUMMARY` / `OPEN INVOICES` use the existing `section_row` (yellow bold).
- Bucket counts ride in the label as `(n)` rather than taking a column — the
  parenthetical count is already the house style (`Flagged Transactions (3)`).
- An aged bucket with a non-zero balance renders its **label** yellow. Amounts
  stay green everywhere: A/R is money coming in, and `money_span` would have to
  be lied to (`money_cell(-x)`) to print red. Lateness is the `Days` column's job.
- `Days` prints `—` when `days_past_due ≤ 0`.
- Invoices sort oldest-first, so the row that needs a phone call is at the top of
  the section.
- Empty state: a single `No open invoices.` row in the description column,
  matching `build_flagged`.

## Reconciling `nigel invoice aging`

**Both commands survive.** `invoice aging` is documented in `docs/invoicing.md`,
sits with the other invoice verbs where someone looking at A/R will actually
look, and removing it would be a breaking CLI change inside an epic whose whole
point is *adding* surface.

It **delegates**: `cli::invoice::aging(today)` becomes

```rust
println!("{}", crate::cli::report::text::aging(today)?);
```

so the two can never disagree about a number or a label. Visible change: the
comfy-table layout replaces `{:>8}: {:.2}`, plus the company-name header
`with_header` adds. That is an improvement, and `docs/invoicing.md`'s sample
output gets updated with it.

It deliberately does **not** become an alias for `report::dispatch` — that is
TTY-aware and would launch a ratatui view from a command that has always been a
one-shot print, breaking every script and the doc's own walkthrough.

## Dashboard home A/R line

The left stats column is a fixed `Constraint::Length(5)` holding YTD Income /
YTD Expenses / Net Profit / Transactions / Flagged. A/R gets **one** line
appended, and the constraint becomes `Length(if has_ar { 6 } else { 5 })`, so a
database with no open invoices renders byte-identically to today (this is most
databases — `nigel demo` creates no invoices).

Label field width is 15 (`" YTD Income     "`), and `A/R Outstanding` is exactly
15 characters, so it aligns with no padding fiddling.

```
 Hello, Dalton. Books won't balance themselves.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 YTD Income     $184,200.00       Account Balances
 YTD Expenses   $121,455.00       BofA Checking          $42,118.02
 Net Profit      $62,745.00       BofA Credit Card       -$1,204.55
 Transactions   1,284             BofA Line of Credit         $0.00
 Flagged        3
 A/R Outstanding  $8,900.00  oldest 61-90        ← new; hidden when no open A/R
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 Monthly Cash Flow 2025 - 26      Top Expenses (3 months)
 ...
```

- Amount via `money_span(outstanding)` → green.
- Suffix `  oldest <label>` — `FOOTER_STYLE` (dark gray) when the oldest
  non-empty bucket is `current`, yellow otherwise. "Oldest" is the *last*
  bucket in the array with `total > 0.005`, since the array runs current → 90+.
- Loading is **non-fatal**: `HomeData` gains
  `ar: Option<ArSummary { outstanding, oldest_bucket, oldest_total }>`, filled by
  `ar_aging_detail(conn, &today).ok().filter(|r| r.outstanding >= 0.005)`.
  A `?` here would let an invoicing hiccup blank the entire dashboard, and the
  `.ok()` and the "only when open invoices exist" gate collapse into one
  expression. Half-cent slack matches the invoicing convention everywhere else.

## Dashboard picker index tables — the fragile part

`REPORT_TYPES`, `EXPORT_TYPES`, `enter_report_view_with_date`, `do_export`,
`do_text_export` and `do_text_export`'s `names` array are six parallel lists
keyed by bare `usize`. Aging goes in at **index 8** in both pickers, which pushes
`"All Reports"` from 8 to 9 in `EXPORT_TYPES` — so the `if idx == 8` bulk-export
branches in `do_export` and `do_text_export` must become `idx == 9`.

A refactor to a single index→`ReportKind` table is tempting and is **not** part
of this task. Instead, add a guard test asserting the picker labels and the slug
order line up, so the next insertion fails loudly instead of silently exporting
the wrong report.

## Tests

- `invoices.rs`: `ar_aging_detail` bucket assignment at each boundary (0/1/30/31/60/61/90/91
  days), `due_date` falling back to `issue_date`, partial payments reducing the
  balance, paid-in-full and void invoices excluded, counts matching the invoice
  list, `outstanding` equal to the bucket sum, oldest-first ordering.
- `reports.rs`: extend `report_kind_slugs_and_granularity` with `(Aging, "aging", None)`.
- `text.rs`: `format_aging` over a fixture — every bucket present even at zero,
  totals row, empty-invoice state.
- `view.rs`: `build_aging` returns a view whose `date_params()` is `(None, None)`
  and whose Left/Right/`m` keys return `Continue`, never `Reload`.
- `dashboard.rs`: picker index guard test; `ArSummary` derivation (oldest bucket
  selection, the `< 0.005` suppression).
- `tests/cli_dispatch.rs`: `nigel report aging --mode export --format text`
  writes a non-empty file; `nigel invoice aging` still prints the bucket labels;
  `report all --format text` now writes nine files.

## Docs (Documentation Policy — not optional)

- `CLAUDE.md`: Reports architecture bullet (`ReportKind` gains `Aging`), the
  Invoicing bullet (`ar_aging_detail`), the Commands block (`nigel report aging`),
  and a Key Design Constraint recording that aging has no date parameters and
  that `invoice aging` prints the report module's own text.
- `README.md`: report list.
- `docs/invoicing.md`: the A/R aging section gains `nigel report aging` and the
  new sample output.

## Open questions for the orchestrator

1. **Is deferring the web endpoints acceptable?** This ships a `ReportKind` the
   HTTP API does not serve, leaving `/api/reports/aging` a 404 until 68.6. The
   alternative is pulling roughly 80 lines of route + export handler forward into
   a task labelled "TUI". Recommendation: defer, and note it on 68.6.
2. **`report all` — include aging or not?** Recommended yes (consistent with
   `balance`/`flagged`), but it changes the file count of an existing command
   from eight to nine, which an integration test and anyone's scripts may notice.
3. **`nigel invoice aging` output format change.** Delegating swaps the terse
   `current: 4200.00` lines for a comfy-table with a company-name header. Better,
   but it is a user-visible change to a shipped command. Accept, or keep the old
   printer and accept the duplication?
4. **One dashboard line or two?** The spec takes one line (total + oldest bucket)
   to keep `stats_area` at six rows. Two lines would let the oldest bucket carry
   its own amount, at the cost of another row stolen from the chart area on short
   terminals.
5. **Should the dashboard A/R line be actionable** — e.g. a `[?]` shortcut jumping
   straight into the aging view? 68.4 is adding invoicing shortcuts to the same
   menu; whoever lands second should own the key assignment so the two do not
   collide.
6. **`--as-of` date.** Aging as-of a past date is a real accounting need (period-end
   A/R). Adding it later means moving `Aging` off `DateGranularity::None`, which
   changes the view's key handling. Confirm we are content to defer, or add the
   flag now while the surface is being built.
