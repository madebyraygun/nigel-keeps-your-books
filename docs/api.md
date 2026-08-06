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

## Static assets

Everything that is not `/auth` or `/api` is served from the SPA bundle embedded
in the binary. Unknown paths return `index.html` so client-side routes survive a
reload, while unknown `/api` paths return a JSON `404` rather than the HTML
shell.
