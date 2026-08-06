//! Embedded SPA hosting.
//!
//! `web/dist` is compiled into the binary (in debug builds too, via the
//! `debug-embed` feature) so a released `nigel` has no filesystem dependency.
//! Until the real SPA lands, `web/dist/index.html` is a committed placeholder.

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/dist/"]
struct Assets;

const INDEX: &str = "index.html";

/// Serve an embedded asset, falling back to the SPA shell so client-side
/// routes survive a page reload. Unknown `/api` paths never reach this handler.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.split('/').any(|segment| segment == "..") {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }

    let path = if path.is_empty() { INDEX } else { path };
    if let Some(file) = Assets::get(path) {
        return embedded_response(file);
    }

    match Assets::get(INDEX) {
        Some(file) => embedded_response(file),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Web assets are missing from this build.",
        )
            .into_response(),
    }
}

fn embedded_response(file: rust_embed::EmbeddedFile) -> Response {
    let mime = file.metadata.mimetype().to_owned();
    (
        [(header::CONTENT_TYPE, mime)],
        Body::from(file.data.into_owned()),
    )
        .into_response()
}
