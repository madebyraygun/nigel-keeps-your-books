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

## Static assets

Everything that is not `/auth` or `/api` is served from the SPA bundle embedded
in the binary. Unknown paths return `index.html` so client-side routes survive a
reload, while unknown `/api` paths return a JSON `404` rather than the HTML
shell.
