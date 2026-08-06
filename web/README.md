# nigel web UI

The single-page app `nigel serve` embeds and serves. An npm workspace with
three packages, built in order by the root `build` script.

| Package | Path | Purpose |
|---|---|---|
| `@nigel/theme` | `packages/theme` | Design tokens as Lit `css` modules composed into one `CSSResult`, plus a generated plain stylesheet. |
| `@nigel/ui` | `packages/ui` | `wc-*` Lit components and the preview harness. Depends on `lit` only — no signals, no API types. |
| `@nigel/app` | `apps/app` | Composition: screens, store, api client. Depends on the other two. |

Stack: lit 3, Web Awesome 3, `@lit-labs/signals` 0.2, vite 6, vitest 2,
TypeScript 5.7. Node 20.19+ (22 recommended). No turbo — the root scripts
chain the three builds explicitly.

## Commands

Run from this directory.

```bash
npm ci                 # install (uses the committed lockfile)
npm run dev            # vite dev server on :5173
npm run preview        # component preview harness on :9090
npm test               # vitest across all three packages
npm run lint           # eslint across all three packages
npm run typecheck      # tsc --noEmit across all three packages
npm run build          # theme -> ui -> app, output to web/dist
```

## Dev loop

The dev server proxies to a running backend, so both halves stay live.

```bash
# terminal 1 — the backend
cargo run -- serve --no-open
#   prints: http://127.0.0.1:5731/auth?token=<hex>

# terminal 2 — the frontend
cd web && npm run dev
#   vite on http://localhost:5173
```

Then open the token URL **on the vite origin**, not the backend's:

```
http://localhost:5173/auth?token=<hex>
```

Why that works: vite proxies `/auth` and `/api` to `127.0.0.1:5731`. The
backend's Host check accepts `localhost` on any port, which is what makes a
dev proxy possible at all. The `Set-Cookie` for `nigel_session` carries no
`Domain`, so the browser stores it host-only for `localhost:5173` — the origin
you are actually on — and the `302` to `/` is relative, so you stay there.
Every later `/api` call is same-origin from the browser's point of view, which
is why the client sends `credentials: 'same-origin'`.

Restarting `nigel serve` mints a new token and invalidates the cookie. The next
API call returns 401, `appUnauthorized` flips, and the shell shows a banner
telling you to reopen the newly printed URL — the page cannot mint a token
itself.

## Component-first workflow (mandatory)

Every visual change ships through `@nigel/ui`:

1. The component lives in `packages/ui/src/components/` as `wc-foo.ts`.
2. A co-located `wc-foo.preview.ts` covers the visible states.
3. `wc-foo.test.ts` calls `describePreviewA11y(preview)`, which runs axe over
   every state the preview declares — add a state and its a11y test comes with
   it.
4. Only then does `apps/app` consume it. There are no bespoke component
   implementations under `apps/app/src/components/` other than the root
   container.

Use Web Awesome `<wa-*>` primitives unless behavior demands custom. Wrappers
read theme tokens and expose them as cascading variables; they never inline a
brand value. Pure logic, state and service work is exempt.

## The api seam

`apps/app/src/api/` is the only module that talks to the server. This is the
hinge the Tauri and multiuser plans swing on, so it has rules:

- `types.ts` mirrors the serde structs by hand, camelCase, one interface per
  response struct named exactly as the Rust one. Keep it in step with
  `docs/api.md` in the same commit as the endpoint.
- `ApiClient` gets **one method per endpoint**, named after it. Routes with
  more than one parameter take a single typed options object, so adding a
  parameter stays non-breaking.
- There is no generic `request()` on the interface, and there must not be. It
  would let screens build URLs, which a Tauri or remote client cannot honour.
- Cross-cutting transport state (`appLocked`, `appUnauthorized`) lives here,
  not in components.

A guard test fails the build if `fetch(`, `XMLHttpRequest`, `EventSource`,
`WebSocket` or `navigator.sendBeacon` appears anywhere under `src` outside
`src/api` — tests included. Use `src/__mocks__/fake-api-client.ts` instead.

## Screens

`apps/app/src/screens/registry.ts` is the single description of a screen:
title, nav label, icon, whether it appears in the sidebar, and how it renders.
The sidebar, the header and the content area all read it, and it is typed as
`Record<ScreenId, ScreenDef>` so a missing entry is a compile error.

Routing is `location.hash`, format `#/<screen>?<params>`. Navigation writes the
hash and nothing else; the `hashchange` listener is the only thing that updates
route state. Unknown hashes fall back to the dashboard.

## How the build reaches the binary

`npm run build` writes to `web/dist`, which `rust-embed` bakes into the binary.

`web/dist` is **generated and gitignored**. The committed fallback is
`web/placeholder/index.html`, and `build.rs` copies it into `web/dist` when no
`index.html` is there, so `cargo build` works on a machine without node — it
just serves a "SPA not built" page.

`build.rs` also emits `cargo:rerun-if-changed` for `web/dist`. This is load
bearing: `rust-embed`'s `debug-embed` decides *when* assets are baked in, not
when cargo reconsiders them, and its proc macro cannot emit the key itself.
Without the build script a fresh `npm run build` followed by `cargo run` would
serve the previously embedded bytes.

One caveat: `rerun-if-changed` on a directory tracks that directory's own
mtime, which covers files appearing and disappearing but not edits nested under
`assets/`. Vite rewrites `index.html` on every build, so that file is tracked
by name as well.
