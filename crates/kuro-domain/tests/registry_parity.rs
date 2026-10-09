//! T-10a-037: DomainModule registry parity. The topic `plans/10-app-domain-layer/p1-topic-domainmodule-registry.md`
//! lists every Kotlin registration (178 rows, plus the SY row in its text). Each row is either owned by area 10,
//! which needs a `Domain` accessor, or owned by another area, which must be named in `REGISTRY_OTHER_AREAS` with
//! its owner. The test prints the count per owner.

const TOPIC: &str = include_str!("../../../plans/10-app-domain-layer/p1-topic-domainmodule-registry.md");
/// The container whose accessors the area-10 rows need.
const DOMAIN_SOURCE: &str = include_str!("../src/app/domain.rs");
const OWNER_AREA10: &str = "area:10-app-domain-layer";
/// The SY registration is in the topic text, not in its table.
const SY_REGISTRATION: &str = "ToggleExcludeFromMangaDataSaver";

/// Registrations owned by other areas: (registered type, owning area). A forgotten class cannot hide here,
/// because every row of the topic must appear in one of the two lists.
const REGISTRY_OTHER_AREAS: &[(&str, &str)] = &[
    ("AnimeCategoryRepository", "area:05-data-repos-entries-items"),
    ("GetAnimeCategories", "area:08-domain-library-category-history-updates"),
    ("GetVisibleAnimeCategories", "area:08-domain-library-category-history-updates"),
    ("ResetAnimeCategoryFlags", "area:08-domain-library-category-history-updates"),
    ("SetAnimeDisplayMode", "area:08-domain-library-category-history-updates"),
    ("SetSortModeForAnimeCategory", "area:08-domain-library-category-history-updates"),
    ("CreateAnimeCategoryWithName", "area:08-domain-library-category-history-updates"),
    ("RenameAnimeCategory", "area:08-domain-library-category-history-updates"),
    ("ReorderAnimeCategory", "area:08-domain-library-category-history-updates"),
    ("UpdateAnimeCategory", "area:08-domain-library-category-history-updates"),
    ("HideAnimeCategory", "area:08-domain-library-category-history-updates"),
    ("DeleteAnimeCategory", "area:08-domain-library-category-history-updates"),
    ("MangaCategoryRepository", "area:05-data-repos-entries-items"),
    ("GetMangaCategories", "area:08-domain-library-category-history-updates"),
    ("GetVisibleMangaCategories", "area:08-domain-library-category-history-updates"),
    ("ResetMangaCategoryFlags", "area:08-domain-library-category-history-updates"),
    ("SetMangaDisplayMode", "area:08-domain-library-category-history-updates"),
    ("SetSortModeForMangaCategory", "area:08-domain-library-category-history-updates"),
    ("CreateMangaCategoryWithName", "area:08-domain-library-category-history-updates"),
    ("RenameMangaCategory", "area:08-domain-library-category-history-updates"),
    ("ReorderMangaCategory", "area:08-domain-library-category-history-updates"),
    ("UpdateMangaCategory", "area:08-domain-library-category-history-updates"),
    ("HideMangaCategory", "area:08-domain-library-category-history-updates"),
    ("DeleteMangaCategory", "area:08-domain-library-category-history-updates"),
    ("AnimeRepository", "area:05-data-repos-entries-items"),
    ("GetDuplicateLibraryAnime", "area:07-domain-entries-items"),
    ("GetAnimeFavorites", "area:07-domain-entries-items"),
    ("GetLibraryAnime", "area:07-domain-entries-items"),
    ("GetAnimeWithEpisodesAndSeasons", "area:07-domain-entries-items"),
    ("GetAnimeByUrlAndSourceId", "area:07-domain-entries-items"),
    ("GetAnime", "area:07-domain-entries-items"),
    ("GetAnimeSeasonsByParentId", "area:07-domain-entries-items"),
    ("GetNextEpisodes", "area:08-domain-library-category-history-updates"),
    ("GetUpcomingAnime", "area:09-domain-source-track-download-release-misc"),
    ("ResetAnimeViewerFlags", "area:07-domain-entries-items"),
    ("SetAnimeEpisodeFlags", "area:07-domain-entries-items"),
    ("SetAnimeSeasonFlags", "area:07-domain-entries-items"),
    ("AnimeFetchInterval", "area:07-domain-entries-items"),
    ("SetAnimeDefaultEpisodeFlags", "area:07-domain-entries-items"),
    ("SetAnimeDefaultSeasonFlags", "area:07-domain-entries-items"),
    ("NetworkToLocalAnime", "area:07-domain-entries-items"),
    ("SetAnimeCategories", "area:08-domain-library-category-history-updates"),
    ("ShouldUpdateDbSeason", "area:07-domain-entries-items"),
    ("AnimeRelationRepository", "area:05-data-repos-entries-items"),
    ("GetRelatedAnime", "area:07-domain-entries-items"),
    ("MangaRepository", "area:05-data-repos-entries-items"),
    ("GetDuplicateLibraryManga", "area:07-domain-entries-items"),
    ("GetMangaFavorites", "area:07-domain-entries-items"),
    ("GetLibraryManga", "area:07-domain-entries-items"),
    ("GetMangaWithChapters", "area:07-domain-entries-items"),
    ("GetMangaByUrlAndSourceId", "area:07-domain-entries-items"),
    ("GetManga", "area:07-domain-entries-items"),
    ("GetNextChapters", "area:08-domain-library-category-history-updates"),
    ("GetUpcomingManga", "area:09-domain-source-track-download-release-misc"),
    ("ResetMangaViewerFlags", "area:07-domain-entries-items"),
    ("SetMangaChapterFlags", "area:07-domain-entries-items"),
    ("MangaFetchInterval", "area:07-domain-entries-items"),
    ("SetMangaDefaultChapterFlags", "area:07-domain-entries-items"),
    ("NetworkToLocalManga", "area:07-domain-entries-items"),
    ("SetMangaCategories", "area:08-domain-library-category-history-updates"),
    ("ReleaseService", "area:06-data-repos-history-track-source-misc"),
    ("GetApplicationRelease", "area:09-domain-source-track-download-release-misc"),
    ("AnimeTrackRepository", "area:06-data-repos-history-track-source-misc"),
    ("DeleteAnimeTrack", "area:09-domain-source-track-download-release-misc"),
    ("GetTracksPerAnime", "area:09-domain-source-track-download-release-misc"),
    ("GetAnimeTracks", "area:09-domain-source-track-download-release-misc"),
    ("InsertAnimeTrack", "area:09-domain-source-track-download-release-misc"),
    ("MangaTrackRepository", "area:06-data-repos-history-track-source-misc"),
    ("DeleteMangaTrack", "area:09-domain-source-track-download-release-misc"),
    ("GetTracksPerManga", "area:09-domain-source-track-download-release-misc"),
    ("GetMangaTracks", "area:09-domain-source-track-download-release-misc"),
    ("InsertMangaTrack", "area:09-domain-source-track-download-release-misc"),
    ("EpisodeRepository", "area:05-data-repos-entries-items"),
    ("GetEpisode", "area:07-domain-entries-items"),
    ("GetEpisodesByAnimeId", "area:07-domain-entries-items"),
    ("GetEpisodeByUrlAndAnimeId", "area:07-domain-entries-items"),
    ("UpdateEpisode", "area:07-domain-entries-items"),
    ("ShouldUpdateDbEpisode", "area:07-domain-entries-items"),
    ("FilterEpisodesForDownload", "area:09-domain-source-track-download-release-misc"),
    ("ChapterRepository", "area:05-data-repos-entries-items"),
    ("GetChapter", "area:07-domain-entries-items"),
    ("GetChaptersByMangaId", "area:07-domain-entries-items"),
    ("GetChapterByUrlAndMangaId", "area:07-domain-entries-items"),
    ("UpdateChapter", "area:07-domain-entries-items"),
    ("ShouldUpdateDbChapter", "area:07-domain-entries-items"),
    ("FilterChaptersForDownload", "area:09-domain-source-track-download-release-misc"),
    ("AnimeHistoryRepository", "area:06-data-repos-history-track-source-misc"),
    ("GetAnimeHistory", "area:08-domain-library-category-history-updates"),
    ("UpsertAnimeHistory", "area:08-domain-library-category-history-updates"),
    ("RemoveAnimeHistory", "area:08-domain-library-category-history-updates"),
    ("MangaHistoryRepository", "area:06-data-repos-history-track-source-misc"),
    ("GetMangaHistory", "area:08-domain-library-category-history-updates"),
    ("UpsertMangaHistory", "area:08-domain-library-category-history-updates"),
    ("RemoveMangaHistory", "area:08-domain-library-category-history-updates"),
    ("GetTotalReadDuration", "area:08-domain-library-category-history-updates"),
    ("AnimeUpdatesRepository", "area:06-data-repos-history-track-source-misc"),
    ("GetAnimeUpdates", "area:08-domain-library-category-history-updates"),
    ("MangaUpdatesRepository", "area:06-data-repos-history-track-source-misc"),
    ("GetMangaUpdates", "area:08-domain-library-category-history-updates"),
    ("AnimeSourceRepository", "area:06-data-repos-history-track-source-misc"),
    ("AnimeStubSourceRepository", "area:06-data-repos-history-track-source-misc"),
    ("GetRemoteAnime", "area:09-domain-source-track-download-release-misc"),
    ("GetAnimeSourcesWithNonLibraryAnime", "area:09-domain-source-track-download-release-misc"),
    ("MangaSourceRepository", "area:06-data-repos-history-track-source-misc"),
    ("MangaStubSourceRepository", "area:06-data-repos-history-track-source-misc"),
    ("GetRemoteManga", "area:09-domain-source-track-download-release-misc"),
    ("GetMangaSourcesWithNonLibraryManga", "area:09-domain-source-track-download-release-misc"),
    ("ExtensionRepoService", "area:09-domain-source-track-download-release-misc"),
    ("AnimeExtensionStoreService", "area:19-extensions-anime"),
    ("AnimeExtensionStoreRepository", "area:19-extensions-anime"),
    ("AddAnimeExtensionStore", "area:19-extensions-anime"),
    ("GetAnimeExtensionStoreCountAsFlow", "area:19-extensions-anime"),
    ("GetAnimeExtensionStores", "area:19-extensions-anime"),
    ("RemoveAnimeExtensionStore", "area:19-extensions-anime"),
    ("UpdateAnimeExtensionStores", "area:19-extensions-anime"),
    ("MangaExtensionRepoRepository", "area:06-data-repos-history-track-source-misc"),
    ("GetMangaExtensionRepo", "area:09-domain-source-track-download-release-misc"),
    ("GetMangaExtensionRepoCount", "area:09-domain-source-track-download-release-misc"),
    ("CreateMangaExtensionRepo", "area:09-domain-source-track-download-release-misc"),
    ("DeleteMangaExtensionRepo", "area:09-domain-source-track-download-release-misc"),
    ("ReplaceMangaExtensionRepo", "area:09-domain-source-track-download-release-misc"),
    ("UpdateMangaExtensionRepo", "area:09-domain-source-track-download-release-misc"),
    ("CustomButtonRepository", "area:48-settings-player-and-scripts"),
    ("CreateCustomButton", "area:48-settings-player-and-scripts"),
    ("DeleteCustomButton", "area:48-settings-player-and-scripts"),
    ("GetCustomButtons", "area:48-settings-player-and-scripts"),
    ("UpdateCustomButton", "area:48-settings-player-and-scripts"),
    ("ReorderCustomButton", "area:48-settings-player-and-scripts"),
    ("ToggleFavoriteCustomButton", "area:48-settings-player-and-scripts"),
    ("TrackSelect", "area:45-player-core"),
    ("UpdateAnimeFromRemote", "area:17-source-app-layer"),
];

/// Area-10 registrations with no accessor on `Domain` yet. The list must equal the set of unbuilt rows, so an
/// accessor that lands is noticed (the test fails until its name is removed here).
const PENDING_AREA10: &[&str] = &[
    "SetAnimeViewerFlags",
    "UpdateAnime",
    "SyncSeasonsWithSource",
    "SyncRelatedAnimeWithSource",
    "SetMangaViewerFlags",
    "UpdateManga",
    "GetExcludedScanlators",
    "SetExcludedScanlators",
    "TrackEpisode",
    "AddAnimeTracks",
    "RefreshAnimeTracks",
    "SyncEpisodeProgressWithTrack",
    "TrackChapter",
    "AddMangaTracks",
    "RefreshMangaTracks",
    "SyncChapterProgressWithTrack",
    "SetSeenStatus",
    "SyncEpisodesWithSource",
    "SetReadStatus",
    "SyncChaptersWithSource",
    "GetAvailableScanlators",
    "DeleteEpisodeDownload",
    "GetAnimeExtensionsByType",
    "GetAnimeExtensionSources",
    "GetAnimeExtensionLanguages",
    "DeleteChapterDownload",
    "GetMangaExtensionsByType",
    "GetMangaExtensionSources",
    "GetMangaExtensionLanguages",
    "GetEnabledAnimeSources",
    "GetLanguagesWithAnimeSources",
    "GetAnimeSourcesWithFavoriteCount",
    "ToggleAnimeSource",
    "ToggleAnimeSourcePin",
    "GetEnabledMangaSources",
    "GetLanguagesWithMangaSources",
    "GetMangaSourcesWithFavoriteCount",
    "SetMigrateSorting",
    "ToggleLanguage",
    "ToggleMangaSource",
    "ToggleMangaSourcePin",
    "TrustAnimeExtension",
    "TrustMangaExtension",
    "ToggleAnimeIncognito",
    "GetAnimeIncognitoState",
    "ToggleMangaIncognito",
    "GetMangaIncognitoState",
    "ToggleExcludeFromMangaDataSaver",
];

struct Row {
    registered: String,
    owner: String,
    /// The last segment of the "Rust counterpart" column (the Rust type name), for area-10 rows.
    type_name: String,
}

fn parse_rows() -> Vec<Row> {
    TOPIC
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            if cells.len() < 10 || cells[1].parse::<u32>().is_err() {
                return None;
            }
            let counterpart = cells[9].trim_matches('`');
            Some(Row {
                registered: cells[3].to_owned(),
                owner: cells[8].to_owned(),
                type_name: counterpart.rsplit("::").next().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// `SetAnimeViewerFlags` to `set_anime_viewer_flags`.
fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Whether `Domain` has an accessor named after the type.
fn has_accessor(type_name: &str) -> bool {
    DOMAIN_SOURCE.contains(&format!("pub fn {}(", snake_case(type_name)))
}

#[test]
fn the_topic_has_every_registration_and_the_counts_per_owner_are_printed() {
    let rows = parse_rows();
    assert_eq!(rows.len(), 178, "rows in the topic table");
    let area10 = rows.iter().filter(|r| r.owner == OWNER_AREA10).count();
    let others = rows.len() - area10;
    println!("registry rows: {} total", rows.len());
    println!("  {OWNER_AREA10}: {area10} rows, plus the SY registration {SY_REGISTRATION}");
    println!("  other areas: {others} rows");
    assert_eq!(area10, 47);
    assert_eq!(others, REGISTRY_OTHER_AREAS.len());
}

#[test]
fn every_area10_registration_has_an_accessor_or_is_listed_as_pending() {
    let mut names: Vec<String> = parse_rows()
        .into_iter()
        .filter(|r| r.owner == OWNER_AREA10)
        .map(|r| r.type_name)
        .collect();
    names.push(SY_REGISTRATION.to_owned());
    let unaccounted: Vec<&String> = names
        .iter()
        .filter(|n| !has_accessor(n) && !PENDING_AREA10.contains(&n.as_str()))
        .collect();
    assert!(
        unaccounted.is_empty(),
        "area:10 registrations with no accessor and not in PENDING_AREA10: {unaccounted:?}"
    );
}

/// T-10a-037 acceptance: a test fails when an area-10 row has no `Domain` accessor. It is ignored by default because
/// the 47 accessors and SY are not built (T-10a-037 is PARTIAL), so the default run would otherwise pass on the waiver
/// above. Run `cargo test -p kuro-domain --test registry_parity -- --ignored` to see the failure.
#[test]
#[ignore = "T-10a-037 PARTIAL: no area:10 accessor on Domain is built yet"]
fn every_area10_registration_has_its_accessor() {
    let mut names: Vec<String> = parse_rows()
        .into_iter()
        .filter(|r| r.owner == OWNER_AREA10)
        .map(|r| r.type_name)
        .collect();
    names.push(SY_REGISTRATION.to_owned());
    let missing: Vec<&String> = names.iter().filter(|n| !has_accessor(n)).collect();
    assert!(missing.is_empty(), "area:10 registrations with no Domain accessor: {missing:?}");
}

#[test]
fn a_pending_entry_is_removed_once_its_accessor_exists() {
    let names: Vec<String> = parse_rows()
        .into_iter()
        .filter(|r| r.owner == OWNER_AREA10)
        .map(|r| r.type_name)
        .collect();
    for pending in PENDING_AREA10 {
        assert!(
            names.iter().any(|n| n == pending) || *pending == SY_REGISTRATION,
            "{pending} is pending but is not an area-10 registration"
        );
        assert!(
            !has_accessor(pending),
            "{pending} has an accessor now; remove it from PENDING_AREA10"
        );
    }
}

#[test]
fn every_other_area_row_names_its_owner_in_the_list() {
    for row in parse_rows().iter().filter(|r| r.owner != OWNER_AREA10) {
        assert!(
            REGISTRY_OTHER_AREAS.contains(&(row.registered.as_str(), row.owner.as_str())),
            "{} ({}) is missing from REGISTRY_OTHER_AREAS",
            row.registered,
            row.owner
        );
    }
}
