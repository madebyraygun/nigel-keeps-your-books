//! Shared server state.

use std::path::PathBuf;
use std::sync::Arc;

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
/// connection from `db_path` inside `spawn_blocking`; there is no pool.
#[derive(Debug, Clone)]
pub struct AppState {
    pub db_path: Arc<PathBuf>,
    pub session_token: Arc<str>,
    pub features: Features,
}

impl AppState {
    pub fn new(db_path: PathBuf, session_token: String) -> Self {
        Self {
            db_path: Arc::new(db_path),
            session_token: Arc::from(session_token.as_str()),
            features: Features::detect(),
        }
    }
}
