//! Embedded SPA hosting.
//!
//! `web/dist` is compiled into the binary (in debug builds too, via the
//! `debug-embed` feature) so a released `nigel` has no filesystem dependency.
//! The directory is built by `npm run build` in `web/` and is not tracked by
//! git; `build.rs` seeds it from `web/placeholder/index.html` when it is
//! missing, and is also what tells cargo to re-embed after a fresh SPA build.

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/dist/"]
struct Assets;

const INDEX: &str = "index.html";

/// Where vite puts the files whose names carry a content hash. A given name
/// under here always means the same bytes, which is what makes them cacheable
/// forever — and what makes a miss here a real 404 rather than the shell.
const HASHED_ASSETS: &str = "assets/";

/// The shell names the hashed bundles it needs, so a cached copy outlives the
/// binary that produced it: after an upgrade it would ask for filenames this
/// build no longer embeds.
const SHELL_CACHE: &str = "no-store";
const HASHED_CACHE: &str = "public, max-age=31536000, immutable";

/// Serve an embedded asset, falling back to the SPA shell so client-side
/// routes survive a page reload. Unknown `/api` paths never reach this handler.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.split('/').any(|segment| segment == "..") {
        return not_found();
    }

    let path = if path.is_empty() { INDEX } else { path };
    let hashed = path.starts_with(HASHED_ASSETS);

    if let Some(file) = Assets::get(path) {
        return embedded_response(file, if hashed { HASHED_CACHE } else { SHELL_CACHE });
    }

    // Answering a bundle request with the shell hands the browser HTML under a
    // `200` and a script's content type, which fails on the MIME check instead
    // of reporting the miss.
    if hashed {
        return not_found();
    }

    match Assets::get(INDEX) {
        Some(file) => embedded_response(file, SHELL_CACHE),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Web assets are missing from this build.",
        )
            .into_response(),
    }
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

fn embedded_response(file: rust_embed::EmbeddedFile, cache_control: &'static str) -> Response {
    let mime = file.metadata.mimetype().to_owned();
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, cache_control.to_owned()),
        ],
        Body::from(file.data.into_owned()),
    )
        .into_response()
}
