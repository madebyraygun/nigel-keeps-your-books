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
  "version": "1.0.1",
  "dataDir": "/home/you/Documents/nigel"
}
```

`initialized` is whether the database file exists, `encrypted` whether it needs
a key, and `locked` whether this process still lacks that key. `companyName` is
`null` while locked or uninitialized — reading it requires the key — and
`dataDir` names the directory of the database the server actually opened.

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

Every endpoint below reads the database, so all of them answer `423 locked`
until an encrypted database has been unlocked. All are `GET`, and all are
read-only.

| Route | Parameters | Response |
|---|---|---|
| `/api/reports/pnl` | `year`, `month`, `from`+`to` | `PnlReport` |
| `/api/reports/expenses` | `year`, `month` | `ExpenseBreakdown` |
| `/api/reports/tax` | `year` | `TaxSummary` |
| `/api/reports/cashflow` | `year`, `month` | `CashflowReport` |
| `/api/reports/balance` | — | `BalanceReport` |
| `/api/reports/flagged` | — | `FlaggedTransaction[]` |
| `/api/reports/register` | `year`, `month`, `from`+`to`, `account` | `RegisterReport` |
| `/api/reports/k1` | `year` | `K1PrepReport` |
| `/api/accounts` | — | `Account[]` |
| `/api/categories` | — | `CategoryRow[]` |
| `/api/rules` | — | `RuleRow[]` |
| `/api/imports` | — | `ImportListItem[]` |
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

The five list endpoints answer with a bare JSON array — no envelope, no
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

## Static assets

Everything that is not `/auth` or `/api` is served from the SPA bundle embedded
in the binary. Unknown paths return `index.html` so client-side routes survive a
reload, while unknown `/api` paths return a JSON `404` rather than the HTML
shell.
