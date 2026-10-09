//! `[network]` preferences (Aniyomi `NetworkPreferences`) and the DNS-over-HTTPS provider table.

use crate::registry::define_prefs;

define_prefs! {
    /// `[network]` section: the three keys of Aniyomi `NetworkPreferences`.
    pub struct NetworkPrefs in "network" {
        /// Kotlin passes `isDebugBuildType` to the constructor, so the Linux default is true in
        /// debug builds. The registry row keeps the class default (false).
        verbose_logging: bool = "verbose_logging" => cfg!(debug_assertions);
        /// `-1` is off; see [`doh_endpoint`] for the ids.
        doh_provider: i32 = "doh_provider" => -1;
        default_user_agent: String = "default_user_agent" => String::from(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0",
        );
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
    use kuro_platform::paths::Paths;

    use super::*;
    use crate::toml_backend::config_file;
    use crate::settings_load::open;
    use crate::store::PrefStore;
    use crate::testutil::{TestDir, TestPaths, assert_section_registered, read_toml};
    use crate::values::PrefValue;

    #[test]
    fn keys_are_the_aniyomi_keys_in_their_network_fields() {
        assert_eq!(
            NetworkPrefs::FIELDS,
            &[
                ("verbose_logging", "verbose_logging"),
                ("doh_provider", "doh_provider"),
                ("default_user_agent", "default_user_agent"),
            ]
        );
        assert_eq!(NetworkPrefs::TABLE, "network");
    }

    #[test]
    fn every_field_is_registered_under_the_network_table() {
        let net = NetworkPrefs::new(PrefStore::in_memory());
        assert_section_registered(NetworkPrefs::TABLE, NetworkPrefs::FIELDS, &net.defaults());
    }

    #[test]
    fn defaults_are_the_plan_defaults() {
        let net = NetworkPrefs::new(PrefStore::in_memory());
        assert_eq!(net.verbose_logging.get(), cfg!(debug_assertions));
        assert_eq!(net.doh_provider.get(), -1);
        assert_eq!(
            net.default_user_agent.get(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"
        );
    }

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

    #[test]
    fn network_values_round_trip_through_config_toml() {
        let paths = TestPaths(TestDir::new("network-round-trip"));
        {
            let store = open(&paths).unwrap();
            let net = NetworkPrefs::new(store.clone());
            net.verbose_logging.set(true).unwrap();
            net.doh_provider.set(3).unwrap();
            net.default_user_agent.set("kuro/1.0".into()).unwrap();
            store.flush().unwrap();
        }
        let store = open(&paths).unwrap();
        let net = NetworkPrefs::new(store.clone());
        assert!(net.verbose_logging.get());
        assert_eq!(net.doh_provider.get(), 3);
        assert_eq!(net.default_user_agent.get(), "kuro/1.0");
        assert_eq!(
            store.get_raw("doh_provider"),
            Some(PrefValue::Int(3)),
            "an Int stays an Int"
        );
        let doc = read_toml(&config_file(&paths.config_dir()));
        assert_eq!(doc["network"]["doh_provider"].as_integer(), Some(3));
        assert_eq!(doc["network"]["verbose_logging"].as_bool(), Some(true));
        assert_eq!(
            doc["network"]["default_user_agent"].as_str(),
            Some("kuro/1.0")
        );
    }
}
