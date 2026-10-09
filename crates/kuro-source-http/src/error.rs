//! Mapping of transport failures to the source error model (`OkHttpExtensions.kt`, `HttpException.kt`; T-15a-023).

use kuro_net::NetError;
use kuro_source::SourceError;

/// A non-2xx status is `Http(code)`. A cancelled call is `Cancelled`, a bad URL is `Url`. Connect, TLS, timeout and
/// read failures are `Transport` with the transport text, which the UI turns into its offline or host message.
pub fn map_net_error(err: NetError) -> SourceError {
    match err {
        NetError::Http { code } => SourceError::Http(code),
        NetError::Canceled => SourceError::Cancelled,
        NetError::InvalidUrl(text) => SourceError::Url(text),
        other => SourceError::Transport(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_and_cancellation_keep_their_meaning() {
        assert_eq!(map_net_error(NetError::Http { code: 503 }), SourceError::Http(503));
        assert_eq!(map_net_error(NetError::Canceled), SourceError::Cancelled);
        assert_eq!(map_net_error(NetError::InvalidUrl("no host".into())), SourceError::Url("no host".into()));
    }

    #[test]
    fn a_timeout_and_a_tls_failure_are_transport_errors_with_their_text() {
        assert_eq!(map_net_error(NetError::Timeout), SourceError::Transport("timeout".into()));
        assert_eq!(map_net_error(NetError::Tls("bad cert".into())), SourceError::Transport("bad cert".into()));
    }
}
