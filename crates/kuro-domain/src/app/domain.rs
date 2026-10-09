//! The domain container (`DomainModule.kt`, `SYDomainModule.kt`; T-10a-001, PARTIAL). It owns the handles the interactors
//! share and gives each one out by a cheap clone, so nothing is a global.
//!
//! Built: the source, backup and download preferences, the storage manager, the source toggles, the manga and anime
//! repositories, and the extension-store repository (accessors above). The construction test uses `kuro-testkit` fakes.
//!
//! Deferred, with the exact gap:
//! - None of the 47 owner-`area:10` rows of `p1-topic-domainmodule-registry.md` has its accessor on `Domain` yet. Some
//!   of their behaviour is built under other names and is not wired here: `IncognitoState` (`source/incognito.rs`) and
//!   `TrustExtension` (`extension/trust.rs`). Most of the remaining rows have no type in the workspace yet.
//! - `Domain::start()` needs the delayed tracking queue, and the `DelayedTrackingStore` singletons (manga, anime) are
//!   named only in the T-10a-001 acceptance, so they wait for the tracking work.
//! - `TrackPreferences` is area 27's, and its task is T-10a-026 (milestone M08).

use std::path::PathBuf;
use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::extension_store::ExtensionStoreRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::handle::Db;
use kuro_prefs::source::SourcePrefs;
use kuro_prefs::store::PrefStore;

use super::source::SourceToggles;
use crate::backup::BackupPrefs;
use crate::download::prefs::DownloadPrefs;
use crate::storage::StorageManager;

/// What a [`Domain`] is built from: the preference store, the two databases, and the base storage directory used until
/// the user picks one.
pub struct DomainDeps {
    pub prefs: PrefStore,
    pub manga_db: Arc<Db>,
    pub anime_db: Arc<Db>,
    /// The platform data directory plus `kurobayomi`, from `kuro_platform::paths::Paths::data_dir`.
    pub default_storage: PathBuf,
}

/// The shared domain handles. Clones are cheap: every field is an `Arc` or a handle that holds one.
#[derive(Clone)]
pub struct Domain {
    source_prefs: SourcePrefs,
    backup_prefs: BackupPrefs,
    download_prefs: DownloadPrefs,
    storage: Arc<StorageManager>,
    toggles: Arc<SourceToggles>,
    manga: MangaRepo,
    anime: AnimeRepo,
    extension_stores: ExtensionStoreRepo,
    anime_db: Arc<Db>,
}

impl Domain {
    /// Builds the container. Nothing is read from disk here; the storage tree is created on first use (see
    /// `StorageManager::ensure_tree`).
    pub fn new(deps: DomainDeps) -> Self {
        let DomainDeps { prefs, manga_db, anime_db, default_storage } = deps;
        Self {
            source_prefs: SourcePrefs::new(prefs.clone()),
            backup_prefs: BackupPrefs::new(&prefs),
            download_prefs: DownloadPrefs::new(&prefs),
            storage: Arc::new(StorageManager::open(&prefs, default_storage)),
            toggles: Arc::new(SourceToggles::default()),
            manga: MangaRepo::new(manga_db),
            anime: AnimeRepo::new(Arc::clone(&anime_db)),
            extension_stores: ExtensionStoreRepo::new(Arc::clone(&anime_db)),
            anime_db,
        }
    }

    pub fn source_prefs(&self) -> &SourcePrefs {
        &self.source_prefs
    }

    pub fn backup_prefs(&self) -> &BackupPrefs {
        &self.backup_prefs
    }

    pub fn download_prefs(&self) -> &DownloadPrefs {
        &self.download_prefs
    }

    pub fn storage(&self) -> Arc<StorageManager> {
        Arc::clone(&self.storage)
    }

    pub fn source_toggles(&self) -> Arc<SourceToggles> {
        Arc::clone(&self.toggles)
    }

    pub fn manga(&self) -> &MangaRepo {
        &self.manga
    }

    pub fn anime(&self) -> &AnimeRepo {
        &self.anime
    }

    pub fn extension_stores(&self) -> &ExtensionStoreRepo {
        &self.extension_stores
    }

    /// The anime database handle, for the live store lists (`extension_store::subscribe_all`).
    pub fn anime_db(&self) -> &Arc<Db> {
        &self.anime_db
    }
}

#[cfg(test)]
mod tests {
    use kuro_testkit::{InMemorySettings, TempDir, TestDb};

    use super::*;

    /// The container's inputs built from `kuro-testkit` fakes: in-memory settings, and migrated databases in `dir`.
    fn deps(dir: &TempDir, settings: &InMemorySettings) -> DomainDeps {
        DomainDeps {
            prefs: PrefStore::clone(settings),
            manga_db: TestDb::manga_handle(dir),
            anime_db: TestDb::anime_handle(dir),
            default_storage: dir.path().join("no-such-base"),
        }
    }

    #[test]
    fn a_domain_builds_from_testkit_fakes_and_shares_its_handles() {
        let dir = TempDir::new().expect("temp dir");
        let settings = InMemorySettings::new();
        let domain = Domain::new(deps(&dir, &settings));
        assert!(domain.source_prefs().enabled_languages.get().contains("en"));
        assert_eq!(domain.backup_prefs().interval.get(), 12);
        let clone = domain.clone();
        assert!(Arc::ptr_eq(&domain.storage(), &clone.storage()), "clones share the storage manager");
        assert!(Arc::ptr_eq(&domain.source_toggles(), &clone.source_toggles()));
        assert!(Arc::ptr_eq(domain.anime_db(), clone.anime_db()), "clones share the anime database");
        assert!(domain.manga().by_id(1).is_err(), "an empty database has no entry 1");
    }

    #[test]
    fn the_container_reads_the_same_preference_store_it_was_built_from() {
        let dir = TempDir::new().expect("temp dir");
        let settings = InMemorySettings::new();
        let domain = Domain::new(deps(&dir, &settings));
        domain.backup_prefs().interval.set(0);
        assert_eq!(settings.get_raw("backup_interval"), Some(kuro_prefs::values::PrefValue::Int(0)));
        assert!(matches!(domain.storage().base(), Err(crate::error::DomainError::StorageUnavailable)));
    }
}
