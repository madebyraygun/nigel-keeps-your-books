---
id: TASK-20
title: Add optional TOTP two-factor authentication for database password protection
status: To Do
assignee: []
created_date: '2026-04-25 18:06'
labels: []
dependencies: []
references:
  - 'https://github.com/madebyraygun/nigel-keeps-your-books/issues/166'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Add optional TOTP-based two-factor authentication (2FA) as a second layer of protection when setting up or unlocking an encrypted database. Ref #161 (inline password on splash screen), #162 (failed password attempt logging).

## Detail

Currently Nigel supports password-protecting the SQLite database via SQLCipher encryption. This feature would add an optional TOTP (Time-based One-Time Password) step, requiring a code from an authenticator app (Authy, Google Authenticator, 1Password, etc.) in addition to the password.

* **Onboarding screen** (`cli/onboarding.rs`): Add an optional checkbox/toggle on the password setup step (currently `FIELD_PASSWORD` at index 2) to enable 2FA. When enabled, display a QR code or secret key for the user to scan into their authenticator app, then prompt for a verification code to confirm setup.
* **Confirm password screen** (`cli/onboarding.rs`, `ConfirmPassword` variant): Add a new TOTP code input section that appears when 2FA is enabled, requiring the user to enter their current authenticator code before proceeding.
* **Settings screen** (`cli/settings_manager.rs`): Add a new menu entry (alongside Business Name and Password) for managing 2FA — enable, disable, or re-generate the TOTP secret. The `PasswordManager` (`cli/password_manager.rs`) should also require a valid TOTP code when changing or removing the password if 2FA is active.
* **Splash screen** (`cli/splash.rs`): When unlocking an encrypted database that has 2FA enabled, prompt for the TOTP code after the password is accepted.
* **TOTP secret storage**: Store the TOTP secret in the `metadata` table (encrypted at rest inside the SQLCipher database). It should never be persisted to disk outside the encrypted database.
* A TOTP library (e.g., `totp-rs`) will be needed for secret generation and code verification.

## Proposed solution

Add a `totp_secret` key to the `metadata` table inside the encrypted database. Introduce a shared TOTP verification helper used by onboarding, splash, password manager, and settings manager screens. Gate the feature behind a cargo feature flag (e.g., `2fa`) to keep it optional. Display the TOTP secret as a copyable string in the TUI (QR code rendering in terminal is a stretch goal).

## Acceptance Criteria

- [ ] Onboarding password step has an optional toggle to enable 2FA
- [ ] When enabled, onboarding displays the TOTP secret and verifies a code before proceeding
- [ ] Confirm password screen requires TOTP code when 2FA is active
- [ ] Splash screen prompts for TOTP code after successful password entry when 2FA is active
- [ ] Settings screen has a new 2FA management entry (enable/disable/regenerate)
- [ ] Password changes and removal require a valid TOTP code when 2FA is active
- [ ] TOTP secret is stored only inside the encrypted database (`metadata` table), never on disk in plaintext
- [ ] CLI subcommands that prompt for password also prompt for TOTP when applicable
- [ ] All linting checks pass
- [ ] Update test coverage
- [ ] Create or update documentation, making sure to remove any out of date information
- [ ] **IMPORTANT**: Any PRs created from this issue must be created as DRAFTS until manually reviewed by the user

---
*Migrated from [GitHub issue #166](https://github.com/madebyraygun/nigel-keeps-your-books/issues/166)*
<!-- SECTION:DESCRIPTION:END -->
