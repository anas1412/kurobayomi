//! The preference aggregate: every section built once over one shared store.
//!
//! Built in the application context at startup, before any widget is created. Sections that are
//! not in this file yet (tracking, downloads, backup, storage, torrent, the subtitle, audio and
//! advanced player sections) are added with their own tasks.

use std::sync::Arc;

use kuro_platform::paths::Paths;
use kuro_platform::secrets::SecretStore;

use crate::PrefError;
use crate::base::BasePrefs;
use crate::library::LibraryPrefs;
use crate::network::NetworkPrefs;
use crate::player::{DecoderPrefs, GesturePrefs, PlayerPrefs};
use crate::reader::ReaderPrefs;
use crate::security::SecurityPrefs;
use crate::settings_load;
use crate::source::SourcePrefs;
use crate::store::PrefStore;
use crate::ui::UiPrefs;

/// Every preference section. Clones of a section share the store.
pub struct Prefs {
    pub base: BasePrefs,
    pub ui: UiPrefs,
    pub sources: SourcePrefs,
    pub library: LibraryPrefs,
    pub network: NetworkPrefs,
    pub security: SecurityPrefs,
    pub reader: ReaderPrefs,
    pub player: PlayerPrefs,
    pub gestures: GesturePrefs,
    pub decoder: DecoderPrefs,
    /// Credential storage for tracker secrets (area 02, part 2 reads through it).
    pub secrets: Arc<dyn SecretStore>,
    store: PrefStore,
}

impl Prefs {
    /// Opens `config.toml` and `state.toml` from `paths`, builds every section and repairs invalid
    /// stored enum and object values to their defaults.
    pub fn open(paths: &dyn Paths, secrets: Arc<dyn SecretStore>) -> Result<Self, PrefError> {
        let prefs = Self::with_store(settings_load::open(paths)?, secrets);
        prefs.repair()?;
        Ok(prefs)
    }

    /// Builds the sections over an existing store. Nothing is repaired.
    pub fn with_store(store: PrefStore, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            base: BasePrefs::new(store.clone()),
            ui: UiPrefs::new(store.clone()),
            sources: SourcePrefs::new(store.clone()),
            library: LibraryPrefs::new(store.clone()),
            network: NetworkPrefs::new(store.clone()),
            security: SecurityPrefs::new(store.clone()),
            reader: ReaderPrefs::new(store.clone()),
            player: PlayerPrefs::new(store.clone()),
            gestures: GesturePrefs::new(store.clone()),
            decoder: DecoderPrefs::new(store.clone()),
            secrets,
            store,
        }
    }

    /// The store every section reads and writes.
    pub fn store(&self) -> &PrefStore {
        &self.store
    }

    /// Startup repair: each section rewrites its invalid stored values to the defaults.
    fn repair(&self) -> Result<(), PrefError> {
        self.base.repair()?;
        self.ui.repair()?;
        self.sources.repair()?;
        self.library.repair()?;
        self.network.repair()?;
        self.security.repair()?;
        self.reader.repair()?;
        self.player.repair()?;
        self.gestures.repair()?;
        self.decoder.repair()
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::paths::Paths;

    use super::*;
    use crate::reader::ReaderHideThreshold;
    use crate::sort::MangaLibrarySort;
    use crate::testutil::{MemorySecrets, TestDir, TestPaths};
    use crate::toml_backend::config_file;
    use crate::values::PrefValue;

    fn secrets() -> Arc<dyn SecretStore> {
        Arc::new(MemorySecrets::default())
    }

    #[test]
    fn sections_share_one_store() {
        let prefs = Prefs::with_store(PrefStore::in_memory(), secrets());
        prefs.base.incognito_mode.set(true).unwrap();
        assert_eq!(
            prefs.store().get_raw("__APP_STATE_incognito_mode"),
            Some(PrefValue::Bool(true))
        );
        prefs.reader.default_reading_mode.set(4).unwrap();
        assert_eq!(prefs.reader.default_reading_mode.get(), 4);
        assert_eq!(prefs.store().get_raw("pref_default_reading_mode_key"), Some(PrefValue::Int(4)));
    }

    #[test]
    fn open_builds_defaults_on_first_run() {
        let paths = TestPaths(TestDir::new("prefs-first-run"));
        let prefs = Prefs::open(&paths, secrets()).unwrap();
        assert_eq!(prefs.reader.reader_hide_threshold.get(), ReaderHideThreshold::LOW);
        assert_eq!(prefs.library.manga_sorting_mode.get().sort(), MangaLibrarySort::default_sort());
    }

    #[test]
    fn startup_repair_rewrites_invalid_enum_and_sort_values() {
        let paths = TestPaths(TestDir::new("prefs-repair"));
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(
            config_file(&paths.config_dir()),
            "[reader]\nreader_hide_threshold = \"NOPE\"\n\n[library]\nmanga_sorting_mode = \"ALPHABETICAL\"\n",
        )
        .unwrap();
        let prefs = Prefs::open(&paths, secrets()).unwrap();
        assert_eq!(
            prefs.store().get_raw("reader_hide_threshold"),
            Some(PrefValue::Str("LOW".into()))
        );
        assert_eq!(
            prefs.store().get_raw("library_sorting_mode"),
            Some(PrefValue::Str("ALPHABETICAL,ASCENDING".into()))
        );
    }

    #[test]
    fn a_valid_value_is_not_repaired() {
        let paths = TestPaths(TestDir::new("prefs-valid"));
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(
            config_file(&paths.config_dir()),
            "[reader]\nreader_hide_threshold = \"HIGH\"\n",
        )
        .unwrap();
        let prefs = Prefs::open(&paths, secrets()).unwrap();
        assert_eq!(prefs.reader.reader_hide_threshold.get(), ReaderHideThreshold::HIGH);
    }
}
