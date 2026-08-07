# Contributing

## Setup

```bash
git clone https://github.com/madebyraygun/nigel-keeps-your-books.git
cd nigel-keeps-your-books
cargo build
```

## Running Tests

```bash
cargo test -- --test-threads=1                       # All tests
cargo test --no-default-features -- --test-threads=1 # Without gusto feature
cargo test --no-default-features --features serve -- --test-threads=1
```

`--test-threads=1` is not optional. The database password is a process-global
(`db::DB_PASSWORD`), and the tests that encrypt, decrypt, unlock and switch data
directories all move it — run in parallel they read each other's keys and fail
in a different combination each time. CI runs them serially for the same reason.

Tests use temporary and in-memory SQLite databases with synthetic data — no
external files or services needed.

The web suite has no such constraint:

```bash
cd web && npm ci && npm test
```

## Project Layout

```
src/
  lib.rs                # Library root — exposes all modules as the `nigel` crate
  main.rs               # Binary entry point: clap parse, panic hook, command dispatch
  cli/                  # CLI subcommands
    mod.rs              # Clap structs (Cli, Commands, subcommands)
    init.rs             # nigel init
    demo.rs             # nigel demo (sample data)
    accounts.rs         # nigel accounts add/list
    import.rs           # nigel import
    categorize.rs       # nigel categorize
    rules.rs            # nigel rules add/list
    review.rs           # nigel review
    report.rs           # nigel report pnl/expenses/tax/cashflow/balance/flagged
    reconcile.rs        # nigel reconcile
  db.rs                 # SQLite schema, connection, category seeding
  models.rs             # Structs (Account, Transaction, Rule, ParsedRow, etc.)
  importer.rs           # ImporterKind enum, format detection, CSV/XLSX parsing
  categorizer.rs        # Rules engine (categorize_transactions)
  reviewer.rs           # Interactive review flow
  reports.rs            # Report data functions (pnl, expenses, tax, etc.)
  reconciler.rs         # Monthly reconciliation
  settings.rs           # Settings management (~/.config/nigel/)
  fmt.rs                # Number formatting helpers
  error.rs              # Error types
```

## Code Style

- Rust stable toolchain
- `cargo clippy` — no warnings
- `cargo fmt` — standard formatting
- Cash amounts are plain `f64` — negative = expense, positive = income
- Prefer simple, flat code over abstractions

## Documentation

Every feature change should update:

- **CLAUDE.md** — architecture, commands, structure, and constraints sections
- **README.md** — features, quick start, and configuration if applicable

## Commits

- Keep commits focused — one logical change per commit
- Use imperative mood in commit messages ("Add feature" not "Added feature")
