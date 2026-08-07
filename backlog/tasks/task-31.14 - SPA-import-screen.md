---
id: TASK-31.14
title: 'SPA: import screen'
status: Done
assignee: []
created_date: '2026-08-06 16:27'
updated_date: '2026-08-07 14:58'
labels:
  - web
  - frontend
dependencies:
  - TASK-31.7
  - TASK-31.9
documentation:
  - docs/superpowers/specs/2026-08-06-task-31.14-import-screen.md
  - docs/superpowers/specs/2026-08-06-epic-31-architecture.md
parent_task_id: TASK-31
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Browser import flow: drag-and-drop plus file picker upload, account selector, optional format override including saved CSV profiles and a generic column-mapping form with save-as-profile, dry-run preview showing detected format, sample rows, and counts, then confirm and show results with a link into review for newly flagged transactions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Upload, preview, confirm flow works end to end with detected format and sample rows shown before anything is written
- [x] #2 Account selector, format override, generic CSV column mapping, and save-profile are available
- [x] #3 Results show imported, skipped, malformed, and flagged counts with a link into review
- [x] #4 Duplicate files and duplicate rows are surfaced clearly
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Spiked `wa-format-bytes` and `wa-input type=number` under jsdom before building on
  them; both survive (`8.21 kB`, value round-trips). Used `wa-format-bytes` in
  `wc-dropzone` instead of the hand-rolled `formatBytes` the plan proposed —
  WA-primitive-first, and it deletes a helper.
- Web Awesome's `wa-select` has no option-group element, so the format list is flat:
  built-ins by name, saved profiles prefixed `Saved:`, then `Generic CSV…`.
- Followed the landed screen convention (`screens/foo.ts` holding both the element and
  `renderFoo`, plus `foo-data.ts` for pure logic), not the `foo-screen.ts` split named
  in the handoff — the four landed screens all do it the first way.
- `screens/registry.ts` needed no edit: `renderImport(ctx)` already matched
  `ScreenDef.render`, so the collision the plan predicted did not happen.
- `wc-dropzone`'s private `inert` getter collided with `HTMLElement.inert` and broke the
  Lit decorators; renamed to `blocked`.
- A landed client test used `payload_too_large` as its example of an *unrecognized*
  error code. Adding it to `API_ERROR_CODES` falsified that test, so it now uses a
  genuinely unknown code and keeps its original intent.
- Live end-to-end against `cargo run -- serve` on a demo database confirmed every branch
  the screen codes against: detect, explicit format, generic mapping + saveProfile
  round-tripping into `/api/csv-profiles`, duplicate file (200, `duplicateFile: true`,
  null format), a consumed upload answering 404 with
  `details.reason = upload_not_found` (which is what the silent re-upload depends on),
  413 on 30 MB, 400 on `.txt`, 400 on format+mapping together, and 404 on an unknown
  account. Gusto is compiled into this build, so the 501 path is covered by unit test
  with an injected error rather than live.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Adds the browser import flow: choose a statement, preview what it would do, confirm.
Front end for 31.7's upload/preview/confirm pipeline, and the last screen before the
epic's manager screens.

## Behaviour

One screen whose panels appear as the decision is made. Four choices are worth naming
because they are not obvious from the diff:

- **The upload is lazy.** Choosing a file sends nothing; Preview uploads and dry-runs in
  one action. A file picked and thought better of never reaches the server's spool, and
  an upload before an account is chosen could not be used anyway. The `uploadId` is
  cached against the file, so fixing a column mapping and previewing again costs one
  request rather than re-sending the bytes.
- **An expired upload re-uploads itself once.** The spool clears after an hour, which a
  preview left open over lunch will find. The file is still in the browser, so the
  honest response is to send it again rather than make someone re-choose it.
- **A duplicate file blocks the confirm.** The server answers 200 with zero counts, so a
  button there would offer a no-op.
- **Format and mapping are one field.** `importRequestBody` derives both from the single
  format choice, so the pair the API refuses with a 400 is unrepresentable rather than
  merely avoided; `saveProfile` likewise cannot travel without the mapping it names.

Failures land where their cause is — 413 under the dropzone, 501 and other 400s under
the format select, a mapping 400 under the mapping form, an unknown account under the
account select plus a toast. 423 and 401 land nowhere: the shell gates both before this
screen is constructed, and the test file says so, so nobody adds them back.

## Changes

- `@nigel/ui`: `wc-dropzone` (drag-and-drop plus picker; the well is a `<button>` so
  keyboard and mouse share one path, and it checks extension and size client-side
  because the server can only answer 413 after 25 MB have crossed the wire),
  `wc-import-form` (account, format and the generic mapping in one component, following
  `wc-register-toolbar`), `wc-sample-table` (not `wc-register-table`, which is a
  `role="grid"` needing an `id`, `categoryId` and `isFlagged` a parsed row does not
  have), `wc-count-grid` (labelled integers, deliberately not the money-formatting
  `wc-stat-card`). Each with a `.preview.ts` and an axe-driven test.
- `apps/app`: `screens/import.ts` (`nigel-import-screen` + `renderImport`) and
  `screens/import-data.ts` (request bodies, counts, format labels, error routing).
- api seam: five methods — `uploadImport`, `previewImport`, `confirmImport`,
  `getImportFormats`, `getCsvProfiles` — plus nine types, `payload_too_large` in
  `API_ERROR_CODES`, and `ApiError.isUploadExpired`. `request()` gains a three-line
  FormData branch that omits the JSON content type, because only the browser can
  generate the multipart boundary; a parallel `upload()` method was rejected since it
  would have duplicated error normalization and the transport signals.
- `uploadImport` takes a `File` and no progress callback: `fetch` cannot report upload
  progress, and promising it on the interface would oblige a Tauri or remote client to
  invent it. The screen shows an indeterminate spinner; `web/README.md` records where a
  determinate implementation would live.

`screens/registry.ts` needed no change — `renderImport(ctx)` already matched.

## Tests

64 new component tests (23 axe states), 29 on the pure logic, 24 driving the whole
screen through `FakeApiClient`, 6 on the client. The fake really consumes an upload on
confirm, so reusing a spent id fails the way the server fails, which is what makes the
retry path testable. Full workspace: 914 passing.

## Verification

`npm run lint`, `typecheck`, `test`, `build` all green. No Rust changed; the hook's
`cargo test --no-default-features -- --test-threads=1` run as a no-regression check,
26 passing. Manual end-to-end against `cargo run -- serve` on a demo database exercised
detect, explicit format, generic mapping with save-profile, duplicate file, expired
upload, 413, `.txt`, format+mapping, and unknown account; the embedded bundle was
confirmed to contain the new screen. Gusto is compiled into this build, so the 501 path
is covered by unit test rather than live.
<!-- SECTION:FINAL_SUMMARY:END -->
