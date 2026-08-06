//! The `nigel serve` HTTP server: an axum app on 127.0.0.1 serving the JSON
//! API and the embedded SPA from the same binary.
//!
//! This is the only async entry point in the crate; `main` stays synchronous
//! and [`run`] owns the tokio runtime.

pub mod auth;
pub mod error;
pub mod routes;
pub mod state;
mod static_files;

use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use axum::routing::get;
use axum::{middleware, Router};
use tokio::net::TcpListener;

use crate::error::{NigelError, Result};
use state::AppState;

/// Start the server and block until shutdown.
pub fn run(port: u16, no_open: bool) -> Result<()> {
    // Shared text formatters are reused for API responses; ANSI escapes have no
    // place in an HTTP body.
    colored::control::set_override(false);

    let db_path = crate::settings::get_data_dir().join("nigel.db");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| NigelError::Other(format!("Couldn't start the async runtime: {e}")))?;

    runtime.block_on(serve(db_path, port, no_open))
}

async fn serve(db_path: PathBuf, port: u16, no_open: bool) -> Result<()> {
    let state = AppState::new(db_path, auth::generate_token());

    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
    let addr = listener.local_addr()?;
    let url = format!(
        "http://127.0.0.1:{}/auth?token={}",
        addr.port(),
        state.session_token
    );

    println!("Nigel is serving on http://127.0.0.1:{}", addr.port());
    println!("Open this URL to start a session:");
    println!("  {url}");
    println!("Press Ctrl-C to stop.");

    if !no_open {
        if let Err(e) = open::that_detached(&url) {
            eprintln!("notice: couldn't open a browser ({e}) — open the URL above yourself.");
        }
    }

    serve_with_shutdown(listener, state, shutdown_signal()).await?;
    println!("Server stopped.");
    Ok(())
}

async fn serve_with_shutdown(
    listener: TcpListener,
    state: AppState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

/// Layers run outermost-first: Host/Origin, then the session cookie, then the
/// route. The session layer wraps the `/api` router itself rather than using
/// `route_layer`, so it also covers that router's 404 fallback — otherwise an
/// unauthenticated request to an unknown `/api` path would skip the check.
/// `/auth` and the static assets sit outside the nest and need no session.
fn build_router(state: AppState) -> Router {
    let api = routes::api_router().layer(middleware::from_fn_with_state(
        state.clone(),
        auth::session_guard,
    ));

    Router::new()
        .nest("/api", api)
        .route("/auth", get(auth::auth_redirect))
        .fallback(static_files::static_handler)
        .layer(middleware::from_fn(auth::host_guard))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use axum::response::Response;
    use tower::ServiceExt;

    const HOST: &str = "127.0.0.1:5731";

    fn test_app() -> (Router, String) {
        let token = auth::generate_token();
        let state = AppState::new(PathBuf::from("/nonexistent/nigel.db"), token.clone());
        (build_router(state), token)
    }

    fn get_request(uri: &str) -> axum::http::request::Builder {
        Request::builder().uri(uri).header(header::HOST, HOST)
    }

    async fn body_string(response: Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body");
        String::from_utf8(bytes.to_vec()).expect("utf-8 body")
    }

    fn content_type(response: &Response) -> String {
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string()
    }

    async fn json_body(response: Response) -> serde_json::Value {
        serde_json::from_str(&body_string(response).await).expect("json body")
    }

    #[tokio::test]
    async fn api_without_session_is_unauthorized() {
        let (app, _token) = test_app();
        let response = app
            .oneshot(get_request("/api/ping").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(content_type(&response).starts_with("application/json"));
        let json = json_body(response).await;
        assert_eq!(json["error"]["code"], "unauthorized");
    }

    #[tokio::test]
    async fn non_local_host_is_forbidden_before_the_session_check() {
        let (app, token) = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/ping")
                    .header(header::HOST, "evil.com")
                    .header(header::COOKIE, format!("nigel_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let json = json_body(response).await;
        assert_eq!(json["error"]["code"], "forbidden");
    }

    #[tokio::test]
    async fn missing_host_header_is_forbidden() {
        let (app, _token) = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/ping")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn cross_origin_request_is_forbidden() {
        let (app, token) = test_app();
        let response = app
            .oneshot(
                get_request("/api/ping")
                    .header(header::ORIGIN, "http://evil.com")
                    .header(header::COOKIE, format!("nigel_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn auth_sets_cookie_and_redirects() {
        let (app, token) = test_app();
        let response = app
            .oneshot(
                get_request(&format!("/auth?token={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(response.headers()[header::LOCATION], "/");

        let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie.contains(&format!("nigel_session={token}")));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Path=/"));
    }

    #[tokio::test]
    async fn auth_with_wrong_token_is_unauthorized_and_sets_no_cookie() {
        let (app, _token) = test_app();
        let response = app
            .oneshot(
                get_request("/auth?token=deadbeef")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }

    #[tokio::test]
    async fn auth_without_token_is_unauthorized() {
        let (app, _token) = test_app();
        let response = app
            .oneshot(get_request("/auth").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn session_cookie_from_auth_unlocks_ping() {
        let (app, token) = test_app();
        let auth = app
            .oneshot(
                get_request(&format!("/auth?token={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = auth.headers()[header::SET_COOKIE].to_str().unwrap();
        let pair = cookie.split(';').next().unwrap().to_string();

        let (app, _) = {
            let state = AppState::new(PathBuf::from("/nonexistent/nigel.db"), token.clone());
            (build_router(state), ())
        };
        let response = app
            .oneshot(
                get_request("/api/ping")
                    .header(header::COOKIE, pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = json_body(response).await;
        assert_eq!(json["ok"], true);
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn index_is_served_at_root_without_a_session() {
        let (app, _token) = test_app();
        let response = app
            .oneshot(get_request("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(content_type(&response).starts_with("text/html"));
        assert!(body_string(response).await.contains("placeholder-notice"));
    }

    #[tokio::test]
    async fn unknown_spa_path_falls_back_to_index() {
        let (app, _token) = test_app();
        let index = app
            .oneshot(get_request("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let index_body = body_string(index).await;

        let (app, _token) = test_app();
        let response = app
            .oneshot(
                get_request("/some/deep/spa/route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(content_type(&response).starts_with("text/html"));
        assert_eq!(body_string(response).await, index_body);
    }

    #[tokio::test]
    async fn unknown_api_path_is_a_json_404() {
        let (app, token) = test_app();
        let response = app
            .oneshot(
                get_request("/api/does-not-exist")
                    .header(header::COOKIE, format!("nigel_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(content_type(&response).starts_with("application/json"));
        let json = json_body(response).await;
        assert_eq!(json["error"]["code"], "not_found");
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("/api/does-not-exist"));
    }

    #[tokio::test]
    async fn graceful_shutdown_ends_the_server() {
        let state = AppState::new(
            PathBuf::from("/nonexistent/nigel.db"),
            auth::generate_token(),
        );
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("bind ephemeral port");
        assert_ne!(listener.local_addr().unwrap().port(), 0);

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(serve_with_shutdown(listener, state, async move {
            let _ = rx.await;
        }));

        tx.send(()).expect("signal shutdown");
        let joined = tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .expect("server did not shut down within 5s")
            .expect("server task panicked");
        assert!(joined.is_ok());
    }
}
