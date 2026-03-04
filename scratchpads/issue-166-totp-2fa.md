# Issue #166: TOTP Two-Factor Authentication

**Link**: https://github.com/daltonrooney/nigel/issues/166
**Branch**: `feature/166-totp-2fa`
**Design**: `docs/plans/2026-03-03-totp-2fa-design.md`

## Tasks

- [ ] 1. Add `totp-rs` dependency and `totp` feature flag to Cargo.toml
- [ ] 2. Create `src/totp.rs` module with core TOTP logic
- [ ] 3. Wire up `mod totp` in `main.rs`
- [ ] 4. Add `TotpEnable`/`TotpDisable` CLI subcommands (`cli/mod.rs`, `cli/password.rs`)
- [ ] 5. Extend `prompt_password_if_needed()` in `db.rs` to prompt for TOTP
- [ ] 6. Add TOTP input phase to splash screen (`cli/splash.rs`)
- [ ] 7. Add TOTP setup screen to onboarding (`cli/onboarding.rs`)
- [ ] 8. Add 2FA enable/disable to password manager (`cli/password_manager.rs`)
- [ ] 9. Add 2FA menu entry to settings manager (`cli/settings_manager.rs`)
- [ ] 10. Update `CLAUDE.md` and `README.md` documentation
- [ ] 11. Write unit tests for TOTP module
- [ ] 12. Integration test: full enable/disable/verify flow

## Notes

- TOTP secret stored in `metadata` table (no migration needed)
- Feature gated behind `totp` cargo feature flag
- No recovery mechanism — restore from backup if authenticator lost
- App-level convenience lock, not cryptographic second factor
