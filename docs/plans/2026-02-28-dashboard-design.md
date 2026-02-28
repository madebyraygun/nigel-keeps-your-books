# Dashboard Design (#16)

## Overview

Add an interactive ratatui dashboard as the default view when running `nigel` with no arguments. The dashboard shows a financial overview and a command chooser menu that transitions inline to other screens.

## Architecture: Single-struct state machine

One `Dashboard` struct with a `DashboardScreen` enum. The draw method switches on the current screen. Transitions swap the enum variant and load screen-specific state. One event loop, one shared DB connection.

```rust
enum DashboardScreen {
    Home,
    Browse(RegisterBrowserState),
    Review(TransactionReviewerState),
    Rules,       // stub
    Report,      // stub
    Import,      // stub
    Reconcile,   // stub
    Export,      // stub
}
```

## Entry point

Make `Cli.command` an `Option<Commands>`. In `main.rs`, `None` dispatches to `cli::dashboard::run()`. All existing `nigel <subcommand>` usage unchanged.

## Home screen layout

```
┌──────────────────────────────────────────────────────┐
│ Nigel: Kettle's on.                                  │  Length(1)
├─────────────────────────────┬────────────────────────┤
│ YTD Income    $XX,XXX       │  Account Balances      │  Length(5)
│ YTD Expenses -$XX,XXX       │  BofA Checking  $X,XXX │
│ Net Profit    $XX,XXX       │  BofA Credit   -$X,XXX │
│ Transactions  1,224         │                        │
│ Flagged       5             │                        │
├─────────────────────────────┴────────────────────────┤
│ ██ ██ ██ ██ ██ ██ ██ ██ ██ ██ ██ ██                  │  Length(8)
│ Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec      │
├──────────────────────────────────────────────────────┤
│ What would you like to do?                           │  Fill(1)
│ > Browse the register                                │
│   Import a statement                                 │
│   Review flagged transactions (5)                    │
│   Reconcile an account                               │
│   View or edit categorization rules                  │
│   View a report                                      │
│   Export a report                                    │
├──────────────────────────────────────────────────────┤
│ Up/Down=navigate, Enter=select, r=refresh, q=quit    │  Length(1)
└──────────────────────────────────────────────────────┘
```

## Data sources

All data loaded from existing report functions — no new queries needed:
- `get_pnl(conn, year, None, None, None)` — income, expenses, net
- `get_balance(conn)` — account balances
- `get_cashflow(conn, year, None)` — monthly bar chart data
- `get_flagged(conn)` — flagged count
- Transaction count: `SELECT COUNT(*) FROM transactions`

Data loaded once at startup, refreshed with `r` key.

## Branding header

Top row: `Nigel: {greeting}` — random greeting selected once at startup via `rand::seq::SliceRandom`.

15 greetings:
- "Kettle's on."
- "Right then, let's have a look at the numbers."
- "Lovely day to reconcile, innit?"
- "Books won't balance themselves."
- "Back again? Brilliant."
- "Another day, another CSV."
- "Shall we see where the money's gone?"
- "Pull up a chair."
- "Everything's in order. Well, mostly."
- "No surprises today. Well... let's not get ahead of ourselves."
- "Fancy a quick look at the numbers?"
- "You're looking well. The books, less so."
- "Ah, there you are."
- "The spreadsheets send their regards."
- "Right then, where were we?"

## Command menu

7 items with `>` marker on selected. Up/Down navigate, Enter transitions.

| Menu item | Screen transition |
|---|---|
| Browse the register | `DashboardScreen::Browse(...)` — inline ratatui browser |
| Import a statement | `DashboardScreen::Import` — stub, prompt for file path |
| Review flagged transactions (N) | `DashboardScreen::Review(...)` — inline ratatui review |
| Reconcile an account | `DashboardScreen::Reconcile` — stub |
| View or edit categorization rules | `DashboardScreen::Rules` — stub |
| View a report | `DashboardScreen::Report` — stub |
| Export a report | `DashboardScreen::Export` — stub |

## State transitions

- **Home + Enter** → load screen state, swap `DashboardScreen` variant
- **Any screen + Esc/q** → `DashboardScreen::Home`, refresh data
- **Ctrl+C** → exit from any screen

## Monthly bar chart

Side-by-side bars per month using ratatui `BarChart` widget. Income (green) and expenses (red) grouped per month. Current fiscal year by default.

## Dependencies

Add `rand = "0.8"` to Cargo.toml.

## Scope for first PR

Implement:
- Home screen with all widgets and command menu
- Inline transition to Browse and Review (refactor existing logic into state machine)
- Stub transitions for remaining 5 commands (show "Coming soon", Esc to return)

## Files to create/modify

- **New:** `src/cli/dashboard.rs` — Dashboard struct, draw, event loop
- **Modify:** `src/cli/mod.rs` — `Option<Commands>`, add dashboard dispatch
- **Modify:** `src/main.rs` — handle `None` command case
- **Modify:** `src/browser.rs` — extract state into reusable struct
- **Modify:** `src/cli/review.rs` — extract state into reusable struct
- **Modify:** `Cargo.toml` — add `rand`
- **Modify:** `CLAUDE.md` — update architecture
