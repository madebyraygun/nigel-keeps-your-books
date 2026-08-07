//! Shared server state.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use tokio::sync::RwLock as TokioRwLock;

use super::error::ApiError;

/// The failure count at which answers start being held back. The first two
/// cost nothing but a message; the third and every one after it is delayed.
const MAX_FREE_ATTEMPTS: u32 = 3;
const BACKOFF_BASE: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);

/// How long the server holds back the answer to the `failures`-th failed
/// unlock: nothing for the first two, then 1s, 2s, 4s… capped at 30s.
pub fn backoff_delay(failures: u32) -> Duration {
    let Some(step) = failures.checked_sub(MAX_FREE_ATTEMPTS) else {
        return Duration::ZERO;
    };
    let Some(factor) = 1u32.checked_shl(step) else {
        return BACKOFF_CAP;
    };
    (BACKOFF_BASE * factor).min(BACKOFF_CAP)
}

/// Failed-unlock bookkeeping: in memory, process-wide, never persisted. There
/// is no hard lockout — whoever is at the keyboard can always restart the
/// process — so repeated failures only get slower.
#[derive(Debug, Default)]
pub struct UnlockGate {
    failures: Mutex<u32>,
}

impl UnlockGate {
    /// Count a failed attempt and report what to tell the caller: how many free
    /// attempts are left, and how long this answer should be held back.
    pub fn record_failure(&self) -> (u32, Duration) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        let mut failures = self.failures.lock().unwrap();
        *failures = failures.saturating_add(1);
        let count = *failures;
        drop(failures);

        (
            MAX_FREE_ATTEMPTS.saturating_sub(count),
            backoff_delay(count),
        )
    }

    pub fn reset(&self) {
        // unwrap: poisoned mutex means a thread panicked — unrecoverable
        *self.failures.lock().unwrap() = 0;
    }
}

/// Build-time capabilities the API advertises to the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Features {
    pub pdf: bool,
    pub gusto: bool,
}

impl Features {
    pub fn detect() -> Self {
        Self {
            pdf: cfg!(feature = "pdf"),
            gusto: cfg!(feature = "gusto"),
        }
    }
}

/// State cloned into every request. Handlers open their own `rusqlite`
/// connection from `db_path()` inside `spawn_blocking`; there is no pool.
///
/// The database password is deliberately absent: it lives in the process-global
/// `db::set_db_password` mutex that every CLI subcommand already uses, so
/// `nigel serve` inherits the CLI's one-database-per-process assumption.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Which database this server is serving. Behind a lock because the web
    /// settings screen can switch data directories while the server runs: a
    /// path fixed at startup would leave every later request reading the old
    /// books under the new directory's name.
    db_path: Arc<RwLock<PathBuf>>,
    pub session_token: Arc<str>,
    pub features: Features,
    pub unlock: Arc<UnlockGate>,
    /// Guards the database *file* rather than its contents.
    ///
    /// `cli::password::encrypt_database` and `decrypt_database` rewrite the
    /// file by rename and delete the `-wal`/`-shm` sidecars. A connection
    /// another request holds across that moment is reading a file that is no
    /// longer the database. Readers take the read side (`routes::with_conn`,
    /// and any handler that opens a connection itself); those two operations
    /// and the data-directory switch take the write side.
    pub db_gate: Arc<TokioRwLock<()>>,
    /// The version of a newer release, once the startup check has found one.
    ///
    /// A slot rather than a value because the check runs in the background:
    /// requests are answered immediately and this fills in when GitHub replies,
    /// so an early `/api/status` reports nothing rather than waiting on the
    /// network.
    update_available: Arc<RwLock<Option<String>>>,
}

impl AppState {
    pub fn new(db_path: PathBuf, session_token: String) -> Self {
        Self {
            db_path: Arc::new(RwLock::new(db_path)),
            session_token: Arc::from(session_token.as_str()),
            features: Features::detect(),
            unlock: Arc::new(UnlockGate::default()),
            db_gate: Arc::new(TokioRwLock::new(())),
            update_available: Arc::new(RwLock::new(None)),
        }
    }

    /// The version of the newer release the startup check found, if any.
    pub fn update_available(&self) -> Option<String> {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        self.update_available.read().unwrap().clone()
    }

    /// Record what the startup update check found.
    pub fn set_update_available(&self, version: Option<String>) {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        *self.update_available.write().unwrap() = version;
    }

    /// The database this server is currently serving.
    ///
    /// Hands back an owned path rather than a guard, so no lock is ever held
    /// across an await point.
    pub fn db_path(&self) -> PathBuf {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        self.db_path.read().unwrap().clone()
    }

    /// The directory the database lives in — where snapshots, backups and
    /// upload spools go.
    pub fn data_dir(&self) -> PathBuf {
        self.db_path()
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf()
    }

    /// Point this server at a different database. Only the data-directory
    /// switch calls this, and only while holding `db_gate` for writing.
    pub fn set_db_path(&self, path: PathBuf) {
        // unwrap: poisoned lock means a thread panicked — unrecoverable
        *self.db_path.write().unwrap() = path;
    }

    /// True when the database is encrypted and this process has not been given
    /// the key yet.
    ///
    /// The encryption state is probed rather than cached at startup: the web
    /// settings screen can encrypt or decrypt the database while the server
    /// runs, and a stale cache would either lock out a valid session or serve a
    /// database whose key was just removed. The password check comes first, so
    /// an unlocked process never touches the filesystem for this.
    pub fn is_locked(&self) -> Result<bool, ApiError> {
        if crate::db::get_db_password().is_some() {
            return Ok(false);
        }
        crate::db::is_encrypted(&self.db_path()).map_err(ApiError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_free_then_doubles_up_to_the_cap() {
        let expected_ms = [0, 0, 1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000];
        for (i, ms) in expected_ms.iter().enumerate() {
            let failures = i as u32 + 1;
            assert_eq!(
                backoff_delay(failures),
                Duration::from_millis(*ms),
                "delay after {failures} failures"
            );
        }
    }

    #[test]
    fn backoff_does_not_overflow_at_absurd_failure_counts() {
        assert_eq!(backoff_delay(u32::MAX), BACKOFF_CAP);
        assert_eq!(backoff_delay(64), BACKOFF_CAP);
        assert_eq!(backoff_delay(0), Duration::ZERO);
    }

    #[test]
    fn attempts_remaining_counts_down_then_saturates() {
        let gate = UnlockGate::default();
        assert_eq!(gate.record_failure(), (2, Duration::ZERO));
        assert_eq!(gate.record_failure(), (1, Duration::ZERO));
        assert_eq!(gate.record_failure(), (0, Duration::from_secs(1)));
        assert_eq!(gate.record_failure(), (0, Duration::from_secs(2)));
    }

    #[test]
    fn reset_restores_the_full_budget() {
        let gate = UnlockGate::default();
        gate.record_failure();
        gate.record_failure();
        gate.reset();
        assert_eq!(gate.record_failure(), (2, Duration::ZERO));
    }
}
