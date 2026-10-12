//! Backup build (area 21, T-21a-012): gathers the library through `LibraryReader`, maps it with the creators and
//! assembles the `Backup` container. Reads go through the reader only, so the database adapter owns the consistent
//! view of the whole pass (`LibraryReader::snapshot`, E-05) and the gating of the non-favourite queries lives here (E-09).

pub mod db;
pub mod entries;
pub mod extensions;
pub mod meta;
pub mod preferences;

use kuro_data::anime::Anime;
use kuro_data::category::Category;
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;
use kuro_data::extension_repo::ExtensionRepo;
use kuro_data::history::{AnimeHistory, MangaHistory};
use kuro_data::manga::Manga;
use kuro_data::track::{AnimeTrack, MangaTrack};
use kuro_platform::kind::Kind;

use crate::error::BackupError;
use crate::options::BackupOptions;
use crate::schema::{Backup, PrefValue};

/// A manga with the rows the creator needs, loaded by the reader.
pub struct MangaEntry {
    pub manga: Manga,
    /// All chapters of the manga, unfiltered (`applyScanlatorFilter = 0`).
    pub chapters: Vec<Chapter>,
    pub excluded_scanlators: std::collections::BTreeSet<String>,
    pub categories: Vec<Category>,
    pub tracks: Vec<MangaTrack>,
    pub history: Vec<MangaHistory>,
}

/// An anime with the rows the creator needs.
pub struct AnimeEntry {
    pub anime: Anime,
    pub episodes: Vec<Episode>,
    pub categories: Vec<Category>,
    pub tracks: Vec<AnimeTrack>,
    pub history: Vec<AnimeHistory>,
}

/// What one build pass returns: the container and the extension packages skipped for a missing artifact.
pub type Built = Result<(Backup, Vec<String>), BackupError>;

/// Read side of the library. `build_backup_report` runs all of its reads inside one [`LibraryReader::snapshot`] (E-05).
pub trait LibraryReader {
    /// Runs `f`, the whole build pass, so that every read it makes sees one state of the library. The default
    /// adds no isolation, for readers over data that cannot change underneath the build.
    fn snapshot(&self, f: &mut dyn FnMut() -> Built) -> Built {
        f()
    }

    fn manga_favorites(&self) -> Result<Vec<MangaEntry>, BackupError>;
    /// Read manga that are not in the library (B-08a). Called only when `read_entries` and `library_entries` are on.
    fn manga_read_not_in_library(&self) -> Result<Vec<MangaEntry>, BackupError>;
    fn anime_favorites(&self) -> Result<Vec<AnimeEntry>, BackupError>;
    /// Watched anime that are not in the library (B-08b).
    fn anime_watched_not_in_library(&self) -> Result<Vec<AnimeEntry>, BackupError>;
    /// All manga categories, system category (id 0) included; the creator filters it.
    fn manga_categories(&self) -> Result<Vec<Category>, BackupError>;
    fn anime_categories(&self) -> Result<Vec<Category>, BackupError>;
    fn manga_extension_repos(&self) -> Result<Vec<ExtensionRepo>, BackupError>;
}

/// Source identity for source references and stub names (B-08d, B-14).
pub trait SourceRegistry {
    fn is_installed(&self, source_id: i64) -> bool;
    /// Installed display name, else the stub name, else "" (E-17).
    fn name_of(&self, source_id: i64) -> String;
    /// "Name (LANG)" for a valid stub, else the numeric id as text (B-14).
    fn stub_label(&self, source_id: i64) -> String;
}

/// App-level preferences as stored, before the key rules.
pub trait PrefStore {
    fn all(&self) -> Vec<(String, PrefValue)>;
}

/// Per-source preferences for configurable sources. `kind` selects the manga or anime list (B-08h).
pub trait SourcePrefStore {
    fn configurable(&self, kind: Kind) -> Vec<(String, Vec<(String, PrefValue)>)>;
}

pub use extensions::ExtensionArtifacts;

pub struct CreateInputs<'a> {
    pub library: &'a dyn LibraryReader,
    pub sources: &'a dyn SourceRegistry,
    pub prefs: &'a dyn PrefStore,
    pub source_prefs: &'a dyn SourcePrefStore,
    /// Read only when `BackupOptions::extensions` is on. `None` writes no extensions.
    pub extensions: Option<&'a dyn ExtensionArtifacts>,
}

/// Builds the uncompressed `Backup` (B-06 steps 2 to 12).
pub fn build_backup(inputs: &CreateInputs<'_>, opts: &BackupOptions) -> Result<Backup, BackupError> {
    build_backup_report(inputs, opts).map(|(backup, _skipped)| backup)
}

/// Like `build_backup`, also returning the packages whose extension artifact was missing and skipped (E-16), for the
/// completion notification.
pub fn build_backup_report(inputs: &CreateInputs<'_>, opts: &BackupOptions) -> Built {
    inputs.library.snapshot(&mut || build_pass(inputs, opts))
}

fn build_pass(inputs: &CreateInputs<'_>, opts: &BackupOptions) -> Built {
    let lib = inputs.library;

    // E-09: the non-favourite queries run only when their result can reach the output.
    let mut manga_entries = Vec::new();
    let mut anime_entries = Vec::new();
    if opts.library_entries {
        manga_entries = lib.manga_favorites()?;
        if opts.read_entries {
            manga_entries.extend(lib.manga_read_not_in_library()?);
        }
        anime_entries = lib.anime_favorites()?;
        if opts.read_entries {
            anime_entries.extend(lib.anime_watched_not_in_library()?);
        }
    }

    let manga = manga_entries
        .iter()
        .map(|e| entries::manga_entry(e, opts))
        .collect::<Result<Vec<_>, _>>()?;
    let anime = anime_entries
        .iter()
        .map(|e| entries::anime_entry(e, opts))
        .collect::<Result<Vec<_>, _>>()?;

    let categories = if opts.categories {
        meta::categories(&lib.manga_categories()?)
    } else {
        Vec::new()
    };
    let anime_categories = if opts.categories {
        meta::categories(&lib.anime_categories()?)
    } else {
        Vec::new()
    };

    let sources = meta::manga_sources(&manga, inputs.sources);
    let anime_sources = meta::anime_sources(&anime, inputs.sources);

    let preferences = if opts.app_settings {
        preferences::app(&inputs.prefs.all(), opts.effective_private())
    } else {
        Vec::new()
    };
    let source_preferences = if opts.source_settings {
        let mut list = preferences::sources(&inputs.source_prefs.configurable(Kind::Anime), opts.effective_private());
        list.extend(preferences::sources(&inputs.source_prefs.configurable(Kind::Manga), opts.effective_private()));
        list
    } else {
        Vec::new()
    };

    // Anime stores and custom buttons are cut: the field stays in the file (empty) so Aniyomi reads it, and restore skips them.
    let anime_extension_stores = Vec::new();
    let custom_buttons = Vec::new();
    let manga_extension_repos = if opts.extension_stores { meta::manga_repos(&lib.manga_extension_repos()?) } else { Vec::new() };

    let (extensions, skipped_extensions) = match inputs.extensions {
        Some(source) if opts.extensions => extensions::collect(source),
        _ => (Vec::new(), Vec::new()),
    };

    let backup = Backup {
        manga,
        categories,
        sources,
        preferences,
        source_preferences,
        manga_extension_repos,
        is_legacy: false,
        anime,
        anime_categories,
        anime_sources,
        extensions,
        anime_extension_stores,
        custom_buttons,
    };
    Ok((backup, skipped_extensions))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use kuro_data::anime::Anime;
    use kuro_data::category::Category;
    use kuro_data::chapter::Chapter;
    use kuro_data::episode::Episode;
    use kuro_data::history::MangaHistory;
    use kuro_data::manga::Manga;
    use kuro_data::track::MangaTrack;

    use super::*;
    use crate::schema::{PrefValue, Backup};

    #[derive(Default)]
    struct Fake {
        manga_fav: Vec<MangaEntry>,
        manga_read: Vec<MangaEntry>,
        anime_fav: Vec<AnimeEntry>,
        anime_watched: Vec<AnimeEntry>,
        manga_cats: Vec<Category>,
        manga_read_calls: Cell<u32>,
        anime_watched_calls: Cell<u32>,
    }

    impl LibraryReader for Fake {
        fn manga_favorites(&self) -> Result<Vec<MangaEntry>, BackupError> {
            Ok(clone_manga(&self.manga_fav))
        }
        fn manga_read_not_in_library(&self) -> Result<Vec<MangaEntry>, BackupError> {
            self.manga_read_calls.set(self.manga_read_calls.get() + 1);
            Ok(clone_manga(&self.manga_read))
        }
        fn anime_favorites(&self) -> Result<Vec<AnimeEntry>, BackupError> {
            Ok(clone_anime(&self.anime_fav))
        }
        fn anime_watched_not_in_library(&self) -> Result<Vec<AnimeEntry>, BackupError> {
            self.anime_watched_calls.set(self.anime_watched_calls.get() + 1);
            Ok(clone_anime(&self.anime_watched))
        }
        fn manga_categories(&self) -> Result<Vec<Category>, BackupError> {
            Ok(self.manga_cats.clone())
        }
        fn anime_categories(&self) -> Result<Vec<Category>, BackupError> {
            Ok(Vec::new())
        }
        fn manga_extension_repos(&self) -> Result<Vec<kuro_data::extension_repo::ExtensionRepo>, BackupError> {
            Ok(Vec::new())
        }
    }

    // Entries are not Clone in kuro-data's wrappers, so the fake rebuilds them field by field.
    fn clone_manga(src: &[MangaEntry]) -> Vec<MangaEntry> {
        src.iter()
            .map(|e| MangaEntry {
                manga: e.manga.clone(),
                chapters: e.chapters.clone(),
                excluded_scanlators: e.excluded_scanlators.clone(),
                categories: e.categories.clone(),
                tracks: e.tracks.clone(),
                history: e.history.clone(),
            })
            .collect()
    }

    fn clone_anime(src: &[AnimeEntry]) -> Vec<AnimeEntry> {
        src.iter()
            .map(|e| AnimeEntry {
                anime: e.anime.clone(),
                episodes: e.episodes.clone(),
                categories: e.categories.clone(),
                tracks: e.tracks.clone(),
                history: e.history.clone(),
            })
            .collect()
    }

    struct NoSources;
    impl SourceRegistry for NoSources {
        fn is_installed(&self, _: i64) -> bool {
            false
        }
        fn name_of(&self, id: i64) -> String {
            format!("src{id}")
        }
        fn stub_label(&self, id: i64) -> String {
            id.to_string()
        }
    }

    struct NoPrefs;
    impl PrefStore for NoPrefs {
        fn all(&self) -> Vec<(String, PrefValue)> {
            Vec::new()
        }
    }
    impl SourcePrefStore for NoPrefs {
        fn configurable(&self, _: Kind) -> Vec<(String, Vec<(String, PrefValue)>)> {
            Vec::new()
        }
    }

    fn manga(id: i64, source: i64, favorite: bool) -> Manga {
        let mut m = Manga::create();
        m.id = id;
        m.source = source;
        m.url = format!("/m/{id}");
        m.title = format!("Title {id}");
        m.favorite = favorite;
        m
    }

    fn chapter(id: i64, manga_id: i64, url: &str, scanlator: Option<&str>) -> Chapter {
        let mut c = Chapter::create();
        c.id = id;
        c.manga_id = manga_id;
        c.url = url.into();
        c.name = format!("Ch {id}");
        c.scanlator = scanlator.map(str::to_owned);
        c.chapter_number = 1.5;
        c
    }

    fn sample_manga_entry() -> MangaEntry {
        MangaEntry {
            manga: manga(1, 7, true),
            chapters: vec![chapter(10, 1, "/c/10", Some("Excluded")), chapter(11, 1, "/c/11", Some("Ok"))],
            excluded_scanlators: ["Excluded".to_string()].into_iter().collect(),
            categories: vec![Category { id: 5, name: "Read".into(), order: 2, flags: 0, hidden: false }, Category { id: 0, name: "Default".into(), order: 0, flags: 0, hidden: false }],
            tracks: vec![MangaTrack { id: 1, manga_id: 1, tracker_id: 2, remote_id: 77, library_id: None, title: "t".into(), last_chapter_read: 3.0, total_chapters: 12, status: 1, score: 8.0, remote_url: "u".into(), start_date: 0, finish_date: 0, private: false }],
            history: vec![MangaHistory { id: 1, chapter_id: 11, last_read: 1000, time_read: 55 }, MangaHistory { id: 2, chapter_id: 999, last_read: 1, time_read: 1 }],
        }
    }

    fn sample_anime_entry() -> AnimeEntry {
        let mut a = Anime::create();
        a.id = 3;
        a.source = 8;
        a.url = "/a/3".into();
        a.favorite = true;
        a.background_url = Some("https://bg".into());
        let mut ep = Episode::create();
        ep.id = 30;
        ep.anime_id = 3;
        ep.url = "/e/30".into();
        ep.name = "E30".into();
        ep.episode_number = 2.0;
        AnimeEntry { anime: a, episodes: vec![ep], categories: Vec::new(), tracks: Vec::new(), history: Vec::new() }
    }

    fn build(fake: &Fake, opts: &BackupOptions) -> Result<Backup, BackupError> {
        let inputs = CreateInputs { library: fake, sources: &NoSources, prefs: &NoPrefs, source_prefs: &NoPrefs, extensions: None };
        build_backup(&inputs, opts)
    }

    #[test]
    fn category_link_is_order_and_system_excluded() {
        let fake = Fake { manga_fav: vec![sample_manga_entry()], ..Default::default() };
        let b = build(&fake, &BackupOptions::default()).unwrap();
        assert_eq!(b.manga[0].categories, vec![2]);
        assert!(!b.is_legacy);
    }

    #[test]
    fn excluded_scanlators_kept_when_chapters_off() {
        let fake = Fake { manga_fav: vec![sample_manga_entry()], ..Default::default() };
        let opts = BackupOptions { chapters: false, ..Default::default() };
        let b = build(&fake, &opts).unwrap();
        assert!(b.manga[0].chapters.is_empty());
        assert_eq!(b.manga[0].excluded_scanlators, vec!["Excluded".to_string()]);
        // Chapters are unfiltered when they are written (applyScanlatorFilter = 0).
        let all = build(&fake, &BackupOptions::default()).unwrap();
        assert_eq!(all.manga[0].chapters.len(), 2);
    }

    #[test]
    fn history_uses_urls_and_drops_orphans() {
        let fake = Fake { manga_fav: vec![sample_manga_entry()], ..Default::default() };
        let b = build(&fake, &BackupOptions::default()).unwrap();
        assert_eq!(b.manga[0].history.len(), 1);
        assert_eq!(b.manga[0].history[0].url, "/c/11");
        assert_eq!(b.manga[0].history[0].read_duration, 55);
    }

    #[test]
    fn tracking_writes_media_id_and_history_off_clears_it() {
        let fake = Fake { manga_fav: vec![sample_manga_entry()], ..Default::default() };
        let b = build(&fake, &BackupOptions::default()).unwrap();
        assert_eq!(b.manga[0].tracking[0].media_id, 77);
        assert_eq!(b.manga[0].tracking[0].media_id_int, 0);
        let off = build(&fake, &BackupOptions { tracking: false, history: false, ..Default::default() }).unwrap();
        assert!(off.manga[0].tracking.is_empty() && off.manga[0].history.is_empty());
    }

    #[test]
    fn library_off_skips_non_favourite_queries() {
        let fake = Fake { manga_fav: vec![sample_manga_entry()], manga_read: vec![sample_manga_entry()], ..Default::default() };
        let opts = BackupOptions { library_entries: false, read_entries: true, ..Default::default() };
        let b = build(&fake, &opts).unwrap();
        assert!(b.manga.is_empty() && b.anime.is_empty());
        assert_eq!(fake.manga_read_calls.get(), 0, "E-09: the non-favourite query does not run");
        assert_eq!(fake.anime_watched_calls.get(), 0);
    }

    #[test]
    fn read_entries_off_drops_non_favourites() {
        let fake = Fake { manga_fav: vec![sample_manga_entry()], manga_read: vec![sample_manga_entry()], ..Default::default() };
        let b = build(&fake, &BackupOptions { read_entries: false, ..Default::default() }).unwrap();
        assert_eq!(b.manga.len(), 1);
        assert_eq!(fake.manga_read_calls.get(), 0);
        let both = build(&fake, &BackupOptions::default()).unwrap();
        assert_eq!(both.manga.len(), 2);
        assert_eq!(fake.manga_read_calls.get(), 1);
    }

    #[test]
    fn source_list_derived_from_entries_first_seen() {
        let fake = Fake { manga_fav: vec![sample_manga_entry()], anime_fav: vec![sample_anime_entry()], ..Default::default() };
        let b = build(&fake, &BackupOptions::default()).unwrap();
        assert_eq!(b.sources.len(), 1);
        assert_eq!(b.sources[0].source_id, 7);
        assert_eq!(b.anime_sources[0].source_id, 8);
    }

    #[test]
    fn background_url_and_episode_written() {
        let fake = Fake { anime_fav: vec![sample_anime_entry()], ..Default::default() };
        let b = build(&fake, &BackupOptions::default()).unwrap();
        assert_eq!(b.anime[0].background_url.as_deref(), Some("https://bg"));
        assert_eq!(b.anime[0].episodes[0].url, "/e/30");
        assert_eq!(b.anime[0].episodes[0].episode_number, 2.0);
        assert_eq!(b.anime[0].id, Some(3));
        assert_eq!(b.anime[0].season_number, -1.0);
    }

    #[test]
    fn categories_option_gates_category_lists() {
        let fake = Fake { manga_cats: vec![Category { id: 4, name: "A".into(), order: 1, flags: 0, hidden: false }], ..Default::default() };
        let on = build(&fake, &BackupOptions::default()).unwrap();
        assert_eq!(on.categories.len(), 1);
        let off = build(&fake, &BackupOptions { categories: false, ..Default::default() }).unwrap();
        assert!(off.categories.is_empty());
    }

    #[test]
    fn overflow_fails_the_backup_and_names_the_row() {
        let mut e = sample_manga_entry();
        e.manga.status = 1i64 << 40;
        let fake = Fake { manga_fav: vec![e], ..Default::default() };
        let err = build(&fake, &BackupOptions::default()).unwrap_err();
        assert!(matches!(err, BackupError::Overflow { entry: "manga", id: 1, field: "status" }));
    }

    #[test]
    fn option_matrix_each_option_alone() {
        let fake = Fake { manga_fav: vec![sample_manga_entry()], anime_fav: vec![sample_anime_entry()], manga_cats: vec![Category { id: 4, name: "A".into(), order: 1, flags: 0, hidden: false }], ..Default::default() };
        let none = BackupOptions {
            library_entries: false, categories: false, chapters: false, tracking: false, history: false, read_entries: false,
            app_settings: false, extension_stores: false, custom_button: false, source_settings: false, private_settings: false, extensions: false,
        };
        // Library entries alone: entries and their sources, no categories, no settings.
        let lib = build(&fake, &BackupOptions { library_entries: true, ..none }).unwrap();
        assert!(!lib.manga.is_empty() && lib.categories.is_empty() && lib.manga[0].chapters.is_empty());
        // Categories alone: only the category lists.
        let cats = build(&fake, &BackupOptions { categories: true, ..none }).unwrap();
        assert!(cats.manga.is_empty() && cats.categories.len() == 1);
        // Chapters alone with library off: nothing (chapters live inside entries).
        let chapters = build(&fake, &BackupOptions { chapters: true, ..none }).unwrap();
        assert!(chapters.manga.is_empty() && chapters.anime.is_empty());
        // Extensions alone: the entries come from the artifact source, not from the library.
        let ext = build(&fake, &BackupOptions { extensions: true, ..none }).unwrap();
        assert!(ext.extensions.is_empty(), "no artifact source supplied");
    }

    struct Artifacts(Vec<(String, Option<Vec<u8>>)>);
    impl ExtensionArtifacts for Artifacts {
        fn installed(&self) -> Vec<(String, Option<Vec<u8>>)> {
            self.0.clone()
        }
    }

    fn build_with(fake: &Fake, arts: &Artifacts, opts: &BackupOptions) -> (Backup, Vec<String>) {
        let inputs = CreateInputs { library: fake, sources: &NoSources, prefs: &NoPrefs, source_prefs: &NoPrefs, extensions: Some(arts) };
        build_backup_report(&inputs, opts).unwrap()
    }

    #[test]
    fn extensions_opt_in() {
        let fake = Fake::default();
        let arts = Artifacts(vec![("anime.ext".into(), Some(vec![1, 2])), ("manga.ext".into(), Some(vec![3]))]);
        let (default, skipped) = build_with(&fake, &arts, &BackupOptions::default());
        assert!(default.extensions.is_empty() && skipped.is_empty(), "default options write no 504 entries");
        let (on, _) = build_with(&fake, &arts, &BackupOptions { extensions: true, ..BackupOptions::default() });
        let names: Vec<&str> = on.extensions.iter().map(|e| e.pkg_name.as_str()).collect();
        assert_eq!(names, vec!["anime.ext", "manga.ext"], "one entry per installed package, source order kept");
        assert_eq!(on.extensions[0].apk, vec![1, 2]);
    }

    #[test]
    fn artifact_missing_skipped() {
        let fake = Fake::default();
        let arts = Artifacts(vec![("a".into(), Some(vec![1])), ("gone".into(), None), ("b".into(), Some(vec![2]))]);
        let (backup, skipped) = build_with(&fake, &arts, &BackupOptions { extensions: true, ..BackupOptions::default() });
        let names: Vec<&str> = backup.extensions.iter().map(|e| e.pkg_name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
        assert_eq!(skipped, vec!["gone".to_string()]);
    }

    /// E-29: emoji, right-to-left text, combining marks and a 4096-character title survive create and decode.
    #[test]
    fn unicode_titles_roundtrip() {
        use crate::wire::Message;
        let long = "長".repeat(4096);
        let titles = ["🙂 Привет 🇯🇵", "مرحبا بالعالم", "e\u{301}a\u{300}\u{323}", long.as_str()];
        let entries: Vec<MangaEntry> = titles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut e = sample_manga_entry();
                e.manga.id = 100 + i as i64;
                e.manga.title = (*t).to_string();
                e
            })
            .collect();
        let fake = Fake { manga_fav: entries, ..Default::default() };
        let backup = build(&fake, &BackupOptions::default()).unwrap();
        let back = Backup::from_bytes(&backup.to_bytes().unwrap()).unwrap();
        let got: Vec<&str> = back.manga.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(got, titles.to_vec());
        assert_eq!(back.manga[3].title.chars().count(), 4096);
    }

    /// E-07: float32 fields lose precision; the stored value is the nearest float32 (documented, format is fixed).
    #[test]
    fn float_precision_documented() {
        use crate::wire::Message;
        let mut entry = sample_manga_entry();
        entry.chapters[1].chapter_number = 1234.5678;
        let fake = Fake { manga_fav: vec![entry], ..Default::default() };
        let backup = build(&fake, &BackupOptions::default()).unwrap();
        let back = Backup::from_bytes(&backup.to_bytes().unwrap()).unwrap();
        let stored = back.manga[0].chapters.iter().map(|c| c.chapter_number).find(|n| *n > 1000.0).unwrap();
        assert_eq!(stored, 1234.5678f32);
        assert_ne!(f64::from(stored), 1234.5678f64, "widening back does not recover the f64");
    }
}
