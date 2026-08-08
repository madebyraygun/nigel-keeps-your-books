# HTTP API

`nigel serve` exposes a JSON API and the web UI on the loopback interface. This
document is the endpoint inventory — every API change updates it in the same
commit.

## Running the server

```bash
nigel serve                 # binds 127.0.0.1:5731 and opens a browser
nigel serve --port 8080     # a different port
nigel serve --port 0        # an ephemeral port (the chosen port is printed)
nigel serve --no-open       # print the URL instead of launching a browser
```

The server prints a URL containing a one-time session token and opens it unless
`--no-open` was passed. Ctrl-C shuts it down, draining in-flight requests.

## Security model

Localhost is not a trust boundary on its own, so three checks apply in order:

1. **Bind.** The listener is on `127.0.0.1` only — never `0.0.0.0`.
2. **Host/Origin.** The `Host` header, and `Origin` when the request carries
   one, must name `localhost`, `127.0.0.1`, or `[::1]` on any port. Anything
   else is `403 forbidden`. Matching is exact, so names like
   `127.0.0.1.example.com` that resolve to loopback are still rejected — this is
   what blocks DNS rebinding. Allowing any port is what lets the vite dev server
   on `:5173` proxy through.
3. **Session.** A 32-byte token is generated per server run.
   `GET /auth?token=<token>` compares it in constant time, sets the
   `nigel_session` cookie (`HttpOnly`, `SameSite=Strict`, `Path=/`), and
   redirects to `/`. Every `/api` route requires that cookie; without it the
   response is `401 unauthorized`. Static assets carry no data and are served
   without a session.

The token is never written to disk and never appears in a response body. The
database password is likewise runtime-only.

## Locked state

An encrypted database starts **locked**: the server has the file but not the
key. `GET /api/ping`, `GET /api/status`, and `POST /api/unlock` answer in that
state; every other `/api` path returns `423 locked` until the password arrives —
including unknown ones, which are locked before they are reported missing. Unlocking also runs any pending schema migrations, which
`nigel serve` cannot run at startup on a database it cannot open.

Unlock is process-wide, so one server run serves one database — the same
assumption every CLI subcommand makes. The password is held only in memory for
the lifetime of the process; it is never written to disk, logged, or echoed in
a response. (It does sit in the request buffer until that is dropped, exactly as
it does in the terminal prompt.)

An unencrypted database is never locked and needs no unlock call.

## Conventions

- All endpoints live under `/api` and speak JSON in both directions.
- Every API-visible field is `camelCase`.
- Date parameters mirror the CLI: `year` (integer), `month` (`YYYY-MM`), `from`
  and `to` (`YYYY-MM-DD`, which must be supplied as a pair), `account` (name).
- The web UI's TypeScript mirror of these structs lives in
  `web/apps/app/src/api/types.ts` — one interface per response struct, named
  after it. Anything added here is added there in the same commit.

### Error envelope

Errors always take this shape, with `details` present only when there is
structured context to give:

```json
{ "error": { "code": "not_found", "message": "No API endpoint at /api/nope" } }
```

| Code | Status | Meaning |
|------|--------|---------|
| `bad_request` | 400 | Malformed or contradictory parameters |
| `unauthorized` | 401 | Missing or invalid session cookie |
| `invalid_password` | 401 | Wrong database password; `details` carries the attempt budget |
| `forbidden` | 403 | Host/Origin check failed |
| `not_found` | 404 | No such route or record |
| `conflict` | 409 | A guardrail blocked the change; `details` carries a reason code |
| `payload_too_large` | 413 | An upload exceeded the size limit |
| `locked` | 423 | The database is encrypted and not yet unlocked |
| `internal` | 500 | Unexpected server-side failure |
| `feature_disabled` | 501 | The build lacks the required cargo feature |

## Endpoints

### `GET /auth?token=<token>`

Exchanges the startup token for a session cookie. Responds `302` with
`Location: /` and a `Set-Cookie` header on success, `401` otherwise. This is the
only route outside `/api` that is part of the API surface.

### `GET /api/ping`

Liveness probe. Requires a session; touches no data.

```json
{ "ok": true, "version": "1.0.1" }
```

### `GET /api/status`

Server and database state. Answers in every state, including locked and
uninitialized, so the SPA can decide which screen to show before it has data.

```json
{
  "initialized": true,
  "encrypted": true,
  "locked": false,
  "companyName": "Raygun LLC",
  "profile": "business",
  "version": "1.0.1",
  "dataDir": "/home/you/Documents/nigel",
  "pdfExport": true,
  "updateAvailable": "1.0.2"
}
```

`initialized` is whether the database file exists, `encrypted` whether it needs
a key, and `locked` whether this process still lacks that key. `companyName` is
`null` while locked or uninitialized — reading it requires the key — and
`dataDir` names the directory of the database the server actually opened.
`profile` is `business` or `personal` — which chart of accounts the database
was created with (`nigel init --profile`). The SPA hides the K-1 worksheet
from the report directory and relabels the name field for personal books;
while locked or uninitialized it reads `business`, since the profile lives in
the database. The K-1 report and export routes stay reachable either way,
exactly as `nigel report k1-prep` does in a terminal.
`pdfExport` is whether this binary was built with the `pdf` feature; a client
uses it to decide whether to offer a PDF download at all (see
[Exporting reports](#exporting-reports)).

`updateAvailable` is the version of a newer release, or `null`. It is the check
`nigel` already runs at launch, under the same rules: the `update_check` setting
turns it off, and GitHub is asked at most once every 24 hours. It runs in a
background task started with the server rather than inside the handler, so it is
`null` on the first status calls of a run and fills in once GitHub answers — a
client that wants to catch it re-reads status rather than expecting it on the
first fetch. A platform with no release asset reports `null`, matching what
`nigel update` could actually install.

### `POST /api/unlock`

Supplies the password for an encrypted database.

```json
{ "password": "…" }
```

On success the process adopts the key, runs any pending migrations, and answers
`200`:

```json
{ "locked": false }
```

Calling it again while already unlocked is a no-op `200`; it is not a password
checker. On an unencrypted database it is `400 bad_request`.

A wrong password is `401` with code `invalid_password`:

```json
{
  "error": {
    "code": "invalid_password",
    "message": "Wrong password.",
    "details": { "attemptsRemaining": 1, "retryAfterMs": 0 }
  }
}
```

`attemptsRemaining` counts down from 3 and then stays at 0 — there is no hard
lockout, because whoever is at the keyboard can always restart the server.
Instead, repeated failures get slower: the server holds the response back before
answering, and `retryAfterMs` reports how long it held *this* one. Since that
delay has already been served, the client may retry immediately.

| Consecutive failures | 1 | 2 | 3 | 4 | 5 | … | 8+ |
|---|---|---|---|---|---|---|---|
| Delay | 0 | 0 | 1s | 2s | 4s | ×2 | 30s |

A successful unlock resets the counter. It is not persisted, so restarting the
server clears it.

## Reading data

Every endpoint below answers `423 locked` until an encrypted database has been
unlocked. All are `GET`, and all are read-only. Most read the database;
`/api/imports/formats` reads a compiled-in list and is gated by policy rather
than need, for the same reason `settings/app` is.

Every `/api/reports/*` route answers with the `{ "granularity": …, "report": … }`
envelope described under [Report responses](#report-responses); the type in the
table is what `report` holds. The list routes below it answer with a bare array.

| Route | Parameters | Response |
|---|---|---|
| `/api/reports/pnl` | `year`, `month`, `from`+`to` | `ReportEnvelope<PnlReport>` |
| `/api/reports/expenses` | `year`, `month` | `ReportEnvelope<ExpenseBreakdown>` |
| `/api/reports/tax` | `year` | `ReportEnvelope<TaxSummary>` |
| `/api/reports/cashflow` | `year`, `month` | `ReportEnvelope<CashflowReport>` |
| `/api/reports/balance` | — | `ReportEnvelope<BalanceReport>` |
| `/api/reports/flagged` | — | `ReportEnvelope<FlaggedTransaction[]>` |
| `/api/reports/register` | `year`, `month`, `from`+`to`, `account` | `ReportEnvelope<RegisterReport>` |
| `/api/reports/k1` | `year` | `ReportEnvelope<K1PrepReport>` |
| `/api/accounts` | — | `Account[]` |
| `/api/categories` | — | `CategoryRow[]` |
| `/api/rules` | — | `RuleRow[]` |
| `/api/imports` | — | `ImportListItem[]` |
| `/api/imports/formats` | — | `ImporterFormat[]` |
| `/api/csv-profiles` | — | `CsvProfile[]` |

### Date parameters

The parameters a route accepts are exactly the flags its `nigel report`
subcommand accepts. Passing one a route does not support is `400`, rather than
being ignored: silently dropping `from`/`to` from an expense breakdown would
answer a question nobody asked.

- `year` — an integer, e.g. `2025`.
- `month` — `YYYY-MM`, zero-padded. Supplying `month` alone also fixes the year.
- `from` and `to` — `YYYY-MM-DD`, zero-padded, and **must be supplied as a
  pair**. One without the other is `400`.
- `account` — an account name, on `/api/reports/register` only. A name no
  account has is `404`; an account that simply has no matching transactions is a
  `200` with an empty `rows`.

When both `year` and `month` are given, `year` wins and `month` contributes only
its month number — `?year=2024&month=2025-03` means March 2024. This matches the
CLI.

Unlike the CLI, which ignores a date it cannot parse and quietly widens the
query, the API rejects it: `?month=2025-13`, `?month=2025-3`, and
`?from=2025-1-5` are all `400`.

### Report responses

Every report is wrapped with the date granularity it supports, so the SPA can
build its date controls from the response instead of a hardcoded table:

```json
{
  "granularity": "monthAndYear",
  "report": { "income": [], "expenses": [], "totalIncome": 0, "totalExpenses": 0, "net": 0 }
}
```

`granularity` is one of `monthAndYear` (P&L, expenses, cash flow, register),
`yearOnly` (tax, K-1), or `none` (balance, flagged). It tells the client which
of `year` and `month` that route will accept.

### List responses

The six list endpoints answer with a bare JSON array — no envelope, no
pagination.

- `/api/accounts` — every account, by name.
- `/api/categories` — the active chart of accounts; soft-deleted categories are
  omitted.
- `/api/rules` — active rules in the order the categorizer applies them:
  priority descending, ties by id. `vendor` is `null` when the rule sets none.
- `/api/imports` — import history, newest first, each with the number of
  transactions still attached. An import whose transactions were undone still
  lists, at `transactionCount: 0`.
- `/api/csv-profiles` — saved generic-CSV column mappings, by name:

```json
[
  {
    "name": "chase",
    "config": { "dateCol": 0, "descCol": 1, "amountCol": 3, "dateFormat": "%m/%d/%Y" }
  }
]
```

## Changing data

Every endpoint in this section is part of a write flow, and every one is
refused with `423 locked` until an encrypted database is unlocked. Three are
`GET`s that a write flow reads first, and two of the `POST`s — `rules/test` and
`imports/preview` — are dry runs that write nothing.

| Route | Method | Body | Response |
|---|---|---|---|
| `/api/transactions/:id` | `PATCH` | `categoryId?`, `vendor?`, `flag?` | `RegisterRow` |
| `/api/categorize` | `POST` | — | `CategorizeResult` |
| `/api/review/queue` | `GET` | — | `FlaggedTxn[]` |
| `/api/review/:id` | `GET` | — | `RegisterRow` |
| `/api/review/:id/apply` | `POST` | `categoryId`, `vendor?`, `createRule?`, `rulePattern?` | `{ transactionId, ruleId }` |
| `/api/review/:id/undo` | `POST` | `ruleId?` | `RegisterRow` |
| `/api/accounts` | `POST` | `name`, `accountType`, `institution?`, `lastFour?` | `Account` (`201`) |
| `/api/accounts/:id` | `PATCH` | `name` | `Account` |
| `/api/accounts/:id` | `DELETE` | — | `{ id, deleted }` |
| `/api/categories` | `POST` | `name`, `categoryType`, `taxLine?`, `formLine?` | `CategoryRow` (`201`) |
| `/api/categories/:id` | `PATCH` | `name?`, `categoryType?`, `taxLine?`, `formLine?` | `CategoryRow` |
| `/api/categories/:id` | `DELETE` | — | `{ id, deleted }` |
| `/api/rules` | `POST` | `pattern`, `categoryId`, `matchType?`, `vendor?`, `priority?` | `RuleRow` (`201`) |
| `/api/rules/:id` | `PATCH` | `pattern?`, `categoryId?`, `matchType?`, `vendor?`, `priority?` | `RuleRow` |
| `/api/rules/:id` | `DELETE` | — | `{ id, deleted }` |
| `/api/rules/test` | `POST` | `pattern`, `matchType?` | `RuleTestResult` |
| `/api/reconcile` | `POST` | `account`, `month`, `statementBalance` | `ReconcileResult` |
| `/api/reconciliations` | `GET` | `account` (query) | `ReconciliationRecord[]` |
| `/api/imports/:id` | `DELETE` | — | `{ id, deletedTransactions }` |

### Write conventions

- **Creating** answers `201` with the new row, which carries the id and any
  server-side defaults. **Editing** and **deleting** answer `200`; deletes carry
  a small JSON body rather than a bare `204` so every response decodes the same
  way.
- **`PATCH` is a true partial update.** A field you omit is left alone, so two
  screens editing different fields of the same row cannot blank each other's
  work. A `PATCH` with no recognized field is `400` — an empty edit is more
  likely a bug than an intention.
- **`null` clears a field**, where clearing makes sense: `vendor` on a
  transaction or a rule, `taxLine` and `formLine` on a category. `categoryId` is
  the exception — `null` there is `400`, because uncategorizing is what
  `/api/review/:id/undo` is for.
- Unknown fields in a body are ignored, except on `/api/imports/preview` and
  `/api/imports/confirm`, where an unrecognized key is a `400`: those two carry
  the column mapping, and a misspelled field there would silently import the
  wrong columns.
- Requesting a route with the wrong method is axum's bodyless `405`, the one
  response on the API that is not an error envelope.

### Transaction edits

`PATCH /api/transactions/:id` is what the register's inline editing sends. All
three fields are applied in one database transaction, so a rejected value — an
unknown `categoryId`, say — leaves the row exactly as it was.

```json
{ "categoryId": 12, "vendor": "Adobe", "flag": false }
```

`flag` is a state, not a toggle: sending `true` twice leaves the transaction
flagged once, which is what lets a client retry without wondering whether the
first attempt landed. The response is the full register row, ready to swap into
a table in place.

`POST /api/categorize` runs the rules engine over everything uncategorized, the
same pass an import ends with, and answers with what it did:

```json
{ "categorized": 12, "stillFlagged": 3 }
```

### Review

`GET /api/review/queue` is the work list — flagged transactions, oldest first.
`GET /api/review/:id` fetches one, for re-reviewing a specific transaction the
way `nigel review --id` does.

`apply` records a decision and optionally turns it into a rule:

```json
{ "categoryId": 12, "vendor": "Adobe", "createRule": true, "rulePattern": "ADOBE" }
```

```json
{ "transactionId": 42, "ruleId": 7 }
```

`ruleId` is `null` unless a rule was created. `createRule` without a
`rulePattern` is `400` rather than a silently rule-less success.

`undo` takes that `ruleId` back:

```json
{ "ruleId": 7 }
```

It re-flags the transaction, clears the category and vendor, and **deletes** the
rule outright — undo leaves no trace of the decision, which is what makes the
review screen's back button safe. The body is required, but `{}` is valid and
means "just restore the transaction". The response is the restored register row.

### Accounts, categories, and rules

Accounts are hard-deleted and categories are soft-deleted, exactly as in the CLI
and the TUI. `PATCH /api/accounts/:id` renames and nothing else — institution
and last four are set at creation, which is all the data layer offers.

Rules address their category by **id**; only the CLI resolves a category name.
`matchType` defaults to `contains` and `priority` to `0`, matching
`nigel rules add`. A `matchType` outside `contains`, `starts_with`, and `regex`
is `400`, as is a `regex` pattern that will not compile — the categorizer
answers "no match" to both, so a rule saved with either would be dead weight.

`POST /api/rules/test` is a dry run against the real transaction descriptions,
the same scan `nigel rules test` prints:

```json
{ "pattern": "ADOBE", "matchType": "contains" }
```

```json
{
  "total": 3,
  "matches": [{ "description": "ADOBE CREATIVE CLOUD", "count": 3 }]
}
```

Identical descriptions collapse into one entry with a count, busiest first.
Matching nothing is a `200` with `total: 0`, not an error.

### Reconcile and undo

`POST /api/reconcile` compares a statement balance against the calculated one
and **records the result**, including a mismatch — that record is how the
history knows which months have been checked. `GET /api/reconciliations`, with
an optional `?account=`, returns that history newest month first.

```json
{ "account": "BofA Checking", "month": "2025-02", "statementBalance": 4928.01 }
```

```json
{
  "isReconciled": true,
  "statementBalance": 4928.01,
  "calculatedBalance": 4928.01,
  "discrepancy": 0.0
}
```

`DELETE /api/imports/:id` rolls back one import — its transactions and its
record — the way `nigel undo` rolls back the most recent one. It answers with
`{ "id": 3, "deletedTransactions": 42 }`. An import that is already gone is
`404`, not a successful undo of nothing.

## Running an import

Importing over HTTP takes three calls, because a browser has no file path to
hand over and no chance to look at a statement before committing it:

| Step | Route | What it does |
|---|---|---|
| 1 | `POST /api/imports/upload` | Parks the file on the server and names it |
| 2 | `POST /api/imports/preview` | Parses it and reports what would happen |
| 3 | `POST /api/imports/confirm` | Snapshots, imports, and categorizes |

Preview and confirm take the same request body, so the second is the first with
the decision made. Nothing carries over between calls except the `uploadId`.

### `POST /api/imports/upload`

A `multipart/form-data` body with one file field, conventionally named `file`
— the first field carrying a filename is the one that is read.

```json
{ "uploadId": "9f3c…", "filename": "april-2025.csv", "size": 8214 }
```

- The limit is **25 MB**; over it is `413 payload_too_large`.
- The file must be a `.csv`, `.xlsx`, or `.xls`; anything else is `400`. The
  extension is kept, because two importers dispatch on it.
- The stored filename is the one you sent, reduced to safe characters. It is
  what the `imports` record and the import history will show.
- Uploads are private to the account running the server (mode `600` in a `700`
  directory) and expire after **an hour**. A confirmed import deletes its
  upload immediately; a failed one keeps it, so the same `uploadId` can be
  retried.

### `POST /api/imports/preview`

```json
{
  "uploadId": "9f3c…",
  "account": "BofA Checking",
  "format": "bofa_checking",
  "mapping": null
}
```

`format` and `mapping` are both optional and **mutually exclusive** — sending
both is `400` rather than a guess about which one you meant:

- Neither: the format is detected from the account's type and the file itself,
  exactly as `nigel import` does with no `--format`.
- `format`: a built-in key from `GET /api/imports/formats`, or the name of a
  saved profile from `GET /api/csv-profiles`. An unknown name is `400`. A
  built-in key this binary was compiled without — `gusto_payroll` in a no-Gusto
  build — is `501 feature_disabled`.
- `mapping`: column positions for a CSV nothing built in can read, the same
  four values a saved profile holds.

```json
{
  "format": "bofa_checking",
  "duplicateFile": false,
  "imported": 42,
  "skipped": 3,
  "malformed": 1,
  "importId": null,
  "sample": [{ "date": "2025-04-01", "description": "ACME CORP", "amount": 3000.0 }]
}
```

`format` is what actually resolved: a built-in key, a profile name, or
`"generic"` for an inline `mapping`. `imported` and `skipped` are what *would*
happen — preview writes nothing at all, and `sample` is the first five rows.

### `POST /api/imports/confirm`

The preview body plus an optional `saveProfile`, which remembers the `mapping`
under that name for next time. It requires a `mapping` to save and refuses the
name of a built-in importer; both are `400`. The profile is written only after
the import succeeds.

The sequence is the one the terminal UI has always used: a pre-import snapshot
into `<data-dir>/snapshots/`, then the import, then auto-categorization.

```json
{
  "format": "bofa_checking",
  "duplicateFile": false,
  "imported": 42,
  "skipped": 3,
  "malformed": 1,
  "importId": 7,
  "sample": [],
  "categorized": 38,
  "stillFlagged": 6,
  "snapshot": "/home/you/Documents/nigel/snapshots/pre-import-20250401-120000.db"
}
```

`importId` addresses the new batch — it is what `DELETE /api/imports/:id` undoes
and what the review screen filters by. `stillFlagged` counts the whole ledger,
not just this import.

### What is data and what is an error

Two outcomes look like failures and are not:

- **A file that was already imported** is `200` with `duplicateFile: true`, zero
  counts, and a null `format` and `importId` — the checksum is checked before
  anything else, so nothing was parsed or written. This is what the CLI prints
  as "This file has already been imported".
- **Rows that could not be parsed** are counted in `malformed` and skipped. A
  statement with a bad row still imports its good ones.

Genuine failures:

| Case | Status |
|---|---|
| `uploadId` unknown or expired | `404`, `details.reason` = `upload_not_found` |
| Account does not exist | `404` |
| Format name unknown | `400` |
| Format needs a cargo feature this build lacks | `501` |
| No importer can read this file for that account type | `400` |
| The file will not parse at all | `400`, carrying the parser's message |
| Upload over 25 MB | `413` |

### Conflict reasons

A `409` always carries `details.reason`, so a client can explain the block in
its own words instead of parsing ours:

| Reason | Also carries | Raised by |
|---|---|---|
| `has_transactions` | `count` | Deleting an account or category still in use |
| `has_active_rules` | `count` | Deleting a category an active rule assigns |
| `duplicate_name` | `name` | Creating or renaming to a name already taken |
| `already_inactive` | — | Editing or deleting an already soft-deleted rule |
| `no_transactions` | `account`, `month` | Reconciling a month with nothing in it |
| `already_encrypted` | — | Setting a password on an encrypted database |
| `not_encrypted` | — | Changing or removing the password on a plaintext one |

```json
{
  "error": {
    "code": "conflict",
    "message": "Cannot delete: account has 5 transactions",
    "details": { "reason": "has_transactions", "count": 5 }
  }
}
```

## Exporting reports

Every report downloads as a PDF or as plain text. These are the only endpoints
that answer with something other than JSON — on success. Errors keep the usual
envelope, so a client that decodes the body on a non-`200` is never surprised.

| Route | Report | Parameters |
|---|---|---|
| `/api/exports/pnl` | `pnl` | `format`, `year`, `month`, `from`+`to` |
| `/api/exports/expenses` | `expenses` | `format`, `year`, `month` |
| `/api/exports/tax` | `tax` | `format`, `year` |
| `/api/exports/cashflow` | `cashflow` | `format`, `year`, `month` |
| `/api/exports/balance` | `balance` | `format` |
| `/api/exports/flagged` | `flagged` | `format` |
| `/api/exports/register` | `register` | `format`, `year`, `month`, `from`+`to`, `account` |
| `/api/exports/k1` | `k1` | `format`, `year` |

The date and account parameters are the ones the matching `/api/reports` route
takes, validated by the same code and rejected with the same message — an
export of a report is the report, in a different wrapper.

`format` is the one parameter of their own, and it is **required**: either `pdf`
or `text`, lowercase. Omitting it is `400`, as is any other value. There is no
default, because guessing one would either hand a text file to something that
asked for a PDF or answer `501` to a caller who never mentioned PDFs.

### Response

```
200 OK
Content-Type: application/pdf                     (format=pdf)
Content-Type: text/plain; charset=utf-8           (format=text)
Content-Disposition: attachment; filename="pnl-2026-08-07.pdf"
```

The filename is the report's slug and today's date — the same name
`nigel report pnl --mode export` writes into `<data_dir>/exports/`, so a file
saved from the browser and one written by the CLI are named alike. The K-1
worksheet is `k1` in the URL and `k1-prep` in the filename, matching the CLI
there too. Nothing from the database appears in the header.

The text body is byte-for-byte what the CLI's `--format text` export writes:
the company name, a blank line, then the report. Terminal colouring is off for
the whole process while serving, so no escape sequences reach the file.

### Without the `pdf` feature

`format=text` is unaffected. `format=pdf` is `501 feature_disabled`, carrying
the same sentence the CLI prints:

```json
{
  "error": {
    "code": "feature_disabled",
    "message": "PDF export requires the 'pdf' feature — build with `cargo build --features pdf`"
  }
}
```

Since browsers download an anchor's target without inspecting it, a client
should read `pdfExport` from `GET /api/status` and not offer the link at all in
such a build — otherwise the saved `.pdf` file is that JSON.

### Not on the API

- **Bulk export.** `nigel report all` writes eight files into a directory; a
  browser downloads one file at a time. There is no `/api/exports/all`.
- **Writing files.** The CLI's `--output` and `--output-dir` choose a path on
  disk. The server only streams bytes back; where they land is the browser's
  business.
## Settings

What the TUI's settings screen covers, plus switching data directories. Every
route here is behind the locked guard, including the two that never touch the
database — see below.

| Route | Method | Body | Response |
|---|---|---|---|
| `/api/settings/app` | `GET` | — | `AppSettings` |
| `/api/settings/app` | `PUT` | `updateCheck` | `AppSettings` |
| `/api/settings/company-name` | `PUT` | `name` | `{ companyName }` |
| `/api/settings/data-dir` | `POST` | `path` | `StatusResponse` |
| `/api/settings/password/set` | `POST` | `newPassword` | `{ encrypted, locked }` |
| `/api/settings/password/change` | `POST` | `currentPassword`, `newPassword` | `{ encrypted, locked }` |
| `/api/settings/password/remove` | `POST` | `currentPassword` | `{ encrypted, locked }` |

### Application settings

`AppSettings` is the part of `settings.json` the web UI shows:

```json
{ "userName": "Dalton", "updateCheck": true, "lastUpdateCheck": "2026-08-06T09:12:00Z" }
```

`updateCheck` is the only field a `PUT` may change. The user name is collected
during onboarding and the timestamp is the updater's bookkeeping; both are
display-only here, and sending them is ignored rather than refused. There is no
`dataDir` field: `/api/status` already reports the directory the server actually
opened, and one value with two sources is one value that will disagree with
itself.

### Business name

`PUT /api/settings/company-name` writes the `company_name` metadata key — the
name `/api/status` reports and the SPA puts in the sidebar. The name is trimmed,
and an empty one is allowed: clearing the business name is what the TUI does
with a blank field.

### Switching data directory

`POST /api/settings/data-dir` is `nigel load` plus the three things a *running*
server has to do. It validates that the target holds a `nigel.db` (`400` if not,
with the same message the CLI prints), rewrites `settings.json`, and then:

- **rebinds this process** to the new database, so every later request reads it.
  Rewriting `settings.json` alone would leave the server serving the old books
  under the new directory's name.
- **clears the password**, so an encrypted target comes up locked rather than
  inheriting the previous database's key.
- **resets the failed-attempt budget**, which belongs to a database rather than
  to a process.
- **runs pending migrations** when the target is unencrypted, the same
  pre-flight `nigel serve` does at startup. An encrypted target is migrated by
  the unlock that follows.

The response is a full `StatusResponse` describing the database just moved to,
so a client that does not reload still knows where it stands. The SPA does
reload, which is the simplest way to make every screen re-derive from scratch.

### Password management

The three password routes wrap the same functions `nigel password` uses, and the
new password is trimmed exactly as the terminal prompt trims it — so a password
set from the browser can always be typed back in at the terminal.

- **set** requires a plaintext database; on an encrypted one it is `409`
  `already_encrypted`. On success the process adopts the new key, so the session
  continues without an unlock.
- **change** and **remove** require an encrypted one; on a plaintext database
  they are `409` `not_encrypted`. Both take the current password and verify it
  before touching the file.
- A wrong `currentPassword` is `401 invalid_password` with the same
  `attemptsRemaining` / `retryAfterMs` details `POST /api/unlock` returns, and it
  draws down the *same* budget. Guessing a password through the change endpoint
  costs exactly what guessing it through unlock costs, or the throttle would be
  decoration.
- An empty new password is `400`. Removing encryption is what `remove` is for.

Encrypting and decrypting rewrite the database file itself — a rename, and the
`-wal`/`-shm` sidecars deleted — so the server holds an exclusive lock across
them. In-flight reads finish first, and reads that arrive during the operation
wait rather than opening a file that is about to stop being the database.

### Why none of these is exempt from the locked guard

`company-name` needs the key, so it could not work while locked in any case. The
other five could:

- `settings/app` reads only `settings.json`, but nothing on the unlock screen
  needs it. Exempting a route to serve a screen that does not exist is how a
  guard rots.
- `data-dir` would make "switch away from a database whose password I forgot" a
  browser action. It is still refused, matching the TUI, where the load screen
  lives behind the same gate. The recovery path is `nigel load` and a restart.
- `password/change` and `password/remove` carry the current password in the
  body, so they would function while locked — and would then be an unthrottled
  password oracle reachable without ever passing the unlock screen.

## Static assets

Everything that is not `/auth` or `/api` is served from the SPA bundle embedded
in the binary. Unknown paths return `index.html` so client-side routes survive a
reload, while unknown `/api` paths return a JSON `404` rather than the HTML
shell.
