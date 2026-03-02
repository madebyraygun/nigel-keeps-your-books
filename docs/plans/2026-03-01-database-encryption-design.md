# Database Encryption via SQLCipher — Design

**Issue:** #66
**Date:** 2026-03-01

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Encryption detection | Try-then-prompt | No persisted flag needed; purely runtime detection via SQLITE_NOTADB error |
| Password threading | Global `OnceLock<Option<String>>` in db.rs | Zero call-site changes across 33 production callers |
| Password management | CLI subcommand + dashboard menu | CLI for scripting, dashboard for interactive use |
| Backup encryption | Preserve source state | Encrypted source → encrypted backup; plain → plain |

## Architecture

### Dependency Change

Replace `bundled` with `bundled-sqlcipher` in Cargo.toml. Keep `backup` feature.

```toml
rusqlite = { version = "0.31", features = ["bundled-sqlcipher", "backup"] }
```

### Password Storage (Runtime Only)

```rust
// db.rs
static DB_PASSWORD: OnceLock<Option<String>> = OnceLock::new();

pub fn set_db_password(password: Option<String>) { ... }
pub fn get_db_password() -> Option<&'static str> { ... }
```

Password is set once at startup and never persisted to disk.

### Connection Opening

`get_connection()` signature unchanged. After `Connection::open()`:
- If global password is `Some(pw)`: issue `PRAGMA key = '...'` (properly quoted)
- Then WAL + foreign_keys as today
- If no password: behave exactly as today

### Encryption Detection (Launch)

1. Try `get_connection()` with no password
2. Test query: `SELECT count(*) FROM sqlite_master`
3. If SQLITE_NOTADB → prompt for password via `rpassword` (masked stdin)
4. Retry with provided password
5. Up to 3 attempts, then exit with error suggesting possible corruption

### Onboarding Flow

When `OnboardingResult.password` is `Some(pw)`:
1. Set global password via `set_db_password()` before first `get_connection()`
2. First connection opens new DB with `PRAGMA key` → DB encrypted from creation

### `nigel password` Subcommand

- `nigel password set` — prompt for new password, encrypt via `PRAGMA rekey`
- `nigel password change` — prompt for current + new, re-key via `PRAGMA rekey`
- `nigel password remove` — prompt for current, decrypt via `PRAGMA rekey = ''`

### Dashboard Menu

Add `p=Password` to command chooser. Inline TUI screen for set/change/remove (same operations as CLI subcommand).

### Backups & Snapshots

`snapshot()` applies the same global password to destination connection via `PRAGMA key`. Encrypted source → encrypted backup. Plain source → plain backup.

### Demo Mode

`nigel demo` never sets a password. Demo DBs always unencrypted.

### Error Handling

- Wrong password → clear error message, re-prompt
- After 3 failed attempts → suggest DB may be corrupted
- Corrupted DB and encrypted DB both return NOTADB; user prompt disambiguates

## Files Affected

| File | Change |
|------|--------|
| `Cargo.toml` | `bundled` → `bundled-sqlcipher`, add `rpassword` dep |
| `src/db.rs` | Add `OnceLock`, `set_db_password()`, `PRAGMA key` in `get_connection()` |
| `src/cli/dashboard.rs` | Wire onboarding password, add encryption detection, add `p=Password` menu |
| `src/cli/backup.rs` | Apply password to destination connection in `snapshot()` |
| `src/cli/mod.rs` | Add `Password` subcommand with set/change/remove |
| `src/cli/password.rs` | New — CLI password management (set/change/remove via PRAGMA rekey) |
| `src/cli/password_manager.rs` | New — TUI password management screen for dashboard |
| `src/main.rs` | Add encryption detection + prompt for all subcommands, dispatch password cmd |
| `src/cli/demo.rs` | Ensure demo never sets password |
