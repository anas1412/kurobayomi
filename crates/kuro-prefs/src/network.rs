//! `[network]` preferences (Aniyomi `NetworkPreferences`) and the DNS-over-HTTPS provider table.

use crate::registry::define_prefs;

define_prefs! {
    /// `[network]` section: no keys: the settings diet cut verbose logging, DNS over HTTPS and the user agent, whose hard-coded
    /// defaults are in `kuro_net::settings`.
    pub struct NetworkPrefs in "network" {
    }
}

/// DNS-over-HTTPS endpoints by `doh_provider` id (Kotlin `DohProviders.kt`).
const DOH_PROVIDERS: &[(i32, &str)] = &[
    (1, "https://cloudflare-dns.com/dns-query"),
    (2, "https://dns.google/dns-query"),
    (3, "https://dns-unfiltered.adguard.com/dns-query"),
    (4, "https://dns.quad9.net/dns-query"),
    (5, "https://dns.alidns.com/dns-query"),
    (6, "https://doh.pub/dns-query"),
    (7, "https://doh.360.cn/dns-query"),
    (8, "https://dns.twnic.tw/dns-query"),
    (9, "https://dns.mullvad.net/dns-query"),
    (10, "https://freedns.controld.com/p0"),
    (11, "https://dns.njal.la/dns-query"),
    (12, "https://free.shecan.ir/dns-query"),
    (13, "https://doh.libredns.gr/dns-query"),
];

/// The DNS-over-HTTPS endpoint for a `doh_provider` id. `-1`, `0` and unknown ids give `None`
/// (system DNS).
pub fn doh_endpoint(provider: i32) -> Option<&'static str> {
    DOH_PROVIDERS
        .iter()
        .find(|(id, _)| *id == provider)
        .map(|(_, url)| *url)
}

/// The `tracing` level for the HTTP target while the app runs: `debug` when verbose logging is
/// on, `warn` otherwise.
pub fn http_log_level(verbose: bool) -> &'static str {
    if verbose { "debug" } else { "warn" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doh_ids_one_to_thirteen_have_an_endpoint_and_others_do_not() {
        for id in 1..=13 {
            assert!(doh_endpoint(id).is_some(), "id {id}");
        }
        for id in [-1, 0, 14] {
            assert_eq!(doh_endpoint(id), None, "id {id}");
        }
        assert_eq!(
            doh_endpoint(1),
            Some("https://cloudflare-dns.com/dns-query")
        );
        assert_eq!(doh_endpoint(13), Some("https://doh.libredns.gr/dns-query"));
    }

    #[test]
    fn the_mullvad_endpoint_has_no_leading_space() {
        assert_eq!(doh_endpoint(9), Some("https://dns.mullvad.net/dns-query"));
    }

    #[test]
    fn verbose_logging_switches_the_http_level() {
        assert_eq!(http_log_level(true), "debug");
        assert_eq!(http_log_level(false), "warn");
    }
}
