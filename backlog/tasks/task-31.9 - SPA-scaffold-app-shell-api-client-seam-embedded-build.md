---
id: TASK-31.9
title: 'SPA scaffold: app shell, api client seam, embedded build'
status: In Progress
assignee:
  - '@agent-31.9'
created_date: '2026-08-06 16:26'
updated_date: '2026-08-06 21:01'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.3
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.9-spa-scaffold.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create the web/ SPA (Vite; framework, component library, and patterns carried over from boxcraft-app). The load-bearing piece is a single api client module that isolates transport: fetch backend now, with the interface shaped so a Tauri invoke backend and a remote-server backend can be swapped in later without touching components. App shell includes routing, navigation, layout, theme, and centralized loading/error/locked/unauthorized states. Build integration embeds the SPA dist into the binary via rust-embed, with the dev workflow (vite dev server proxying to a running nigel serve) and CI story documented.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 SPA builds and is embedded in the binary; nigel serve serves it at the root path
- [ ] #2 All server communication flows through the single api client module — no scattered fetch calls — with token and locked-state handling centralized
- [ ] #3 Routing and navigation shell exists with routes stubbed for all planned screens
- [ ] #4 Dev workflow (vite proxy against nigel serve) is documented and works
- [ ] #5 CI builds the SPA and cargo test stays green
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## 0. Tooling check (done)

Local: node v24.18.0, npm 11.16.0. Vite 6 requires Node ^18 || ^20 || >=22;
Node 18 is EOL. Plan pins `engines.node >= 20.19.0` and CI/release on Node 22
(actions/setup-node@v4). No other tooling needed -- no turbo, no playwright
(axe runs under jsdom in vitest).

## 1. File tree of web/

```
web/
  package.json            # workspaces root: ["packages/*", "apps/*"], no turbo
  package-lock.json       # committed; CI uses `npm ci`
  tsconfig.json           # base compiler options (shared)
  eslint.config.mjs       # flat config, ported from boxcraft
  .gitignore              # node_modules/, */dist/, dist/
  README.md               # dev guide (deliverable 6)
  placeholder/
    index.html            # committed no-node fallback (moved from dist/)
  dist/                   # GENERATED, gitignored -- vite build output / build.rs seed
  packages/
    theme/
      package.json  tsconfig.json  vitest.config.ts
      scripts/build-css.js
      src/index.ts
      src/global.ts                  # ::part() overrides for wa-* primitives
      src/themes/nigel.ts            # composed CSSResult
      src/themes/index.ts
      src/tokens/{color,gradient,typography,spacing,radius,shadow,motion}.ts
      __tests__/{nigel-theme,palette-parity,contrast,build-css}.test.ts
    ui/
      package.json  tsconfig.json  tsconfig.build.json  vitest.config.ts
      src/index.ts  src/test-setup.ts
      src/icons/{index,icon-base,icons}.ts
      src/icons/icons.preview.ts  src/icons/icons.test.ts
      src/components/index.ts
      src/components/wc-app-shell.{ts,preview.ts,test.ts}
      src/components/wc-nav-sidebar.{ts,preview.ts,test.ts}
      src/components/wc-toast.{ts,preview.ts,test.ts}
      src/components/wc-confirm.{ts,preview.ts,test.ts}
      src/components/wc-money.{ts,preview.ts,test.ts}
      src/components/wc-empty-state.{ts,preview.ts,test.ts}
      src/components/wc-spinner.{ts,preview.ts,test.ts}
      src/components/__mocks__/nav.ts        # shared NavItem fixtures
      preview/index.html  preview/main.ts  preview/vite.config.ts
      preview/{manifest,router,types,a11y,previews-json-plugin}.ts (+ .test.ts)
      preview/axe-suite.ts                   # NEW: runs axe over every preview state
      preview/app/preview-app.ts
      preview/__fixtures__/{alpha,beta}.preview.ts
  apps/
    app/
      package.json  tsconfig.json  tsconfig.build.json  vite.config.ts
      index.html
      src/main.ts
      src/test-setup.ts
      src/components/nigel-app.ts
      src/mixins/signal-watcher.ts
      src/state/app-store.ts
      src/api/{types,client,index}.ts
      src/screens/registry.ts
      src/screens/hash-route.ts
      src/screens/{dashboard,register,review,import,reports,accounts,
                   categories,rules,reconcile,undo,settings,unlock}.ts
      src/__tests__/{api-seam,dependency-manifest}.test.ts
      src/api/client.test.ts
      src/screens/{registry,hash-route}.test.ts
      src/state/app-store.test.ts
      src/components/nigel-app.test.ts
      src/__mocks__/fake-api-client.ts
build.rs                  # NEW at crate root -- see section 7
```

Root `web/package.json` (pinned; no turbo, explicit ordered build):

```json
{
  "name": "nigel-web", "private": true, "type": "module",
  "workspaces": ["packages/*", "apps/*"],
  "engines": { "node": ">=20.19.0" },
  "scripts": {
    "build": "npm run build -w @nigel/theme && npm run build -w @nigel/ui && npm run build -w @nigel/app",
    "dev": "npm run dev -w @nigel/app",
    "preview": "npm run preview -w @nigel/ui",
    "test": "npm run test --workspaces --if-present",
    "lint": "npm run lint --workspaces --if-present",
    "typecheck": "npm run typecheck --workspaces --if-present",
    "clean": "npm run clean --workspaces --if-present && rm -rf dist"
  },
  "devDependencies": {
    "@eslint/js": "^9.18.0", "eslint": "^9.18.0", "globals": "^15.14.0",
    "typescript": "^5.7.2", "typescript-eslint": "^8.20.0"
  }
}
```

`@nigel/theme` deps: `lit ^3.2.1` (dep + peer), dev `typescript ^5.7.2`,
`vitest ^2.1.8`. Scripts: `build` = `tsc && node scripts/build-css.js`,
`test`, `lint`, `typecheck` = `tsc --noEmit -p tsconfig.json`, `clean`.
Exports map: `.` -> dist/index.js, `./nigel` -> dist/themes/nigel.js,
`./css/nigel.css` -> dist/css/nigel.css.

`@nigel/ui` deps: `lit ^3.2.1`; dev `@nigel/theme "*"`, `axe-core ^4.10.2`,
`jsdom ^27.4.0`, `typescript ^5.7.2`, `vite ^6.0.6`, `vitest ^2.1.8`.
Scripts: `build` = `tsc -p tsconfig.build.json`, `preview` =
`vite --config preview/vite.config.ts`, `test`, `lint`, `typecheck`, `clean`.
Exports map with the `"development"` condition pointing at `./src/*.ts`
(`.`, `./components`, `./icons`) exactly as boxcraft does; no `./tokens`
export -- boxcraft's duplicate-palette mistake is not ported.

`@nigel/app` deps: `@awesome.me/webawesome ^3.1.0`, `@lit-labs/signals ^0.2.0`,
`@nigel/theme "*"`, `@nigel/ui "*"`, `lit ^3.2.1`; dev `@types/node ^22.10.2`,
`jsdom ^27.4.0`, `typescript ^5.7.2`, `vite ^6.0.6`, `vitest ^2.1.8`.
Scripts: `dev` = `vite`, `build` = `tsc -p tsconfig.build.json && vite build`,
`test`, `lint`, `typecheck`, `clean`.

tsconfigs: `web/tsconfig.json` is boxcraft's base (target ES2022, module
ESNext, moduleResolution bundler, strict, isolatedModules, noEmit,
skipLibCheck) plus `paths` for `@nigel/ui`, `@nigel/ui/*`, `@nigel/theme`.
Each package tsconfig extends it and adds `experimentalDecorators: true`,
`useDefineForClassFields: false`, `noUnusedLocals/Parameters`,
`noFallthroughCasesInSwitch`, `allowImportingTsExtensions` (per the epic's
pinned stack; identical to boxcraft's per-package tsconfigs). `tsconfig.build.json`
in ui/app flips `noEmit:false` + declarations and excludes
`**/*.test.ts`, `**/__tests__`, `**/*.preview.ts`, `preview/**`, `**/__mocks__`.

## 2. @nigel/theme

Structure forked from boxcraft: one `css` tagged-template per token category,
composed into a single `CSSResult` in `themes/nigel.ts` in the order
color -> typography -> spacing -> gradient -> radius -> shadow -> motion ->
colorDark -> global (dark last so its higher-specificity selectors win;
global last so ::part overrides can read every token). No `font-faces.ts`:
nigel ships no bundled webfonts (see Open decisions).

Palette derivation from `src/effects.rs` GRADIENT (the seven pastels
#ffb3ba pink, #ffc8a2 peach, #ffe0a3 yellow, #c9ffcb mint, #bae1ff cyan,
#c4b7ff lavender, #ffb3de magenta):

- `gradient.ts` exports `NIGEL_PALETTE` (the seven hexes, in effects.rs order)
  and `gradientCss` defining `--nc-grad-brand` (the full 7-stop 90deg ramp --
  the same ramp the splash logo and the current placeholder page use),
  `--nc-grad-brand-hover` (same ramp, `filter: brightness(1.04)` companion),
  `--nc-grad-brand-soft` (same stops at 18% alpha, for headers/selected rows).
- Pastels are decorative only. Every *solid* interactive/semantic color is a
  darkened derivation that clears WCAG AA, verified by `contrast.test.ts`.

Light (`colorCss`, `:root`) -- surfaces reuse the placeholder page's values so
the built SPA is visually continuous with 31.3's shell:

  --wa-color-bg #fdfcfb; --wa-color-surface #ffffff;
  --wa-color-surface-alt #f6f4fb; --wa-color-border #e6e3f0;
  --wa-color-border-soft #f0eef7; --wa-color-text #2b2b33;
  --wa-color-muted #6b6b7b; --wa-color-brand #6b4ff0 (lavender deepened);
  --wa-color-brand-hover #573ed6; --wa-color-on-brand #ffffff;
  --wa-color-focus #6b4ff0; --wa-color-danger #c2314a (pink deepened);
  --wa-color-success #1f7a44 (mint deepened); --wa-color-warning #a06a10
  (yellow deepened); --wa-color-info #1f6ea8 (cyan deepened);
  plus `color-scheme: light dark`.

Dark (`colorDarkCss`, both `@media (prefers-color-scheme: dark)
:root:not(.light-mode)` and `:root.dark-mode`) -- here the pastels are used
raw, which is where they read best:

  bg #17171d; surface #1f1f28; surface-alt #25252f; border #2e2e3c;
  border-soft #26262f; text #ece9f5; muted #9a97ab; brand #c4b7ff;
  brand-hover #d5cbff; on-brand #17171d; danger #ffb3ba; success #8ee6a0;
  warning #ffe0a3; info #bae1ff.

Nigel-specific `--nc-*` (mirroring src/tui.rs money semantics: positive =
GREEN rgb(80,220,100), negative = red):

  --nc-color-income   light #1f7a44 / dark #7fe0a0
  --nc-color-expense  light #c2314a / dark #ff9fa8
  --nc-color-flagged  light #a06a10 / dark #ffe0a3
  --nc-color-selected-bg  light #f1edff / dark #2a2740  (tui SELECTED_STYLE)
  --nc-font-money     var(--wa-font-family-mono)
  --nc-icon-size      20px
  --nc-sidebar-width  232px
  --nc-sidebar-collapsed-width 56px
  --nc-header-height  48px
  --nc-transition-fast 120ms ease
  --nc-transition-base 200ms ease

typography.ts: `--wa-font-family-sans` = system stack
(ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif);
`--wa-font-family-mono` = ui-monospace, "SF Mono", Menlo, Consolas, monospace;
sizes s 12 / base 14 / lg 16 / xl 20 / 2xl 26px; `--wa-line-height` 1.5.
spacing.ts xs4 s8 m12 l16 xl24 2xl32. radius.ts sm 6 / md 10 / lg 14 / pill.
shadow.ts sm/md/lg tuned for the pale surfaces. motion.ts durations/easings
plus a `@media (prefers-reduced-motion: reduce)` block zeroing them.

global.ts (document-level `::part()` overrides, same pattern as boxcraft):
`wa-button[variant=brand|primary]::part(base)` gets `--nc-grad-brand`,
white text, transparent border, hover brightness, active translateY(1px);
`wa-button::part(label)` mono + letter-spacing; `::part(form-control-label)`
mono across wa-input/select/switch/checkbox/radio/textarea;
`wa-dialog::part(header|body)` surface + border tokens; focus-visible ring
using `--wa-color-focus`.

scripts/build-css.js: copied verbatim in shape -- imports
`dist/themes/nigel.js`, writes `dist/css/nigel.css` = banner + cssText.

## 3. @nigel/ui

Preview harness copied from `packages/ui/preview/` with `@boxcraft` ->
`@nigel` renames: `types.ts`, `manifest.ts` (import.meta.glob over
`../src/**/*.preview.ts` + pure `collectPreviews`), `router.ts` (query-string
`?preview=&state=&mode=preview`), `previews-json-plugin.ts`,
`app/preview-app.ts`, `main.ts`, `index.html`, `vite.config.ts` on
**port 9090** with aliases to `../../theme/dist/css/nigel.css` and
`../../theme/src/index.ts`. Fixtures under `preview/__fixtures__/` kept.

Addition over boxcraft: `preview/axe-suite.ts` exporting
`describePreviewA11y(preview: Preview)` which mounts each `preview.states[i]`
into a fresh document.body and asserts `axe.run(..., {resultTypes:['violations']})`
is empty. Boxcraft hand-copies state setup into its axe tests and drifts;
this makes workflow step 3 automatic -- every component test file is one call.

`src/test-setup.ts` copied verbatim (axe `region` rule off, ElementInternals
shims, HTMLDialogElement showModal/show/close shims) -- both shims are load-
bearing for `wc-confirm` under jsdom.

Icons: `icons/icon-base.ts` = boxcraft's `WcIconBase` with `--wc-icon-size`
renamed `--nc-icon-size` (default 20px), abstract `renderIcon(): SVGTemplateResult`,
`size`/`label` props, `role=img|presentation` + `aria-hidden` driven by `label`.
Starter set in `icons/icons.ts`, 24x24 stroke-2 currentColor paths authored
in-house: wc-icon-dashboard, -register, -review, -import, -report, -account,
-category, -rule, -reconcile, -undo, -settings, -lock (one per screen) plus
-search, -close, -check, -flag, -chevron-left, -chevron-right.
`icons.preview.ts`: one preview, states `grid` (all icons, labelled),
`sizes` (16/20/28 via --nc-icon-size), `colored` (inherits currentColor).
`icons.test.ts`: every registered tag renders an `<svg>`; `label` flips
role/aria-hidden; `describePreviewA11y`.

Components (each: `wc-foo.ts` + `wc-foo.preview.ts` + `wc-foo.test.ts`,
canonical file skeleton types -> @customElement -> static styles -> @property
-> handlers -> render -> HTMLElementTagNameMap; `reflect: true` only where CSS
keys off it; variants as `:host([variant])`):

1. **wc-app-shell** -- layout frame only (boxcraft's also switched modules;
   nigel's routing lives in the app, so the shell is dumb).
   props: `appName='Nigel'`, `screenTitle=''`, `sidebarCollapsed` (bool, reflect).
   slots: `sidebar`, `header-actions`, default (content), `banner` (reauth /
   update notices). events: `nc-sidebar-toggle`. Renders one `<wc-toast>`
   internally. WA imports: none.
   preview states: default, with-header-actions, collapsed, with-banner,
   empty-content. axe each.

2. **wc-nav-sidebar**
   props: `items: NavItem[]` (`{id,label,icon?,disabled?}`), `active: string`,
   `collapsed` (bool, reflect). events: `nc-navigate` `{id}` (suppressed for
   disabled items), `nc-sidebar-toggle`.
   markup: `<nav aria-label="Primary"><ul><li><button aria-current="page"
   aria-disabled=...>` with `<wc-icon-*>` + `.nav-label`; collapsed keeps the
   label as an aria-label + title.
   preview states: expanded, collapsed, with-disabled, long-list, no-icons.

3. **wc-toast** -- region + typed bus.
   exports `NC_TOAST_EVENT = 'nc-toast'`, `NcToastVariant = 'info'|'success'|'danger'`,
   `NcToastAction {label,onClick}`, `NcToastDetail {message,variant?,duration?,action?}`,
   `dispatchNcToast(target, detail)`, `HTMLElementEventMap` augmentation.
   Carries over boxcraft's popover top-layer trick verbatim (`popover="manual"`
   + hide/show on each new toast so it re-stacks above `wa-dialog`'s showModal),
   `role=alert`/`aria-live=assertive` for danger vs `status`/`polite`,
   defaults 4000ms / 8000ms with an action, `<=0` disables auto-dismiss,
   invalid detail logged and ignored.
   Departure: the region listens on **window** (events are bubbles+composed) so
   a toast fires correctly from inside a `wa-dialog` top-layer subtree or any
   component not nested under the shell. Boxcraft listens on the shell host.
   props: `initial?: NcToastDetail` (seeds one toast on first update -- lets
   preview states and axe render a visible toast without timing games).
   preview states: info, success, danger, with-action, long-message.
   tests: bus delivers, variant maps to role, fake-timer auto-dismiss, action
   click invokes + dismisses, dismiss on disconnect, axe per state.

4. **wc-confirm** -- the `window.confirm` replacement (wa-dialog wrapper).
   WA imports: `dist/components/dialog/dialog.js`, `dist/components/button/button.js`.
   props: `open` (bool, reflect), `heading`, `message`, `confirmLabel='Confirm'`,
   `cancelLabel='Cancel'`, `variant: 'default'|'danger'`.
   events: `nc-confirm`, `nc-cancel`. Also exports
   `confirmDialog(opts): Promise<boolean>` -- mounts, awaits, removes -- the
   ergonomic call site for 31.16/31.17 destructive actions.
   preview states: closed, open-default, open-danger, long-message.
   tests: open/close, both events, promise resolves true/false, Esc = cancel,
   focus lands on the cancel button for `danger`, axe per state.

5. **wc-money** -- the SPA's `money_span`.
   props: `amount: number`, `currency='USD'`, `locale?`, `variant:
   'signed'|'plain'` , `zeroNeutral=true`, `align: 'end'|'start'`.
   Renders `<span part="amount">` in `--nc-font-money` with
   `font-variant-numeric: tabular-nums`; negative -> `--nc-color-expense`,
   positive -> `--nc-color-income`, zero -> `--wa-color-muted` when zeroNeutral.
   Formatting via `Intl.NumberFormat(locale, {style:'currency',currency})`,
   which produces `-$500.00` / `$1,234.56` -- byte-identical to `fmt::money`
   for USD (verified against fmt.rs's own test vectors, which are reused).
   Departure from money_span: the sign is rendered as a literal `-`, not
   conveyed by color alone (WCAG 1.4.1); the TUI can rely on color, a browser
   cannot.
   preview states: positive, negative, zero, large, cents, column (five rows
   right-aligned, showing tabular alignment).
   tests: the fmt.rs vectors ($1,234.56 / -$500.00 / $0.00 / $1,000,000.99 /
   $42.10) with locale pinned to en-US, color class per sign, axe per state.

6. **wc-empty-state**
   props: `heading`, `message`, `icon` (tag name, optional); slots `actions`,
   default. preview states: default, with-action, with-icon, compact.

7. **wc-spinner**
   props: `size: 's'|'m'|'l'`, `label='Loading'`, `inline` (bool).
   `role="status"` + `aria-live="polite"` + visually-hidden label; animation
   suppressed under `prefers-reduced-motion`.
   preview states: small, medium, large, inline-with-text, in-card.

`src/components/index.ts` re-exports every component class + its types +
`dispatchNcToast` + `confirmDialog`; `src/index.ts` re-exports components +
icons (no tokens export).

## 4. @nigel/app

`index.html`: `<nigel-app>` + `<script type="module" src="/src/main.ts">`,
plus a tiny reset reading `--wa-color-bg/-text/-font-family-sans` so the page
is themed before the first Lit paint.

`src/main.ts` (3 lines, boxcraft's bootstrap shape):
`import '@nigel/theme/css/nigel.css'; import '@nigel/ui'; import './components/nigel-app.js';`

`src/mixins/signal-watcher.ts`: `export { SignalWatcher, signal, computed, Signal }
from '@lit-labs/signals';` -- the single seam; nothing else in the app imports
`@lit-labs/signals` directly (checked by lint convention + reviewed in code
review, same as boxcraft).

`src/state/app-store.ts` -- boxcraft's project-store pattern: module-scope
`Signal.State` privates, a `ReadonlySignal<T> {get(): T}` view interface, a
`AppStore` interface listing signals + computed + actions, an
`initializeAppStore(client: ApiClient): AppStore` and a `getAppStore()` that
throws if uninitialized.

  status: ReadonlySignal<StatusResponse | null>
  statusLoading: ReadonlySignal<boolean>
  statusError: ReadonlySignal<ApiError | null>
  locked: Signal.Computed<boolean>        // status?.locked ?? false || appLocked.get()
  initialized: Signal.Computed<boolean>
  companyName: Signal.Computed<string>    // status.companyName ?? 'Nigel'
  version: Signal.Computed<string>
  actions: refreshStatus(), unlock(password) (returns
  {ok:true} | {ok:false, attemptsRemaining, retryAfterMs} -- 31.10 consumes it)

`src/screens/registry.ts` -- the deliberate departure from boxcraft's
three-place switch. Single source of truth for nav, header title and content:

  export type ScreenId = 'dashboard'|'register'|'review'|'import'|'reports'
    |'accounts'|'categories'|'rules'|'reconcile'|'undo'|'settings'|'unlock';
  export interface ScreenDef { id; title; navLabel; icon; inNav: boolean;
    render(): TemplateResult }
  const DEFS: Record<ScreenId, ScreenDef> = {...}   // compile-time exhaustive
  export const SCREENS: ReadonlyMap<ScreenId, ScreenDef> = new Map(Object.entries(DEFS)...)
  export const DEFAULT_SCREEN: ScreenId = 'dashboard';
  export function isScreenId(v: string): v is ScreenId
  export function navItems(): NavItem[]   // SCREENS filtered by inNav, order preserved

`unlock` is `inNav: false` (reached only by the locked gate). The 11 others
appear in the sidebar in dashboard, register, review, import, reports,
accounts, categories, rules, reconcile, undo, settings order. Adding a screen
in 31.10-31.17 means editing one object literal; the `Record<ScreenId,...>`
type makes a missing entry a compile error.

`src/screens/hash-route.ts` -- pure, DOM-free, unit-tested:

  export interface Route { screen: ScreenId; params: URLSearchParams }
  export function parseHash(hash: string): Route     // '#/register?year=2025'
  export function routeToHash(route: Route): string  // unknown/empty -> DEFAULT_SCREEN

Params are parsed and preserved now (not used by the scaffold) so 31.12+ can
deep-link `#/register?account=BofA%20Checking` without changing the seam.

`src/screens/*.ts` -- 12 stubs, each a single exported render function
returning `<wc-empty-state heading=... message="Arrives in task 31.NN.">`.
`unlock.ts` instead explains that the database is encrypted and points at
31.10. Screen tasks replace the function body only.

`src/components/nigel-app.ts` -- the smart container, `SignalWatcher(LitElement)`:
- connectedCallback: `const client = new FetchApiClient()`,
  `initializeAppStore(client)`, `void store.refreshStatus()`,
  `window.addEventListener('hashchange', ...)`, seed `this.route` from
  `location.hash` (writing the default hash when empty).
- render precedence: no status yet -> `<wc-spinner label="Connecting">`;
  `store.locked` -> the unlock screen inside the shell with every nav item
  disabled, *without* rewriting the hash (so 31.10 can return the user to
  where they were); `appUnauthorized` -> the shell with a `banner` slot
  explaining the session expired and to reopen the URL `nigel serve` printed
  (the SPA cannot mint a token); otherwise
  `<wc-app-shell screenTitle=...><wc-nav-sidebar slot="sidebar" .items=${navItems()}
  active=${route.screen}>...</wc-app-shell>` with
  `SCREENS.get(route.screen)!.render()` in the default slot.
- `nc-navigate` -> sets `location.hash = routeToHash(...)`. hashchange is the
  only writer of route state (single direction, back button works).
- `document.title = ${def.title} - ${companyName}`.
- statusError -> a danger `dispatchNcToast` once, plus a retry button.

## 5. Api client seam

`src/api/types.ts` (initial content -- exactly the three landed endpoints;
API tasks append, one interface per Rust response struct, same name):

  export interface PingResponse { ok: boolean; version: string }
  export interface StatusResponse { initialized: boolean; encrypted: boolean;
    locked: boolean; companyName: string | null; version: string; dataDir: string }
  export interface UnlockRequest { password: string }
  export interface UnlockResponse { locked: boolean }
  export const API_ERROR_CODES = ['bad_request','unauthorized','invalid_password',
    'forbidden','not_found','conflict','locked','internal','feature_disabled'] as const;
  export type ApiErrorCode = (typeof API_ERROR_CODES)[number] | 'unknown';
  export interface ApiErrorEnvelope { error: { code: string; message: string; details?: unknown } }
  export interface InvalidPasswordDetails { attemptsRemaining: number; retryAfterMs: number }

`'unknown'` is the forward-compatibility escape: an unrecognized code from a
newer server (31.7's `payload_too_large`) normalizes to `'unknown'` with the
literal string kept on `rawCode`, instead of crashing or silently type-lying.

`src/api/client.ts`:

  export class ApiError extends Error {
    readonly code: ApiErrorCode; readonly rawCode: string;
    readonly status: number; readonly details?: unknown;
    get isLocked(); get isUnauthorized();
    invalidPasswordDetails(): InvalidPasswordDetails | null;
  }
  export interface ApiClient {
    ping(): Promise<PingResponse>;
    getStatus(): Promise<StatusResponse>;
    unlock(password: string): Promise<UnlockResponse>;
  }
  export class FetchApiClient implements ApiClient {
    constructor(opts?: { baseUrl?: string; fetchImpl?: typeof fetch })  // baseUrl '/api'
    private request<T>(method, path, body?): Promise<T>
  }
  export const appLocked: Signal.State<boolean>;
  export const appUnauthorized: Signal.State<boolean>;

**Method granularity:** one method per endpoint, named after the endpoint;
multi-parameter routes (31.5+) take a single typed options object
(`getPnl(params: ReportParams)`). No generic `request()`/`get()` on the public
`ApiClient` interface -- exposing one would let screens hand-roll paths and
defeat the whole point of the seam (a Tauri `InvokeApiClient` has no URLs to
give them). This rule is written into web/README.md for 31.10-31.17.

`request()` normalization:
1. `fetchImpl(baseUrl + path, {method, credentials:'same-origin',
   headers:{'Content-Type':'application/json'}, body: body && JSON.stringify(body)})`.
   `same-origin` is correct both when served by the binary and behind the vite
   proxy (the cookie is set on the vite origin by the proxied `/auth` 302).
2. fetch rejection -> `ApiError('unknown', status 0, "Could not reach the
   nigel server.")` -- distinguishable from any server answer.
3. non-2xx: parse the body as JSON; if it matches the envelope, take
   code/message/details, mapping unrecognized codes to `'unknown'`; otherwise
   derive the code from the status (400/401/403/404/409/423/500/501) and use
   `res.statusText`.
4. transport signal wiring, the whole reason these live here:
   - `status === 423` -> `appLocked.set(true)`
   - `status === 401 && code !== 'invalid_password'` -> `appUnauthorized.set(true)`
     (the exclusion matters: a wrong unlock password is also a 401 and must
     not raise the "session expired" banner -- that is the unlock form's business)
   - any 2xx -> `appUnauthorized.set(false)`; `getStatus()` additionally drives
     `appLocked.set(body.locked)` so the signal has exactly one authority
5. 204 / empty body -> resolves `undefined as T`.

`src/api/index.ts` re-exports types, ApiError, ApiClient, FetchApiClient and
the two signals.

Guard `src/__tests__/api-seam.test.ts`: walks `src/**/*.ts` excluding
`src/api/`, fails on any match of `/\bfetch\s*\(/`, `/new\s+XMLHttpRequest/`,
`/new\s+EventSource/`, `/new\s+WebSocket/`, `navigator.sendBeacon`, reporting
file + line. Test files are in scope -- a screen test that reaches for fetch
instead of a fake client is exactly the drift this prevents;
`src/__mocks__/fake-api-client.ts` implements `ApiClient` and is what tests use.

## 6. Vite / build integration

`web/apps/app/vite.config.ts`:

  root: __dirname
  build: { outDir: resolve(__dirname,'../../dist'), emptyOutDir: true,
           target: 'esnext', sourcemap: false }     // emptyOutDir explicit --
           // vite refuses to clear a dir outside root otherwise
  resolve.alias (array form, specific first):
    '@nigel/theme/css/nigel.css' -> ../../packages/theme/dist/css/nigel.css
    '@nigel/theme'               -> ../../packages/theme/src/index.ts
    '@nigel/ui'                  -> ../../packages/ui/src
  server: { port: 5173, strictPort: true, proxy: {
    '/api':  { target: 'http://127.0.0.1:5731', changeOrigin: true },
    '/auth': { target: 'http://127.0.0.1:5731', changeOrigin: true } } }
  test: { environment:'node', globals:true, setupFiles:['./src/test-setup.ts'],
    include:['src/**/*.test.ts'],
    environmentMatchGlobs: [['**/components/**','jsdom'], ['**/state/**','jsdom'],
      ['**/api/**','jsdom']],
    deps:{optimizer:{web:{include:['lit','@lit-labs/signals']}}} }

`base` stays `/` (assets land at `/assets/...`, which the rust-embed handler
serves directly; only unknown paths fall through to index.html).

`src/test-setup.ts` in the app mirrors the ui one (ElementInternals + dialog
shims) since screens mount `@nigel/ui` components under jsdom.

## 7. The committed-fallback mechanism (and the rust-embed staleness trap)

Problem with the status quo: `web/dist/index.html` is committed *and* is
where vite writes. Every `npm run build` would leave the tree dirty and every
`git pull` could clobber a local build.

Mechanism:
- Move the placeholder to `web/placeholder/index.html` (committed; wording
  updated to "SPA not built -- run `npm run build` in web/"). Keeps the
  existing pastel gradient styling so the fallback still looks like nigel.
- `web/dist/` is fully gitignored (root `.gitignore` gains `web/dist/`).
- New crate-root `build.rs`:
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rerun-if-changed=web/dist/index.html");
    println!("cargo:rerun-if-changed=web/placeholder/index.html");
    // create web/dist if absent; copy the placeholder in only when
    // web/dist/index.html does not exist -- never clobbers a real build
  Runs unconditionally (cheap; a few stat calls) so
  `cargo test --no-default-features`, which never compiles rust-embed, is
  unaffected.

This build script does double duty. **rust-embed's `debug-embed` does not make
cargo rebuild when `web/dist` changes** -- it only means debug builds embed
rather than read from disk. Without a build script emitting
`cargo:rerun-if-changed`, a fresh `npm run build` followed by `cargo run`
serves the *previously embedded* bytes. This is a real, currently-latent bug in
31.3's setup that 31.9 must fix, and the same build.rs is the fix.
(Caveat noted in web/README.md: `rerun-if-changed` on a directory tracks that
directory's mtime, so nested-only edits under `assets/` would not trigger --
but vite rewrites `index.html` on every build, which is tracked explicitly.)

## 8. Dev workflow (documented verbatim in web/README.md + CLAUDE.md)

One-time: `cd web && npm ci`

Terminal 1:
    cargo run -- serve --no-open
    # prints e.g. http://127.0.0.1:5731/auth?token=1f3a...

Terminal 2:
    cd web && npm run dev            # vite on http://localhost:5173

Browser: take the token from terminal 1 and open it **on the vite origin**:
    http://localhost:5173/auth?token=1f3a...

Why that works end to end:
- vite proxies `/auth` to 5731 with `changeOrigin: true`, so the backend sees
  `Host: 127.0.0.1:5731` and passes its Host check. (It would also pass
  without changeOrigin -- nigel's rule accepts `localhost` on *any* port,
  which is exactly what makes a dev proxy possible.)
- The `Set-Cookie` for `nigel_session` carries no `Domain`, so the browser
  stores it host-only for `localhost:5173`, the origin the user is actually on.
- The 302 target is `/`, a relative redirect, so the browser stays on 5173 and
  vite serves the SPA with HMR.
- Subsequent `/api` XHRs are same-origin from the browser's view; the cookie
  rides along (`credentials: 'same-origin'`), the proxy forwards it, and the
  `SameSite=Strict` attribute is satisfied because everything is one site.
- Restarting `nigel serve` mints a new token and invalidates the cookie: the
  next `/api` call 401s, `appUnauthorized` flips, and the shell's banner tells
  the user to reopen the newly printed URL. (This is why the scaffold carries
  `appUnauthorized` even though there is no login screen.)

Component work: `cd web && npm run preview` -> http://localhost:9090.
Full embedded check: `cd web && npm run build` then `cargo run -- serve`.

## 9. CI

`.github/workflows/ci.yml`, `check` job -- inserted after checkout and
**before** every cargo step, so the embed always sees fresh dist:

    - uses: actions/setup-node@v4
      with: { node-version: 22, cache: npm, cache-dependency-path: web/package-lock.json }
    - name: Install web dependencies
      run: npm ci
      working-directory: web
    - name: Lint web
      run: npm run lint
      working-directory: web
    - name: Test web
      run: npm test
      working-directory: web
    - name: Build web
      run: npm run build
      working-directory: web

Runs on both matrix legs (ubuntu + macos). The existing six cargo steps are
unchanged.

`.github/workflows/release.yml` needs the **same node setup + `npm ci` +
`npm run build`** added to *both* build jobs (the macOS universal job before
its two `cargo build --release` calls, and the linux/windows matrix job before
its `cargo build --release`). Without this every shipped binary would embed
the "SPA not built" placeholder -- the release workflow currently has no node
step at all. This is not optional and is part of 31.9.

## 10. Docs

- **CLAUDE.md**
  - Architecture: a `**Web UI (SPA):**` bullet -- `web/` npm workspace
    (@nigel/theme, @nigel/ui, @nigel/app; no turbo), the screen registry +
    hash routing, the `src/api/` seam and its two transport signals, the
    `dispatchNcToast` bus, `wc-confirm` over `window.confirm`, `--wa-*`
    shadowing + `--nc-*` tokens, preview harness on 9090.
  - New `## Component-First UI Workflow (MANDATORY)` section carried over from
    boxcraft's CLAUDE.md (4 steps, pre-merge checklist, component selection,
    where-things-live table retargeted at the three nigel packages).
  - Commands: a `### Web UI` block -- npm ci / build / dev / preview / test /
    lint, plus the two-terminal dev loop with the tokenized URL step.
  - Project Structure: expand the `web/` subtree per section 1 and add `build.rs`.
  - Key Design Constraints: (a) every server call goes through
    `web/apps/app/src/api/` -- enforced by a guard test; (b) screen identity
    lives only in the registry, `location.hash` is the only route writer;
    (c) no `window.confirm`; (d) `web/dist` is generated and gitignored,
    `web/placeholder/index.html` + `build.rs` keep `cargo build` working
    without node, and `build.rs` is what makes cargo notice a fresh SPA build;
    (e) 401 `invalid_password` must never be treated as a session failure.
- **README.md**: a "Building from source" note -- Node 20.19+ (22 recommended),
  `cd web && npm ci && npm run build` before `cargo build --release`, otherwise
  the binary serves the placeholder page; one line in Features that the web UI
  follows the system light/dark preference.
- **web/README.md** (new): workspace layout, the mandatory component-first
  workflow, the exact dev loop, preview harness, token reference table, the
  api-client rules for screen tasks (one method per endpoint, options object,
  no generic request, types mirror docs/api.md), test/lint commands, and the
  dist/placeholder/build.rs mechanism with the rerun-if-changed caveat.
- **docs/api.md**: three lines under Conventions pointing at
  `web/apps/app/src/api/types.ts` as the TS mirror that API tasks keep in sync.

## 11. Test plan

@nigel/theme (node env, 4 files):
- nigel-theme.test.ts -- contract by string match on the composed cssText:
  every required `--wa-*` and `--nc-*` token name present; `prefers-color-scheme:
  dark`, `.dark-mode`, `.light-mode` present; `wa-button`, `::part(base)`,
  `::part(label)`, `::part(form-control-label)`, `wa-dialog` present;
  token order (color before colorDark before global).
- palette-parity.test.ts -- reads `src/effects.rs` from the repo root, extracts
  the hexes from the GRADIENT const block, asserts the set equals
  `NIGEL_PALETTE`. Keeps the derivation load-bearing rather than decorative.
- contrast.test.ts -- pure WCAG relative-luminance helper; asserts >= 4.5:1 for
  text/bg, text/surface, muted/bg, on-brand/brand, income/bg, expense/bg,
  income/surface, expense/surface, in **both** light and dark. This is what
  keeps the pastel palette honest.
- build-css.test.ts -- adapted from boxcraft: after build, `dist/css/nigel.css`
  exists and contains the banner + the cssText.

@nigel/ui (jsdom, ~12 files): every component test file calls
`describePreviewA11y(preview)` (axe, zero violations, per preview state) plus
behavior tests -- sidebar (event emitted / suppressed for disabled,
aria-current, collapsed labels), toast (bus delivery, role by variant,
fake-timer dismissal, action click, invalid detail ignored), confirm (events,
promise resolution, Esc, focus), money (the five fmt.rs vectors, sign color,
tabular-nums, zero-neutral), spinner (role/aria-live, reduced-motion),
empty-state (slot rendering), icons (svg present, label -> role img).
Harness tests ported: manifest (collectPreviews sort), router (parse/serialize),
previews-json-plugin (extract), types, a11y.

@nigel/app (~7 files):
- api/client.test.ts (jsdom, stubbed fetch): 2xx JSON; envelope error ->
  typed ApiError (code/status/message/details); 423 -> appLocked true;
  401 unauthorized -> appUnauthorized true; **401 invalid_password ->
  appUnauthorized stays false** + `invalidPasswordDetails()` typed;
  unrecognized code -> 'unknown' with rawCode preserved; network rejection ->
  status 0; non-JSON error body -> status-derived code; asserts
  `credentials:'same-origin'` and the `/api` base on the request init;
  getStatus drives appLocked from the body.
- __tests__/api-seam.test.ts -- the no-fetch-outside-src/api guard.
- __tests__/dependency-manifest.test.ts -- boxcraft's walk, `@nigel/*` edition.
- screens/registry.test.ts -- every ScreenId resolves; navItems excludes
  unlock and preserves order; titles unique; every icon is a registered tag.
- screens/hash-route.test.ts -- round-trip, unknown -> dashboard, empty ->
  dashboard, params preserved, malformed input safe.
- state/app-store.test.ts -- refreshStatus populates; failure sets statusError;
  `locked` reacts to both status.locked and appLocked; getAppStore before init
  throws; unlock success/failure outcomes.
- components/nigel-app.test.ts -- spinner before status; shell + sidebar after;
  hashchange swaps screens; nc-navigate writes location.hash; locked status
  forces the unlock screen without rewriting the hash; unauthorized renders the
  banner. Driven entirely by `FakeApiClient implements ApiClient`, which is
  itself the proof the seam is swappable for Tauri.

## 12. Implementation order (one commit per phase)

1. Workspace skeleton, root configs, eslint, `build.rs`, placeholder move,
   `.gitignore`. Gate: `cargo build` green with no node installed.
2. @nigel/theme complete with its four test files.
3. @nigel/ui: harness + axe-suite + icons, then the seven components strictly
   preview -> axe test -> implementation, per the mandatory workflow.
4. @nigel/app: api seam first (types -> client -> tests -> guards), then store,
   registry + hash-route, nigel-app, the 12 stubs.
5. Build integration + dev proxy + the end-to-end serve check.
6. CI + release workflow + docs.

## 13. Verification

web/ (from `web/`):
1. npm ci
2. npm run lint
3. npm test              (all three workspaces green)
4. npm run typecheck
5. npm run build         -> web/dist/index.html + web/dist/assets/* exist
6. npm run preview       -> :9090 renders every state of every component (manual)

cargo (the 8-command matrix):
1. cargo fmt --check
2. cargo build
3. cargo test
4. cargo test --no-default-features
5. cargo test --no-default-features --features serve
6. cargo clippy --all-targets -- -D warnings
7. cargo clippy --no-default-features --all-targets -- -D warnings
   -- expected to report exactly the 2 known task-34 needless_return lints
   (cli/dashboard.rs:852, cli/report/mod.rs:160) and nothing else
8. End to end: `cargo run -- serve --no-open`, open the printed /auth?token=
   URL. Confirm: the real SPA shell (not the placeholder); 11 sidebar entries;
   clicking each swaps content and the hash; `#/register` deep-links and
   survives a reload (rust-embed SPA fallback); browser back returns to the
   previous screen; a toast renders; toggling the OS light/dark preference
   restyles live. Plus `curl -sI 127.0.0.1:5731/assets/<hash>.js` -> 200 with a
   JS content type (proves non-index assets embed).

Fallback path: `rm -rf web/dist && touch build.rs && cargo build && cargo run --
serve --no-open` -> the "SPA not built" placeholder renders, nothing panics.
Staleness path: `npm run build` then `cargo run -- serve` without touching any
.rs file -> the new bundle is served (proves build.rs's rerun-if-changed works).
<!-- SECTION:PLAN:END -->
