# Task 68.4 — TUI client and invoice management screens

Two new inline dashboard screens following the manager-screen pattern
(`account_manager.rs`, `category_manager.rs`, `rules_manager.rs`,
`reconcile_manager.rs`):

- `src/cli/client_manager.rs` — Clients: list, add, edit.
- `src/cli/invoice_manager.rs` — Invoices: scrollable list, detail view with
  line items / payments / balance, and three actions (send, record payment,
  void) with confirmations.

Both are reached from the dashboard command chooser.

## Dashboard shortcuts

Taken today: `b i r c a t u z v e l p s`, plus `q` (quit) and `F5` (refresh).

| Letter | Item | Mnemonic |
|---|---|---|
| `n` | `[n] Invoices` | i**N**voices — same second-consonant convention as `t`=ca**T**egories, `u`=r**U**les |
| `k` | `[k] Clients` | the /k/ sound **cl**ients opens with; `c` belongs to Reconcile |

`k` is the weaker of the two. Alternatives, all free: `o` (inv**O**ices),
`m`, `w`, `d`, `f`, `g`, `h`, `j`, `x`, `y`. See open questions.

`MENU_ITEMS` grows from 13 to 15 entries. **`MENU_LEFT_COUNT` must go from 7 to
8** or the last right-column entry is clipped: the menu block is
`MENU_LEFT_COUNT + 1` rows tall, one of which is the "What would you like to
do?" title, so the right column can only ever show `MENU_LEFT_COUNT` items.
With 15 items and `MENU_LEFT_COUNT = 7` the right column needs 8 rows and has 7.

New order (indices are what `activate_menu_item` matches on, so every arm after
7 shifts by two):

```
left  (0-7)   b Browse · i Import · r Review · c Reconcile ·
              a Accounts · t Categories · u Rules · z Undo
right (8-14)  n Invoices · k Clients · v View report · e Export report ·
              l Load · p Settings · s Snake
```

`menu_item_line`'s `if i == 2` flagged-count special case still points at
Review and does not move.

## Data layer this task consumes

### From task 68.1 — reconciled against its landed spec

`docs/superpowers/specs/2026-08-08-task-68-1-invoicing-edit-void-design.md`
landed while this was being written. Four things it defines are consumed here,
at their real signatures:

```rust
// src/invoicing/clients.rs
#[derive(Debug, Default, Clone)]
pub struct ClientUpdate {
    pub name: Option<String>,                 // NOT NULL — cannot be cleared
    pub email: Option<Option<String>>,        // Some(None) clears
    pub billing_address: Option<Option<String>>,
    pub notes: Option<Option<String>>,
}
pub fn update_client(conn: &Connection, id: i64, update: &ClientUpdate) -> Result<()>;

// src/invoicing/invoices.rs
/// Writes `voided_at`; `refresh_status` derives the `void` status from it.
pub fn void_invoice(conn: &Connection, invoice_id: i64, voided_on: &str) -> Result<()>;
/// Not already void, no recorded payments. The TUI's pre-flight.
pub fn ensure_voidable(conn: &Connection, invoice: &Invoice) -> Result<()>;
/// `YYYY-MM-DD` or `Invalid("Invalid <what>: <value> (expected YYYY-MM-DD)")`.
pub fn validate_date(value: &str, what: &str) -> Result<()>;
```

Consequences for this design, all improvements on what was assumed:

- The edit form always holds every current value, so it sends every field:
  `name: Some(v)`, and `Some(None)` / `Some(Some(v))` for each optional one
  depending on whether the field was left blank. `ClientUpdate::is_empty()`
  therefore never fires, and the blank-name check lands in the data layer with
  the message this spec had already chosen independently — `Name is required`.
- `void_invoice` takes the void date. The TUI passes today as `%Y-%m-%d`.
- `ensure_voidable` **is** the pre-flight guard S10 wanted; no guard logic is
  re-derived in the screen.
- `validate_date` removes what was going to be a TUI-only date rule. The pay
  form calls `validate_date(&date, "payment date")` and renders its message.
- 68.1 adds a `voided_at` column (migration v5) and `Invoice.voided_at`, so the
  detail view can print `Voided  2026-08-07` for a void invoice.
- `get_client` now reports a missing row as `NotFound` (`Client not found: id 99`)
  rather than letting `QueryReturnedNoRows` escape as `Database error: …`.

Guard sentences the screens render verbatim, from 68.1's wording table:

| Condition | Message |
|---|---|
| Void, send | `Invoice #1254 is void and cannot be sent.` |
| Void, pay | `Invoice #1254 is void and cannot be paid.` |
| Already void, void | `Invoice #1254 is already void.` |
| Payments recorded, void | `Invoice #1254 has 1250.00 in recorded payments and cannot be voided.` |

68.1 routes the void guard through `NigelError::Conflict { code, message }`, and
`Display` is the message alone, so these read identically in the CLI and the TUI.
Whichever function 68.1 leaves as the void-for-send/pay guard — the existing
`cli::invoice::ensure_not_void` or a relocated equivalent — is what the screens
call; they never re-derive it.

Invoice **editing** is deliberately not consumed. Task 68.4's scope is client
list/add/edit plus invoice list/detail/send/pay/void; a draft-invoice edit form
is not in it, so `InvoiceUpdate` / `update_invoice` / `ensure_editable` never
appear here.

68.1's `client_summary` / `ClientSummary` is also not consumed — it is per-id,
so a list of them would be N+1. See open question 6.

### New in this task

Two read-only additions to `src/invoicing/invoices.rs`, because the screens need
figures the existing surface cannot produce:

```rust
pub struct InvoiceListRow {
    pub id: i64,
    pub number: i64,
    pub status: String,
    pub client_name: String,
    pub total: f64,
    pub paid: f64,
    pub due_date: Option<String>,
}

/// Newest first, one query — the list needs a balance per row and
/// `paid_amount` per row would be N+1.
pub fn list_invoices(conn: &Connection) -> Result<Vec<InvoiceListRow>>;

/// Payment history for the detail view, oldest first.
pub fn payments(conn: &Connection, invoice_id: i64) -> Result<Vec<InvoicePayment>>;
```

`models::InvoicePayment` already exists and is unused; this is what starts using
it.

### Reused as-is

`clients::{list_clients, add_client, get_client}`,
`invoices::{get_invoice, get_invoice_by_number, line_items, paid_amount,
record_payment}`, `send::send_invoice`, `settings::invoicing_config`.

Three private helpers in `src/cli/invoice.rs` become `pub(crate)` with no
behaviour change, so the TUI enforces the same guards in the same words:

- `ensure_not_void(&Invoice, action: &str) -> Result<()>`
- `build_clients(InvoicingConfig) -> Result<(StripeClient, R2Publisher, MailgunClient)>`
- `payment_amount(&Invoice, paid: f64, requested: Option<f64>) -> Result<f64>`

## Money and status rendering

Amounts use `fmt::money` (`$2,000.00`), not the CLI's bare `{:.2}` and not
`tui::money_span`. `money_span` prints the absolute value and lets colour carry
the sign — every figure on these screens is a positive receivable, so a
sign-derived colour would be noise. Colour carries **status** instead:

| Status | Style |
|---|---|
| `draft` | `Color::DarkGray` |
| `sent` | `Color::Cyan` |
| `partial` | `Color::Yellow` |
| `paid` | `tui::GREEN` |
| `overdue` | `Color::Red` |
| `void` | `Color::DarkGray` |

An absent value (no email, no due date, no address) renders as `—`, never as an
invented empty string or `$0.00`.

The list shows the **stored** status, exactly as `nigel invoice list` does. It
does not call `refresh_status` on load, so an invoice that crossed its due date
since the last write still reads `sent` rather than `overdue`. Opening a screen
should not rewrite rows. See open questions — task 68.5 (aging view) is where
this starts to matter.

---

# Clients screen

`src/cli/client_manager.rs`

```rust
pub enum ClientAction { Continue, Close }

enum Screen {
    List,
    Add(ClientForm),
    Edit(ClientForm),
}

pub struct ClientManager {
    clients: Vec<Client>,
    selection: usize,
    scroll_offset: usize,
    last_visible_rows: usize,
    screen: Screen,
    status_message: Option<String>,
    status_ttl: u8,      // 3 keypresses, category_manager's TTL
    greeting: String,
}
```

`ClientManager::new(conn, greeting)` is infallible and swallows a load error
into an empty list, matching `CategoryManager::new` / `RulesManager::new`.

No delete. The data layer has no `delete_client`, a client with invoices must
not vanish under them, and 68.1 does not add one.

## S1 — Client list

```
 Hello, Dalton. Kettle's on.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 Clients (4)

   Name                         Email                        Billing address
 > Acme Co                      ap@acme.test                 1 Main St, Portla…
   Blackwood & Sons             billing@blackwood.test       —
   Cedar Systems                —                            88 Cedar Way, Ben…
   Dovetail Studio              hello@dovetail.test          —




 a=add  e=edit  Esc=back  q=quit
```

Columns: marker 3, Name 28 (truncate at 26 + `…`), Email 28 (truncate 26),
Billing address fills the rest (truncate to width). Sorted by name — that is
`list_clients`' `ORDER BY name`, not a choice this screen makes.

Keys: `Up`/`Down` move (with `ensure_visible` scrolling), `a` add, `e` edit,
`Esc`/`q` close.

Empty:

```
 Clients (0)

   No clients yet. Press 'a' to add one.
```

## S2 — Add client

```
 Hello, Dalton. Kettle's on.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 Add Client

   Name           Acme Co_
   Email
   Address
   Notes

   Email is the address `send` mails the invoice to.

 Tab=next field  Enter=save  Esc=cancel
```

Four `FieldKind::Text` fields, `category_manager`'s `FormField` shape. The hint
line under the form is static and dim (`FOOTER_STYLE`); it is replaced by the
status message when one is set.

Keys: `Tab`/`Down` next field, `BackTab`/`Up` previous, printable chars append,
`Backspace` deletes, `Enter` saves, `Esc` cancels back to S1.

Validation on `Enter`:

| Field | Rule | Message |
|---|---|---|
| Name | trimmed, non-empty | `Name is required` |
| Email | trimmed; empty → `None` | none |
| Address | trimmed; empty → `None` | none |
| Notes | trimmed; empty → `None` | none |

Email is **not** validated for shape. `nigel client add` does not validate it
either, and the only thing that cares is `send_invoice`, which fails by name
(`client 'Acme Co' has no email`) when it is absent. Inventing a TUI-only email
rule would let the two front ends disagree about what a client is.

Failure state (the form stays open, message under the fields in yellow, exactly
`category_manager`'s behaviour):

```
 Add Client

   Name           _
   Email          ap@acme.test
   Address
   Notes

   Name is required
```

Success: reload, back to S1, status line `Added client: Acme Co`.

## S3 — Edit client

Identical layout, titled `Edit Client`, fields prefilled from the selected row.
Saves through `clients::update_client`. Success: reload, back to S1, status line
`Updated client: Acme Co`. A data-layer error renders as `e.to_string()` in the
same yellow line — the screen never re-words a `NigelError`.

```
 Edit Client

   Name           Cedar Systems
   Email          ops@cedar.test_
   Address        88 Cedar Way, Bend OR
   Notes          Net 30, PO required

   Email is the address `send` mails the invoice to.

 Tab=next field  Enter=save  Esc=cancel
```

---

# Invoices screen

`src/cli/invoice_manager.rs`

```rust
pub enum InvoiceAction {
    Continue,
    Close,
    /// The screen has entered a blocking state and needs the controller to
    /// paint it before the work runs. See "Sending" below.
    Perform,
}

enum Screen {
    List,
    Detail,
    ConfirmSend,
    Sending,
    ActionResult { title: String, lines: Vec<String>, is_error: bool },
    PayForm(PayForm),
    ConfirmVoid,
}

struct Detail {
    invoice: Invoice,
    client: Client,
    items: Vec<InvoiceLineItem>,
    payments: Vec<InvoicePayment>,
    paid: f64,
}
```

`Detail` is loaded once on entry and reloaded after every mutation (payment,
void, send). `detail_scroll: usize` scrolls it.

**Actions live on the detail view only.** `Enter` on the list opens the
invoice; `s`, `p` and `v` are bound there and nowhere else. Sending emails a
client and voiding is terminal — both deserve looking at the invoice first, and
the list's job is to find it. This also halves the state matrix. See open
questions for the alternative.

## S4 — Invoice list

```
 Hello, Dalton. Kettle's on.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 Invoices (6)

   #      Status   Client                       Total      Balance  Due
 > 1256   draft    Acme Co                  $1,250.00    $1,250.00  —
   1255   overdue  Blackwood & Sons         $3,400.00    $3,400.00  2026-07-05
   1254   partial  Cedar Systems            $2,000.00      $750.00  2026-08-15
   1253   paid     Dovetail Studio            $900.00        $0.00  2026-07-20
   1252   void     Acme Co                    $480.00        $0.00  2026-06-30
   1251   sent     Acme Co                  $1,100.00    $1,100.00  2026-08-22


 Enter=open  Esc=back  q=quit
```

Column budget, exactly 80 columns: marker 3, `#` 6, gap 1, Status 8, gap 1,
Client 24 (truncate 22), gap 1, Total 12 right-aligned, gap 1, Balance 12
right-aligned, gap 1, Due 10. Only the Status cell is coloured.

Newest first (`ORDER BY number DESC`) — `nigel invoice list`'s order.

Keys: `Up`/`Down`/`PageUp`/`PageDown`/`Home`/`End` move, `Enter` opens the
selected invoice, `Esc`/`q` close.

Empty:

```
 Invoices (0)

   No invoices yet. Draft one with `nigel invoice new` — the dashboard
   cannot create invoices yet.
```

That sentence points at a terminal command from inside a terminal UI, which is
tolerable, but the underlying gap is real and unassigned. See open questions.

## S5 — Invoice detail

```
 Hello, Dalton. Kettle's on.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 Invoice #1254   partial

   Client    Cedar Systems
   Email     ops@cedar.test
   Issued    2026-07-16       Due  2026-08-15       Currency  USD

   Description                                  Qty        Unit       Amount
   Strategy workshop                           1.00    1,200.00     1,200.00
   Implementation support                      8.00      100.00       800.00
                                                            ─────────────────
                                               Subtotal        $2,000.00
                                               Tax                 $0.00
                                               Total          $2,000.00

   Payments
   2026-08-01   ach                                          $1,250.00
                                               Paid          $1,250.00
                                               Balance         $750.00

   Pay link  https://buy.stripe.com/test_9AQ3cw0Xy1234567

 s=send  p=record payment  v=void  Up/Down=scroll  Esc=back  q=quit
```

The status word beside the number is coloured. `Payments` and its rows are
omitted entirely when there are none — no empty table:

```
 Invoice #1256   draft

   Client    Acme Co
   Email     —
   Issued    2026-08-06       Due  —                Currency  USD

   Description                                  Qty        Unit       Amount
   Retainer — August                           1.00    1,250.00     1,250.00
                                                            ─────────────────
                                               Subtotal        $1,250.00
                                               Tax                 $0.00
                                               Total          $1,250.00

                                               Paid                $0.00
                                               Balance         $1,250.00

 s=send  p=record payment  v=void  Up/Down=scroll  Esc=back  q=quit
```

`Pay link` appears only when `stripe_payment_link_url` is set. A void invoice
keeps the same layout; its footer drops the actions:

```
 Invoice #1252   void

   ...

 Up/Down=scroll  Esc=back  q=quit
```

Keys: `Up`/`Down`/`PageUp`/`PageDown` scroll, `s` send, `p` pay, `v` void,
`Esc`/`q` back to S4.

## S6 — Confirm send

The confirmation is appended inline at the bottom of the detail view and the
footer swaps to `y`/`n`, which is `category_manager`'s delete-confirmation
pattern. The invoice stays on screen while you decide.

Three guards run **before** the dialog opens, so the dialog never offers
something that is going to fail. Each failure sets the yellow status line and
leaves the screen on Detail:

| Guard | Source | Message |
|---|---|---|
| Void | `cli::invoice::ensure_not_void(inv, "sent")` | `Invoice #1252 is void and cannot be sent.` |
| No client email | checked here, worded as `send.rs` does | `client 'Acme Co' has no email` |
| Missing config | `cli::invoice::build_clients(invoicing_config())` | `missing invoicing config: r2_bucket (set it in settings.json or the matching NIGEL_ env var)` |

`build_clients` touches no network — it only reads settings and names the first
absent key — so running it at confirm time is free and moves a multi-second
failure to an instant one.

First send:

```
 Invoice #1254   partial

   ... detail as above ...

   Send invoice #1254 to ops@cedar.test?
   Cedar Systems · $2,000.00. Creates a Stripe payment link, publishes the
   page and PDF, then emails the client.

 y=send  n=cancel
```

Re-send (`published_at.is_some()`):

```
   Re-send invoice #1254 to ops@cedar.test?
   Published 2026-07-16. The existing payment link is reused; the page and
   PDF are republished and the client is emailed again.

 y=send  n=cancel
```

`n` or `Esc` returns to Detail unchanged. `y` moves to S7.

## S7 — Sending

`send_invoice` is a multi-second, blocking, three-hop network call (Stripe → R2
→ Mailgun). The dashboard loop is `terminal.draw()` → blocking `event::read()`
→ `handle_key()`, so anything `handle_key` does happens *after* the last paint:
naively calling `send_invoice` from the `y` handler freezes the screen showing
the confirmation dialog, as if the keypress had been dropped.

Two ways out were considered.

**A thread plus a poll loop** would give a real animated spinner. It also needs
a second `rusqlite::Connection` (a `Connection` is `!Sync` and `send_invoice`
takes `&Connection`), which means a second handle on a possibly-SQLCipher
database and a second writer racing the main one for the `mark_published`
UPDATE. That is a lot of machinery, and a new class of `SQLITE_BUSY` bug, for
one button.

**Draw-then-block** is what this spec picks, and it matches how
`import_manager.rs` already handles a long import — it just does the work on the
main thread. The difference is that the frame the user is left staring at is
chosen deliberately rather than inherited:

1. `handle_key` sets `screen = Screen::Sending` and returns
   `InvoiceAction::Perform`.
2. The dashboard, after the borrow ends, calls `terminal.draw(...)` a second
   time — painting S7 — and then `mgr.perform_pending(&conn)`.
3. `perform_pending` runs the send, reloads the detail, and sets
   `Screen::ActionResult`. The next loop iteration paints it.

There is no spinner: the frame is frozen for the duration. It says so.

```
 Hello, Dalton. Kettle's on.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 Sending invoice #1254

   Creating the Stripe payment link, publishing the page and PDF, and
   emailing ops@cedar.test.

   This can take a few seconds. Nigel is not reading keys until it finishes.

 Working…
```

After the send returns, `perform_pending` **drains buffered input** before
handing control back:

```rust
while crossterm::event::poll(std::time::Duration::ZERO).unwrap_or(false) {
    let _ = crossterm::event::read();
}
```

Without it, a user who mashed Enter while the terminal was unresponsive
dismisses the result screen before reading it.

## S8 — Send result

Success:

```
 Invoice #1254 sent

   https://billing.rygn.io/i/aB3xY7kQ9mZ1pR4t/
   Emailed to ops@cedar.test.

 Esc=back
```

Failure — the second line is derived from the reloaded row rather than assumed,
because a failed *first* send leaves a draft while a failed *re-send* leaves an
invoice that is still published:

```
 Send failed

   upload down
   Invoice #1254 is still partial. Nothing was published or emailed.

 Esc=back
```

The message is `e.to_string()` verbatim. `send_invoice` fails closed —
publishing before emailing, marking published last — so "nothing was published
or emailed" is true for every failure after the Stripe step. The Stripe payment
link, if it was created before the failure, *is* persisted and reused on the
next attempt; that is existing `send_invoice` behaviour and the screen does not
narrate it.

`Esc` (or any key) returns to Detail, which now shows the reloaded invoice.

## S9 — Record payment

Guarded before the form opens, via `ensure_not_void(inv, "paid")`:
`Invoice #1252 is void and cannot be paid.`

```
 Hello, Dalton. Kettle's on.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

 Record a Payment — invoice #1254

   Client     Cedar Systems
   Total      $2,000.00      Paid  $1,250.00      Balance  $750.00

   Amount   $ 750.00_
   Date       2026-08-07
   Method     < direct_deposit >

 Tab=next field  Left/Right=method  Enter=record  Esc=cancel
```

Fields:

| Field | Kind | Prefill | Accepts |
|---|---|---|---|
| Amount | Text | outstanding balance as `{:.2}`, or empty when the balance is ≤ 0 | digits, `.`, `,` |
| Date | Text | today, `%Y-%m-%d` | digits, `-` |
| Method | Selector | `direct_deposit` | — |

The method options are exactly `direct_deposit`, `ach`, `stripe`, `other`.
That is not a style choice: `invoice_payments.method` carries a
`CHECK (method IN ('stripe','ach','direct_deposit','other'))`, so a fifth
option would be a runtime constraint failure. The default matches
`nigel invoice pay`'s `default_value = "direct_deposit"`.

The Total / Paid / Balance line above the fields is what makes an overpayment
visible. Overpayment is **allowed** — `payment_amount` accepts it deliberately,
because a bank really does sometimes send more than the invoice.

Validation on `Enter`, in order:

| Condition | Message |
|---|---|
| Amount empty | `Amount is required` |
| Amount unparseable | `Amount must be a number` |
| Amount ≤ 0 or not finite | `Amount must be a finite number greater than zero.` |
| Date empty | `Date is required (YYYY-MM-DD)` |
| Date malformed | `invoices::validate_date(&date, "payment date")`'s message: `Invalid payment date: 2026-8-7 (expected YYYY-MM-DD)` |

One deliberate departure from `cli/invoice.rs`: the zero/negative message drops
the CLI's `--amount` prefix (`--amount must be a finite number greater than
zero, got 0.00.`). A flag name is useless beside a form field. This is the same
call the SPA's reconcile screen made about the CLI's "run `nigel accounts list`"
404.

The date **is** shape-checked, which `nigel invoice pay` does not do today —
but through 68.1's `validate_date`, not a rule invented here, so the two front
ends cannot disagree. It matters because the date is typed free-hand into a
prefilled field and a malformed one poisons `refresh_status` (ISO string
comparison against `due_date`) and `ar_aging` (`parse_from_str`, which falls
back to *today* on failure).

The amount field never reaches `payment_amount`'s `None` branch — it is
prefilled and required — so the CLI's "no outstanding balance, pass `--amount`
to record a payment anyway" sentence never renders. A settled invoice opens the
form with an empty Amount and the balance line reading `$0.00`; typing a number
records it.

Failure renders under the form in yellow:

```
   Amount   $ 0_
   Date       2026-08-07
   Method     < direct_deposit >

   Amount must be a finite number greater than zero.
```

Success: `record_payment` → reload detail → back to S5, status line
`Recorded $750.00 against invoice #1254 (paid).` — the shape of the CLI's
`Recorded 750.00 against invoice #1254 (paid)`, with `fmt::money`.

## S10 — Confirm void

Guarded by 68.1's `invoices::ensure_voidable(conn, &invoice)`, run as a
pre-flight so the dialog is never offered for an invoice that will reject it.
Failures land in the yellow status line on Detail:

- `Invoice #1252 is already void.`
- `Invoice #1254 has 1250.00 in recorded payments and cannot be voided.`

Inline on the detail view, same shape as S6:

```
 Invoice #1256   draft

   ... detail as above ...

   Void invoice #1256 for Acme Co ($1,250.00)?
   Void is permanent. A void invoice can never be sent or paid.

 y=void  n=cancel
```

`y` → `void_invoice(conn, id, &today)` (today as `%Y-%m-%d`, which becomes
`voided_at`) → reload → stay on Detail, now showing `void`, a `Voided 2026-08-07`
line, and a footer without actions. Status line: `Voided invoice #1256.` A
failure sets the status line to `e.to_string()` and stays on Detail.

`n` or `Esc` → Detail unchanged.

## Dashboard wiring

Two new `DashboardScreen` variants, two `draw` arms, two `handle_key` arms —
all copies of the existing manager arms — plus one new thing, the `Perform`
seam:

```rust
// alongside `pending_reload`, a loop-local, not a Dashboard field
let mut pending_invoice_work = false;

DashboardScreen::Invoices(ref mut mgr) => {
    match mgr.handle_key(key.code, &conn) {
        InvoiceAction::Close => return_home = true,
        InvoiceAction::Continue => {}
        InvoiceAction::Perform => pending_invoice_work = true,
    }
    false
}

// after the borrow ends, beside the `pending_reload` block
if pending_invoice_work {
    let _ = terminal.draw(|frame| dashboard.draw(frame)); // paint S7
    if let DashboardScreen::Invoices(ref mut mgr) = dashboard.screen {
        mgr.perform_pending(&conn);
    }
}
```

`ClientManager` needs no such seam — `ClientAction` is `Continue`/`Close`, like
every other manager.

Returning home from either screen runs the existing `load_data(&conn)` refresh.
Neither screen touches the transaction register, so the home figures do not
move; the refresh is free and keeps the arm identical to its siblings.

## Testability seam for send

`perform_pending` builds the real Stripe/R2/Mailgun clients and delegates to a
generic function, so tests drive the whole flow with the fakes without a socket:

```rust
impl InvoiceManager {
    /// Real clients from settings. Called only by the dashboard.
    pub fn perform_pending(&mut self, conn: &Connection) { /* build + delegate */ }

    /// The half that is testable.
    pub(crate) fn perform_send<G: PaymentGateway, P: AssetPublisher, M: Mailer>(
        &mut self,
        conn: &Connection,
        today: &str,
        contact_email: &str,
        gateway: &G,
        publisher: &P,
        mailer: &M,
    );
}
```

Sends need a real PDF to publish and attach, so `send_invoice` only succeeds in
a `pdf` build. Every send test is gated `#[cfg(all(test, feature = "pdf"))]`,
the same gate `src/invoicing/send.rs` already uses and for the same reason.

## How these screens are tested

Manager screens in this repo are **mostly untested**. `account_manager.rs`,
`category_manager.rs`, `rules_manager.rs`, `reconcile_manager.rs`,
`import_manager.rs`, `undo_manager.rs` and `load_manager.rs` have no `mod tests`
at all. The two precedents that exist:

- `src/cli/settings_manager.rs` — builds the manager against a `tempfile` DB,
  drives `handle_key`, asserts on manager state and on DB rows.
- `src/browser.rs` — a large pure-state suite over scrolling, selection, search
  and edit buffers, with no terminal involved.

Neither renders a frame. These screens follow suit: state and data-layer
assertions through `handle_key`, no `draw()` under test, plus direct tests for
the pure helpers (status colour, truncation, form validation, the `Perform`
transition). Test DBs need `init_db` **and** `run_migrations` — the invoicing
tables arrive in migration v4.

`cargo test -- --test-threads=1`; the DB password is a process global.

## Documentation

`CLAUDE.md` gains two Architecture bullets (Client Manager, Invoice Manager),
updates the Dashboard bullet's shortcut list, and notes the draw-then-block send
under Key Design Constraints. `README.md` gains the two screens wherever the
dashboard commands are listed. `docs/invoicing.md` gains a short "From the
dashboard" section.

---

## Open questions for the orchestrator

1. **Shortcut letters.** `n` = Invoices (i**N**voices) is solid. `k` = Clients
   is the weak one — it leans on the /k/ sound because `c` is Reconcile. Free
   alternatives: `m`, `w`, `d`, `f`, `g`, `h`, `j`, `o`, `x`, `y`. Worth asking
   whether `k` reads, or whether Clients should take `o` and Invoices `n` stay.

2. **Actions on the list, or detail only?** This spec binds `s`/`p`/`v` to the
   detail view only, so you always look at an invoice before emailing or voiding
   it. The cost is two keystrokes on the common "money landed, record it" flow.
   Should `p` (and only `p`) also fire from the list?

3. **The dashboard cannot create an invoice.** 68.4's scope is list/detail plus
   three actions; no subtask in epic 68 adds a new-invoice form to the TUI, so
   `nigel invoice new` stays the only way in. The empty state has to say so. Is
   that intended, or should a draft form be folded in here or filed as 68.7?

4. **Stale `overdue`.** The list prints the stored status, so an invoice that
   passed its due date since the last write still reads `sent`. Matching
   `nigel invoice list` exactly, and no screen should rewrite rows on open — but
   the alternative (derive overdue for display only, from `due_date` vs today,
   without writing) is one line and arguably more truthful. Which?

5. **Invoice list columns.** Five columns fit 80 exactly (`#`, Status, Client,
   Total, Balance, Due). Balance is the number you act on but it is not in the
   task text, and dropping it would give Client 12 more characters. Keep it?

6. **A client detail view?** 68.1 built `client_summary` / `ClientSummary`
   (client + invoice history + outstanding) for `nigel client show`. An
   `Enter`-opens-detail screen on the Clients list would be nearly free and
   would show a client's invoices and what they owe. It is outside 68.4's
   "list, add, edit", and the third list column would probably become
   outstanding instead of Billing address. Fold it in, or leave it for 68.6's
   web parity to define first?

7. **No client delete.** Deliberate — no `delete_client` exists and a client with
   invoices must not disappear. Confirm that "list, add, edit" is the whole
   Clients screen.

8. **The frozen frame during send.** No spinner: the terminal is genuinely
   unresponsive for the duration and S7 says so in as many words. Confirm that
   is preferable to a thread, a second DB connection, and a real spinner.

9. **One reworded payment error.** The TUI drops `--amount ` from the
   "greater than zero" message, because a flag name is useless beside a form
   field. That is a departure from "the same error wording the CLI prints" in
   the task text. Sanctioned? (The date check is no longer a departure — it goes
   through 68.1's `validate_date`.)

10. **Ordering against 68.1.** This design now consumes 68.1's real signatures
    (`ClientUpdate`, `void_invoice(.., voided_on)`, `ensure_voidable`,
    `validate_date`, `Invoice.voided_at` / migration v5). 68.4 cannot start its
    edit and void steps until 68.1 lands. Every other step — the invoice list,
    detail, payment form, send flow, and both dashboard wirings — is independent
    and could run in parallel. Worth sequencing that way?
