//! A string that will not print itself.
//!
//! Passwords arrive over HTTP as ordinary JSON strings, and everything they
//! land in — request structs, error paths, `#[derive(Debug)]` output — is one
//! stray format call away from a log line. [`Secret`] makes that impossible by
//! construction: its `Debug` is redacted and it zeroizes on drop, matching what
//! `splash.rs` and `password_manager.rs` already do for TUI input buffers.

use std::fmt;

use serde::Deserialize;
use zeroize::Zeroize;

#[derive(Deserialize, Default, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    /// Read the wrapped value. Every call site is a place a password can escape,
    /// so the name is deliberately loud.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Holder {
        password: Secret,
    }

    #[test]
    fn debug_never_shows_the_value() {
        let secret = Secret::from("hunter2".to_string());
        let rendered = format!("{secret:?}");
        assert_eq!(rendered, "Secret(<redacted>)");
        assert!(!rendered.contains("hunter2"));
    }

    #[test]
    fn redaction_survives_a_derived_debug() {
        let holder: Holder = serde_json::from_str(r#"{"password": "hunter2"}"#).expect("json");
        let rendered = format!("{holder:?}");
        assert!(!rendered.contains("hunter2"), "leaked in {rendered}");
        assert!(rendered.contains("<redacted>"), "got {rendered}");
    }

    #[test]
    fn deserializes_transparently() {
        let holder: Holder = serde_json::from_str(r#"{"password": "hunter2"}"#).expect("json");
        assert_eq!(holder.password.expose(), "hunter2");
    }
}
