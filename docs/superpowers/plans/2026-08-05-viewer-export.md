# Report Exports in the Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export any report for the exact period being viewed (task-26): the report viewer gains export keys, the standalone export picker/format screens are deleted, the menu entry becomes "View Reports", and Export All survives as a secondary function of the picker.

**Architecture:** The dashboard's `ReportView` screen arm intercepts `e` (PDF) / `t` (text) before delegating to `view.handle_key`, and calls the existing `do_export`/`do_text_export(idx, year, month)` helpers with `current_report_idx` + `view.date_params()` — the trait method built for exactly this. `ReportPickerMode`, `EXPORT_TYPES`, and the `ExportFormatPicker` screen are removed; the single picker gains a final "Export All Reports" entry.

**Tech Stack:** Rust, ratatui, existing `cli/report/export.rs` + `cli/report/text.rs` helpers.

## Global Constraints

- Direction and ACs: backlog task-26 (option b, chosen by the human). CLI report flags (`--mode export`, `--format`, `--output`, `report all`) unchanged.
- `pdf` is a default feature but can be off: with `--no-default-features`, `e` (PDF) must be absent/harmless and `t` (text) still works — mirror the existing `EXPORT_FORMATS` cfg pattern with cfg-gated key handling.
- Conventional Commits; `cargo test --features pdf` and `cargo test --no-default-features --features gusto` pass; `cargo clippy -- -D warnings` and `cargo fmt --check` clean.
- Docs describe current behavior only (CLAUDE.md dashboard/menu bullets, README if it mentions the export flow).
- TUI paths have no headless harness: extract logic that can be pure (key→export-action mapping) and unit-test it; everything else gets a documented manual verification step in the report.

---

## File Structure

- `src/cli/dashboard.rs` — **modify**: viewer export keys + status message; delete `ReportPickerMode`, `EXPORT_TYPES`, `EXPORT_FORMATS`, `ExportFormatPicker`; picker gains "Export All Reports" entry; menu entry renamed "View Reports"; home-screen `e` shortcut retired or remapped to the picker.
- `src/tui.rs` — **modify** (only if footer hints live there): viewer footer gains `e export pdf · t export text` hints.
- `CLAUDE.md`, `README.md` — **modify**: dashboard/menu/export descriptions.
- `CHANGELOG.md` — **modify**: new `[Unreleased]` section describing the change.

---

## Task 1: Export keys in the report viewer

**Files:**
- Modify: `src/cli/dashboard.rs` (the `DashboardScreen::ReportView` key arm, ~line 1202)
- Modify: viewer footer hint text (wherever the report view footer renders — `src/tui.rs` `run_report_view` or the dashboard's draw path for `ReportView`; find and update the actual location)
- Test: inline in `src/cli/dashboard.rs`

**Interfaces:**
- Consumes: `view.date_params() -> (Option<i32>, Option<String>)`, `dashboard.current_report_idx: Option<usize>`, `do_export(idx, year, month)`, `do_text_export(idx, year, month)`.
- Produces: a pure `viewer_export_action(code: KeyCode) -> Option<ExportAction>` helper with `enum ExportAction { Pdf, Text }` (`Pdf` variant cfg-gated on `feature = "pdf"`), consumed by the key arm; Task 2 relies on the same enum for the picker's Export All entry.

- [ ] **Step 1: Write the failing test**

Add to `src/cli/dashboard.rs` tests (create the module if the file has none):

```rust
#[cfg(test)]
mod viewer_export_tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[test]
    fn export_keys_map_to_actions() {
        #[cfg(feature = "pdf")]
        assert!(matches!(
            viewer_export_action(KeyCode::Char('e')),
            Some(ExportAction::Pdf)
        ));
        #[cfg(not(feature = "pdf"))]
        assert!(viewer_export_action(KeyCode::Char('e')).is_none());
        assert!(matches!(
            viewer_export_action(KeyCode::Char('t')),
            Some(ExportAction::Text)
        ));
        assert!(viewer_export_action(KeyCode::Char('m')).is_none());
        assert!(viewer_export_action(KeyCode::Esc).is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test viewer_export_tests 2>&1 | head`
Expected: FAIL to compile (`viewer_export_action` undefined).

- [ ] **Step 3: Implement**

Add near the dashboard's other helpers:

```rust
#[derive(Debug, Clone, Copy)]
enum ExportAction {
    #[cfg(feature = "pdf")]
    Pdf,
    Text,
}

fn viewer_export_action(code: KeyCode) -> Option<ExportAction> {
    match code {
        #[cfg(feature = "pdf")]
        KeyCode::Char('e') => Some(ExportAction::Pdf),
        KeyCode::Char('t') => Some(ExportAction::Text),
        _ => None,
    }
}
```

In the `DashboardScreen::ReportView` key arm (~line 1202), before `view.handle_key(key.code)`: if `viewer_export_action(key.code)` is `Some(action)` and `current_report_idx` is `Some(idx)`, read `let (year, month) = view.date_params();`, run the matching helper (`do_export(idx, year, month)` / `do_text_export(idx, year, month)`), and surface the returned "Exported …" string through the dashboard's existing status-message mechanism (find how `pending_export` results are displayed today and reuse it exactly — same field, same styling). An `Err` surfaces through the same mechanism as existing export errors. Do not forward the key to `view.handle_key` when it was consumed as an export.

Update the report-view footer hint wherever it renders to include `e export pdf · t export text` (text-only variant when the `pdf` feature is off).

- [ ] **Step 4: Run tests + both feature configs**

Run: `cargo test --features pdf && cargo test --no-default-features --features gusto && cargo clippy -- -D warnings && cargo fmt --check`
Expected: PASS/clean.

- [ ] **Step 5: Manual verification (document output in your report)**

`cargo run --features pdf` → dashboard → View a Report → K-1 → page back to a previous year with Left arrow → `e` → status shows `Exported …/exports/…pdf`; `t` → text file. Confirm the exported file's period matches the viewed period (open the text file).

- [ ] **Step 6: Commit**

```bash
git add src/cli/dashboard.rs src/tui.rs
git commit -m "feat(reports): export the viewed period directly from the report viewer"
```

---

## Task 2: Remove export screens; single "View Reports" picker with Export All

**Files:**
- Modify: `src/cli/dashboard.rs`
- Modify: `CLAUDE.md`, `README.md`, `CHANGELOG.md`
- Test: inline (picker-entry constants) + existing suite

**Interfaces:**
- Consumes: `ExportAction`, `do_export`/`do_text_export` with `idx == 8` meaning "All Reports" (existing behavior of the helpers — keep their index contract intact).
- Produces: single `REPORT_TYPES` picker with a final `"Export All Reports"` entry; no `ReportPickerMode`, no `EXPORT_TYPES`, no `EXPORT_FORMATS`, no `DashboardScreen::ExportFormatPicker`.

- [ ] **Step 1: Delete the export flow**

In `src/cli/dashboard.rs`: remove `ReportPickerMode` (the picker keeps only its `selection`), `EXPORT_TYPES`, `EXPORT_FORMATS`, the `ExportFormatPicker` screen variant, its draw arm (~line 335) and key arm (~line 1245), and the home-menu "Export report" entry. The home `e` shortcut either opens the (single) report picker like `v`, or is removed — pick whichever keeps the menu help accurate, state the choice in your report.

- [ ] **Step 2: Rename and extend the picker**

Picker title becomes `"View Reports"`; menu entry text becomes `"View Reports"` (was "View report"/"View a Report" — match the actual current string). Append `"Export All Reports"` to `REPORT_TYPES`. In the picker's Enter handling: the last index triggers export-all — `Enter` runs the PDF variant (`do_export(8, year, month)`) when the `pdf` feature is on, and the text variant otherwise; `t` on that entry always runs `do_text_export(8, ..)`. Footer/help line on the picker states the keys. Non-last entries behave as today (open the viewer).

Keep `do_export`/`do_text_export` signatures and their `idx == 8` "all reports" branch unchanged so the helpers stay shared.

- [ ] **Step 3: Adjust tests + picker constants sanity test**

Add to the dashboard test module:

```rust
    #[test]
    fn picker_last_entry_is_export_all() {
        assert_eq!(REPORT_TYPES.last(), Some(&"Export All Reports"));
        // The export helpers' index contract: idx 8 == all reports.
        assert_eq!(REPORT_TYPES.len() - 1, 8);
    }
```

Fix any existing test/code referencing the removed items.

- [ ] **Step 4: Docs + changelog**

- `CLAUDE.md`: Dashboard bullet's command list (`e=Export report` entry) and the Reports bullet — describe the current flow: viewer exports the displayed period via `e`/`t`; picker is "View Reports" with an Export All entry. Current behavior only.
- `README.md`: update any mention of the dashboard export flow.
- `CHANGELOG.md`: add an `[Unreleased]` section at the top: exports moved into the report viewer (export exactly the period on screen, any year); standalone export screens removed; Export All available from the View Reports picker.

- [ ] **Step 5: Run everything**

Run: `cargo test --features pdf && cargo test --no-default-features --features gusto && cargo clippy -- -D warnings && cargo fmt --check`
Expected: PASS/clean.

- [ ] **Step 6: Manual verification (document in report)**

Dashboard: menu shows "View Reports" and no export entry; picker lists 9 entries ending in "Export All Reports"; Enter on it exports all (status message), `t` on it exports all as text; a normal entry still opens the viewer; `e`/`t` inside the viewer still export the viewed period.

- [ ] **Step 7: Commit**

```bash
git add src/cli/dashboard.rs CLAUDE.md README.md CHANGELOG.md
git commit -m "feat(reports): single View Reports picker with Export All; remove export screens"
```

---

## Self-Review

**Spec coverage (task-26 ACs):** AC1 (export any period from viewer) → Task 1. AC2 (screens removed, menu renamed) → Task 2. AC3 (Export All secondary) → Task 2. AC4 (CLI unchanged) → neither task touches `cli/report/mod.rs` flags. ✓

**Placeholder scan:** the two "find the actual location/string" directives (footer render site, exact menu label) are deliberate — the implementer verifies against the file rather than trusting stale line numbers; the required end state is fully specified. ✓

**Type consistency:** `ExportAction` defined in Task 1, consumed in Task 2; `do_export`/`do_text_export(idx, year, month)` signatures unchanged throughout; index-8 contract pinned by test. ✓
