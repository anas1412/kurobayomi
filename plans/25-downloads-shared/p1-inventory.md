# 25-downloads-shared — Part 1 inventory

Scope command: `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 25-downloads-shared 1`
Source root: `/home/blackbox/aniyomi-src/` (all paths repo-relative).
Scope totals: 4 files, 591 LOC.

Read status: every scope file was read in full (`cat -n`). Callers and callees outside the scope were read where a scope file's behaviour depends on them (listed under "Callees read" below). Callees are owned by other areas and are only summarised here.

## Scope files

| Path | LOC | Declared names (complete) | Purpose | Port decision | Target Rust module | Priority | Task ids |
|---|---|---|---|---|---|---|---|
| app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt | 19 | `class DeleteEpisodeDownload(sourceManager: AnimeSourceManager, downloadManager: AnimeDownloadManager)`; `suspend fun awaitAll(anime: Anime, vararg episodes: Episode)` | Domain interactor: deletes downloaded files of episodes, only when the anime's source is resolvable. | Rewrite (small). The Kotlin is a 2-line guard around a manager call. | `kurobayomi::download::delete` (generic `DeleteDownloads<Kind>`) | P1 | T-25a-010, T-25a-013, T-25a-015, T-25a-016 |
| app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt | 19 | `class DeleteChapterDownload(sourceManager: MangaSourceManager, downloadManager: MangaDownloadManager)`; `suspend fun awaitAll(manga: Manga, vararg chapters: Chapter)` | Same as above for manga chapters. | Rewrite (small), shares the generic implementation with the anime one. | `kurobayomi::download::delete` | P1 | T-25a-011, T-25a-013, T-25a-015, T-25a-016 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt | 461 | `data object DownloadsTab : Tab`; members `options` (get, composable), `Content()`, `private fun AnimeActions(animeScreenModel, animeDownloadList)`, `private fun MangaActions(mangaScreenModel, mangaDownloadList)`. No top-level functions, no other classes. | The "Download queue" screen: top app bar with anime-count pill, two-page pager (Anime, Manga), tab row with badges, extended FAB (start/pause), sort and "Cancel all" actions, FAB auto-collapse on scroll. | Rewrite as a GTK4 page. Behaviour ported exactly (see p1-behavior.md section 4). Android-only pieces (AnimatedVectorDrawable tab icon, NestedScrollConnection, HorizontalPager, Voyager Tab/ScreenModel) replaced by GTK equivalents (see p1-rust-design.md section 9). | `kurobayomi::download::tab` (page), `kurobayomi::download::queue` (model), `kurobayomi::download::row` (row) | P1 (page and model), P2 (drag, polish), P3 (animation drop) | T-25a-020 .. T-25a-034, T-25a-037 .. T-25a-040 |
| domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt | 92 | `class DownloadPreferences(preferenceStore: PreferenceStore)`; 23 accessor functions: `downloadOnlyOverWifi`, `useExternalDownloader`, `externalDownloaderSelection`, `saveChaptersAsCBZ`, `splitTallImages`, `autoDownloadWhileReading`, `autoDownloadWhileWatching`, `removeAfterReadSlots`, `removeAfterMarkedAsRead`, `removeBookmarkedChapters`, `downloadFillermarkedItems`, `removeExcludeCategories`, `removeExcludeAnimeCategories`, `downloadNewChapters`, `downloadNewEpisodes`, `downloadNewChapterCategories`, `downloadNewEpisodeCategories`, `downloadNewChapterCategoriesExclude`, `downloadNewEpisodeCategoriesExclude`, `numberOfDownloads`, `downloadSpeedLimit`, `downloadNewUnreadChaptersOnly`, `downloadNewUnseenEpisodesOnly`. `companion object` with 6 `private const val` keys (`REMOVE_EXCLUDE_MANGA_CATEGORIES_PREF_KEY`, `REMOVE_EXCLUDE_ANIME_CATEGORIES_PREF_KEY`, `DOWNLOAD_NEW_MANGA_CATEGORIES_PREF_KEY`, `DOWNLOAD_NEW_ANIME_CATEGORIES_PREF_KEY`, `DOWNLOAD_NEW_MANGA_CATEGORIES_EXCLUDE_PREF_KEY`, `DOWNLOAD_NEW_ANIME_CATEGORIES_EXCLUDE_PREF_KEY`) and `val categoryPreferenceKeys: Set<String>`. | Typed access to all download preference keys with defaults. | Rewrite as a typed struct over the preference store (area:02-preferences-system). | `kurobayomi::download::prefs` | P0 (T-25a-001 typed store), P1 (T-25a-002 .. 006) | T-25a-001 .. T-25a-006 |

Count of top-level declarations across scope: 4 classes/objects (`DeleteEpisodeDownload`, `DeleteChapterDownload`, `DownloadsTab`, `DownloadPreferences`), 0 interfaces, 0 enums, 0 typealiases, 0 top-level functions, 0 composables at file top level. Composables inside `DownloadsTab` are members, not top-level.

## Callees read (owned by other areas; summarised, not re-specified here)

| Path | Used by | Area owner (proposal) |
|---|---|---|
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt | DeleteEpisodeDownload, DownloadsTab (via queue model) | area:24-downloads-anime |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt | DeleteChapterDownload, DownloadsTab | area:23-downloads-manga |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt | removeFromQueue, clearQueue, queue state | area:24-downloads-anime |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt | removeFromQueue, clearQueue, speed limit (line 525) | area:23-downloads-manga |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt | the Anime pager page of DownloadsTab | this area (queue model, see p1-topic-queue-model.md) |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt | the Manga pager page of DownloadsTab | this area (queue model) |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueTab.kt, .../manga/MangaDownloadQueueTab.kt | pager page wrappers (`animeDownloadTab`, `mangaDownloadTab`) | this area |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreen.kt, .../manga/MangaDownloadQueueScreen.kt | list body, EmptyScreen | this area (list body in T-25a-025) |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt, .../manga/MangaDownloadHolder.kt | row UI, progress bar, popup menu | this area (T-25a-026) |
| app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt | calls DeleteChapterDownload.awaitAll when `removeAfterMarkedAsRead` | area:07-domain-entries-items (consumer contract in T-25a-014) |
| app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt | calls DeleteEpisodeDownload.awaitAll when `removeAfterMarkedAsRead` | area:07-domain-entries-items (consumer contract in T-25a-014) |
| app/src/main/java/eu/kanade/domain/DomainModule.kt | DI registration lines 339 and 351 | area:10-app-domain-layer |
| app/src/main/res/menu/download_single.xml | item popup menu resource | this area |
| app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt | settings UI for all DownloadPreferences keys | area:47-settings-screens-general |
| presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Tabs.kt | `TabText` (badge pill) | area:49-theme-and-design-system |
| i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml | all user-visible strings | area:51-i18n |

## Kurobayomi mockup touched by this area

| Path | Relevance |
|---|---|
| /home/blackbox/kurobayomi/src/screens.rs `fn downloads()` (line 354) | Current mockup of the download queue page. Replaced by T-25a-033. |
| /home/blackbox/kurobayomi/src/screens.rs `fn download_button()` (line 134) | Mockup per-entry download menu; behaviour spec in p1-behavior.md section 6. |
| /home/blackbox/kurobayomi/spec/app.json, screen id `download_queue` (line 732) | Spec for the queue screen; updated by T-25a-033. |
| /home/blackbox/kurobayomi/spec/settings.json, screen id `settings_downloads` | Owned by area:47; referenced only. |

## Port-decision legend

- Port: behaviour copied with the same algorithm and constants.
- Rewrite: same behaviour, new implementation in Rust/GTK.
- Replace-with-crate / Replace-with-native: a Linux/GTK equivalent is used (named in p1-rust-design.md).
- Drop: not ported (none in this scope; the only Android-only items are replaced, see p1-rust-design.md section 9).

## Coverage check

Every path printed by the scope command appears verbatim in the table above (self-check run at the end of the authoring pass).
