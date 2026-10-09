//! Network settings read from `Settings.network` (T-12a-007). The keys and defaults live in `kuro-prefs`; this
//! type is the plain snapshot the client is built from.

use kuro_prefs::network::NetworkPrefs;

/// A snapshot of the `[network]` preferences, taken when the client is built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkSettings {
    pub verbose_logging: bool,
    /// `-1` is off; see `kuro_prefs::network::doh_endpoint`.
    pub doh_provider: i32,
    /// The default `User-Agent`, trimmed.
    pub default_user_agent: String,
}

impl NetworkSettings {
    pub fn from_prefs(prefs: &NetworkPrefs) -> Self {
        Self {
            verbose_logging: prefs.verbose_logging.get(),
            doh_provider: prefs.doh_provider.get(),
            default_user_agent: prefs.default_user_agent.get().trim().to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;

    #[test]
    fn a_missing_file_yields_the_defaults() {
        let prefs = NetworkPrefs::new(PrefStore::in_memory());
        let settings = NetworkSettings::from_prefs(&prefs);
        assert_eq!(settings.verbose_logging, cfg!(debug_assertions));
        assert_eq!(settings.doh_provider, -1);
        assert_eq!(
            settings.default_user_agent,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"
        );
    }

    #[test]
    fn the_user_agent_is_trimmed_on_read() {
        let prefs = NetworkPrefs::new(PrefStore::in_memory());
        prefs.default_user_agent.set("  kuro/1.0\t ".into());
        assert_eq!(NetworkSettings::from_prefs(&prefs).default_user_agent, "kuro/1.0");
    }
}
