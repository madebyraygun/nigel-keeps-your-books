pub mod clients;
pub mod gateway;
pub mod import_invoiceshelf;
pub mod invoices;
pub mod mailgun;
pub mod r2;
pub mod render;
pub mod render_html;
pub mod send;
pub mod stripe;
pub mod sync;

use std::time::Duration;

/// How long an invoicing request may spend reaching the far end.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long an invoicing request may take in total, connection included.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The one HTTP client Stripe, R2 and Mailgun are all reached through.
///
/// `reqwest` has no default timeout: a TCP connection that is accepted and then
/// never answered hangs until the OS gives up, which is a wedged terminal for
/// the CLI and a blocking thread the server never gets back. Bounding it here
/// rather than at each call site is what makes "every outbound invoicing
/// request is bounded" a property of the module instead of a habit.
pub(crate) fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        // What `reqwest::blocking::Client::new()` does, and for the same reason:
        // the only way this fails is a TLS backend that cannot initialise, which
        // no amount of error handling at the call site would make recoverable.
        .expect("build the invoicing HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_http_client_is_bounded_at_both_ends() {
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(30));
        // Builds with those settings applied — a rejected combination would
        // panic here rather than on the first send.
        let _client = http_client();
    }

    /// The bound is only real if nothing reaches the network around it. Read at
    /// compile time, so this costs no IO and cannot go stale against a moved
    /// file.
    #[test]
    fn no_invoicing_client_builds_its_own_unbounded_reqwest_client() {
        let sources = [
            ("stripe.rs", include_str!("stripe.rs")),
            ("r2.rs", include_str!("r2.rs")),
            ("mailgun.rs", include_str!("mailgun.rs")),
        ];
        for (name, source) in sources {
            assert!(
                !source.contains("blocking::Client::new()"),
                "{name} builds a reqwest client with no timeout"
            );
            assert!(
                source.contains("http_client()"),
                "{name} does not go through the shared client"
            );
        }
    }
}
