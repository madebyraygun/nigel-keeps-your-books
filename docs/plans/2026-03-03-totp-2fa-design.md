# TOTP Two-Factor Authentication Design

**Issue**: [#166](https://github.com/daltonrooney/nigel/issues/166)
**Date**: 2026-03-03

## Summary

Add optional TOTP-based two-factor authentication as a true second factor for encrypted databases. The TOTP secret is stored in the OS keychain (separate from the database), so knowing the database password alone is insufficient to bypass 2FA.

## Security Model

The TOTP secret is stored in the OS keychain via the `keyring` crate — **not** inside the encrypted database. This makes it a genuine second factor:

- **Factor 1 (something you know)**: Database password (unlocks SQLCipher encryption)
- **Factor 2 (something you have)**: TOTP code from authenticator app (secret stored in OS keychain)

An attacker with only the database password cannot generate TOTP codes because the secret lives in the OS keychain (macOS Keychain, Windows Credential Store, or Linux Secret Service/keyutils). The database stores only a `totp_enabled` flag in the `metadata` table to signal that TOTP verification is required.

**Device-bound**: TOTP enrollment is tied to the machine's keychain. Backing up/restoring the database to another machine will not carry over the TOTP secret — the user must re-enable 2FA on each machine.

## Decisions

- **Feature flag**: `totp` (cargo feature)
- **TOTP library**: `totp-rs` with `rand` feature (for secret generation)
- **Keychain library**: `keyring` (cross-platform: macOS Keychain, Windows Credential Store, Linux keyutils/Secret Service)
- **Secret storage**: OS keychain via `keyring` crate; `metadata` table stores only a `totp_enabled` flag
- **Keychain entry key**: service=`"nigel"`, user=`"totp:<canonical_db_path>"`
- **CLI structure**: `nigel password totp-enable` / `nigel password totp-disable`
- **Recovery**: None — if authenticator is lost, user must have access to the OS keychain to disable 2FA, or restore from a backup (which won't have TOTP enabled since it's device-bound)
- **Display**: Copyable base32 string in TUI (QR code rendering is out of scope)

## Architecture

### New Module: `src/totp.rs`

All TOTP logic encapsulated in one module, gated with `#[cfg(feature = "totp")]`.

```rust
/// Generate a new TOTP secret. Returns (base32_secret, otpauth_uri).
pub fn generate_secret(issuer: &str, account: &str) -> Result<(String, String)>

/// Verify a 6-digit TOTP code against a base32-encoded secret.
/// Uses SHA1, 30s step, 1 step skew tolerance.
pub fn verify_code(secret: &str, code: &str) -> bool

/// Check if TOTP is enabled for this database (checks metadata flag).
pub fn is_enabled(conn: &Connection) -> bool

/// Check if TOTP is enabled by checking the keychain directly (no DB needed).
pub fn is_enabled_in_keychain(db_path: &Path) -> bool

/// Enable TOTP: store secret in OS keychain, set metadata flag.
pub fn enable(conn: &Connection, db_path: &Path, secret: &str) -> Result<()>

/// Disable TOTP: remove secret from OS keychain, clear metadata flag.
pub fn disable(conn: &Connection, db_path: &Path) -> Result<()>

/// Get the TOTP secret from the OS keychain.
pub fn get_secret(db_path: &Path) -> Result<String>

/// CLI helper: if TOTP is enabled, prompt for code via stdin (up to 3 attempts).
pub fn prompt_totp_if_needed(db_path: &Path) -> Result<()>

/// Build the keyring entry key for a given database path.
fn keychain_entry(db_path: &Path) -> keyring::Entry
```

### Data Flow

```
Password Entry → Validate Password → Open DB → Check metadata for totp_enabled
  → (if enabled) → Prompt TOTP Code → Fetch secret from OS keychain → Verify → Proceed
  → (if not enabled) → Proceed immediately
```

The TOTP check happens after password validation because the `totp_enabled` flag is inside the encrypted database. The actual secret is fetched from the OS keychain for verification.

### Enable Flow

```
User chooses "Enable 2FA" → Generate random secret → Display base32 string
  → User scans into authenticator app → User enters verification code
  → Verify code against generated secret → If valid:
    → Store secret in OS keychain (keyring)
    → Set metadata totp_enabled = "true" in database
```

### Disable Flow

```
User chooses "Disable 2FA" → Prompt for current TOTP code
  → Fetch secret from OS keychain → Verify code → If valid:
    → Delete secret from OS keychain
    → Remove totp_enabled from metadata
```

## Screen Changes

### Splash (`cli/splash.rs`)

After `try_password()` succeeds:
1. Open a temporary connection to check `totp::is_enabled()`
2. If enabled, switch to TOTP input phase (new `totp_input: String` field)
3. Display "Enter authenticator code:" below password area
4. On Enter, fetch secret from keychain, verify code
5. 3 attempts max, same as password

### Onboarding (`cli/onboarding.rs`)

New `Screen::TotpSetup` between `ConfirmPassword` and `ActionPicker`:
1. Only shown when a password is set
2. Generates a TOTP secret and displays it as a copyable base32 string
3. Prompts for verification code to confirm setup
4. "Skip" option (Esc) to proceed without 2FA
5. Adds `totp_secret: Option<String>` to `OnboardingResult`
6. The caller (dashboard.rs) stores the secret in the keychain after DB creation

### Password Manager (`cli/password_manager.rs`)

New menu items (context-dependent):
- When encrypted + no TOTP: "Enable 2FA"
- When encrypted + TOTP active: "Disable 2FA"
- When TOTP active: password change/remove requires TOTP verification first

Enable flow: generate secret → display → verify code → store in keychain + set flag
Disable flow: verify current code → remove from keychain + clear flag

### Settings Manager (`cli/settings_manager.rs`)

New `MENU_TOTP = 2` menu entry: "Two-Factor Auth"
- Only visible when database is encrypted
- Delegates to password manager's TOTP enable/disable flow

### CLI (`cli/password.rs`, `cli/mod.rs`)

New `PasswordCommand` variants:
- `TotpEnable` — generates secret, displays, prompts verification, stores in keychain
- `TotpDisable` — prompts for current code, removes from keychain

Both require the database to be encrypted first.

### DB (`db.rs`)

Extend `prompt_password_if_needed()`:
- After successful password prompt, call `totp::prompt_totp_if_needed()`
- Used by all CLI subcommands automatically

## Files Changed

| File | Type | Description |
|------|------|-------------|
| `src/totp.rs` | New | TOTP module (secret gen, verification, keychain storage) |
| `Cargo.toml` | Modify | Add `totp-rs`, `keyring` deps; `totp` feature flag |
| `src/main.rs` | Modify | Add `mod totp` |
| `src/cli/mod.rs` | Modify | Add `TotpEnable`/`TotpDisable` commands |
| `src/cli/password.rs` | Modify | Add `run_totp_enable()`, `run_totp_disable()` |
| `src/cli/splash.rs` | Modify | Add TOTP input phase after password success |
| `src/cli/onboarding.rs` | Modify | Add `TotpSetup` screen, `totp_secret` in result |
| `src/cli/password_manager.rs` | Modify | Add 2FA enable/disable menu items + flows |
| `src/cli/settings_manager.rs` | Modify | Add 2FA menu entry |
| `src/db.rs` | Modify | Extend `prompt_password_if_needed()` with TOTP |
| `CLAUDE.md` | Modify | Document feature |
| `README.md` | Modify | Document feature |
