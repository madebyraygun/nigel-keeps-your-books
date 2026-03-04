# TOTP Two-Factor Authentication Design

**Issue**: [#166](https://github.com/daltonrooney/nigel/issues/166)
**Date**: 2026-03-03

## Summary

Add optional TOTP-based two-factor authentication as an app-level convenience lock for encrypted databases. The TOTP code is required in addition to the password when unlocking through Nigel's UI.

**Security note**: This is an application-level check, not a cryptographic second factor. The TOTP secret is stored inside the SQLCipher-encrypted database, so anyone who knows the password can bypass TOTP by accessing the database directly. This is accepted as a UX-level guard against casual access.

## Decisions

- **Feature flag**: `totp` (cargo feature)
- **Library**: `totp-rs` with `gen_secret` and `otpauth` features
- **Storage**: `metadata` table with `totp_secret` key (no migration needed)
- **CLI structure**: `nigel password totp-enable` / `nigel password totp-disable`
- **Recovery**: None — users must restore from an unencrypted backup if they lose their authenticator
- **Display**: Copyable base32 string in TUI (QR code is out of scope)

## Architecture

### New Module: `src/totp.rs`

All TOTP logic encapsulated in one module, gated with `#[cfg(feature = "totp")]`.

```rust
pub fn generate_secret(issuer: &str, account: &str) -> (String, String)
    // Returns (base32_secret, otpauth_uri)

pub fn verify_code(secret: &str, code: &str) -> bool
    // 6-digit TOTP, 30s window, 1 step tolerance

pub fn is_enabled(conn: &Connection) -> bool
    // Checks metadata for totp_secret key

pub fn enable(conn: &Connection, secret: &str)
    // Stores secret in metadata

pub fn disable(conn: &Connection)
    // Removes totp_secret from metadata

pub fn prompt_totp_if_needed(db_path: &Path) -> Result<()>
    // CLI helper: checks if TOTP enabled, prompts via stdin
```

### Data Flow

```
Password Entry → Validate Password → Open DB → Check totp_secret in metadata
  → (if present) → Prompt TOTP Code → Verify → Unlock
  → (if absent) → Unlock immediately
```

The TOTP check must happen after password validation because the secret is inside the encrypted database.

## Screen Changes

### Splash (`cli/splash.rs`)

After `try_password()` succeeds:
1. Open a temporary connection to check `totp::is_enabled()`
2. If enabled, switch to TOTP input phase (new `totp_input: String` field)
3. Display "Enter authenticator code:" below password area
4. On Enter, verify code against stored secret
5. 3 attempts max, same as password

### Onboarding (`cli/onboarding.rs`)

New `Screen::TotpSetup` between `ConfirmPassword` and `ActionPicker`:
1. Only shown when a password is set
2. Displays TOTP secret as copyable base32 string
3. Prompts for verification code to confirm setup
4. "Skip" option to proceed without 2FA
5. Adds `totp_secret: Option<String>` to `OnboardingResult`

### Password Manager (`cli/password_manager.rs`)

New menu items (context-dependent):
- When encrypted + no TOTP: "Enable 2FA"
- When encrypted + TOTP active: "Disable 2FA"
- When TOTP active: password change/remove requires TOTP verification first

Enable flow: generate secret → display → verify code → store
Disable flow: verify current code → remove from metadata

### Settings Manager (`cli/settings_manager.rs`)

New `MENU_TOTP = 2` menu entry: "Two-Factor Auth"
- Delegates to password manager's TOTP flow
- Only visible when database is encrypted

### CLI (`cli/password.rs`, `cli/mod.rs`)

New `PasswordCommand` variants:
- `TotpEnable` — generates secret, displays, prompts verification
- `TotpDisable` — prompts for current code, removes secret

Both require the database to be encrypted first.

### DB (`db.rs`)

Extend `prompt_password_if_needed()`:
- After successful password prompt, call `totp::prompt_totp_if_needed()`
- Used by all CLI subcommands automatically

## Files Changed

| File | Type | Description |
|------|------|-------------|
| `src/totp.rs` | New | TOTP module (all logic) |
| `Cargo.toml` | Modify | Add `totp-rs` dep, `totp` feature |
| `src/main.rs` | Modify | Add `mod totp` |
| `src/cli/mod.rs` | Modify | Add `TotpEnable`/`TotpDisable` commands |
| `src/cli/password.rs` | Modify | Add `run_totp_enable()`, `run_totp_disable()` |
| `src/cli/splash.rs` | Modify | Add TOTP input phase |
| `src/cli/onboarding.rs` | Modify | Add `TotpSetup` screen |
| `src/cli/password_manager.rs` | Modify | Add 2FA menu items + flows |
| `src/cli/settings_manager.rs` | Modify | Add 2FA menu entry |
| `src/db.rs` | Modify | Extend `prompt_password_if_needed()` |
| `CLAUDE.md` | Modify | Document feature |
| `README.md` | Modify | Document feature |
