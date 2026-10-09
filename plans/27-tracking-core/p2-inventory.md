# area:27-tracking-core, part 2 (track UI, login activity, entry track items) — inventory

Repo root for every path below: `/home/blackbox/aniyomi-src` (repo-relative paths, Apache-2.0 source, read-only).
Scope command: `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 27-tracking-core 2` prints 6 files, 1 879 LOC in total.
Part 1 (tracker framework, models, interactors, presentation selectors, enhanced/deletable interfaces) is documented by the part-1 author in `p1-*.md` of this same folder. Part 2 cites part-1 symbols by name; it does not redefine them.

Every file below was read in full. `AnimeTrackInfoDialog.kt` was read line by line (lines 1-874). `MangaTrackInfoDialog.kt` was checked with a full `diff` against the anime file: all 874 lines are either identical (same line number) or listed in the diff. The 3 differing import/alias areas and all `anime`→`manga` renames are in the behavior doc.

## Summary table

| # | Repo-relative path | LOC | Declared names (top level and nested, all of them) | Purpose | Port decision (reason) | Target Rust module / crate | Priority | Task ids |
|---|---|---|---|---|---|---|---|---|
| 1 | app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt | 874 | Top-level: `AnimeTrackInfoDialogHomeScreen` (public data class), `TrackStatusSelectorScreen` (private data class), `TrackEpisodeSelectorScreen` (private), `TrackScoreSelectorScreen` (private), `TrackDateSelectorScreen` (private), `TrackDateRemoverScreen` (private), `TrackServiceSearchScreen` (public data class), `TrackerAnimeRemoveScreen` (private). Nested in each: `private class Model` and `data class State` (AnimeTrackInfoDialogHomeScreen.Model.State; TrackStatusSelectorScreen.Model.State; TrackEpisodeSelectorScreen.Model.State; TrackScoreSelectorScreen.Model.State; TrackServiceSearchScreen.Model.State). `TrackDateSelectorScreen.Model` and `TrackDateRemoverScreen.Model` and `TrackerAnimeRemoveScreen.Model` are `ScreenModel` classes without State. Private extension functions on `Context`: `copyTrackerLink`; private method `openTrackerInBrowser`. Import alias: `tachiyomi.domain.track.anime.model.AnimeTrack as DbAnimeTrack` (same domain class, not the legacy DB class). | Anime track info dialog family: home list of tracker rows and every per-tracker sub-dialog (status, episode number, score, start/finish date, date removal, remote search, remove tracking). Each sub-dialog is a Voyager Screen with its own ScreenModel. | Rewrite (UI is Compose/Voyager; logic must be reimplemented as GTK dialogs with a Rust state model; the anime and manga copies are duplicated in Kotlin and must be ONE generic Rust implementation keyed by `EntryKind`). | `src/track/` GUI module (`home.rs`, `status.rs`, `progress.rs`, `score.rs`, `dates.rs`, `search.rs`, `remove.rs`), state in `crates/kuro-track/src/item.rs`, `progress.rs`, `dates.rs` | P0 (home, status, progress, score, search, remove); P1 (dates, enhanced match, open/copy/private) | T-27b-001 .. T-27b-018, T-27b-020, T-27b-030 .. T-27b-032, T-27b-034 .. T-27b-037, T-27b-039, T-27b-041 |
| 2 | app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt | 874 | Identical structure to #1 with renamed types: `MangaTrackInfoDialogHomeScreen` (public), `TrackStatusSelectorScreen` (private), `TrackChapterSelectorScreen` (private), `TrackScoreSelectorScreen` (private), `TrackDateSelectorScreen` (private), `TrackDateRemoverScreen` (private), `TrackServiceSearchScreen` (public), `TrackerMangaRemoveScreen` (private). Nested `Model`/`State` as in #1. Private extension `copyTrackerLink`, private `openTrackerInBrowser`. Import alias `tachiyomi.domain.track.manga.model.MangaTrack as DbMangaTrack`. | Manga track info dialog family: same screens as #1 with chapter progress instead of episode progress. | Rewrite (same reasons as #1). Merged into one generic Rust implementation with #1; do not port twice. | same as #1 (`EntryKind::Manga`) | P0 / P1 as #1 | T-27b-001 .. T-27b-018, T-27b-020, T-27b-030 .. T-27b-032, T-27b-034 .. T-27b-037, T-27b-039, T-27b-041 |
| 3 | app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackItem.kt | 6 | `data class AnimeTrackItem(val track: AnimeTrack?, val tracker: Tracker)` (one class, no other declarations). `AnimeTrack` here is the domain class `tachiyomi.domain.track.anime.model.AnimeTrack`. | Pairing of one logged-in anime tracker with its (optional) local track row. Rows with `track == null` are "tracker not bound to this entry" rows. | Rewrite as a plain Rust struct `TrackItem { tracker: TrackerId, track: Option<TrackRecord> }` (shared with manga). Port decision is trivial; it becomes a generic `TrackItem` with kind carried by `EntryKind`. | `crates/kuro-track/src/item.rs` | P0 | T-27b-001, T-27b-002, T-27b-003 |
| 4 | app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackItem.kt | 6 | `data class MangaTrackItem(val track: MangaTrack?, val tracker: Tracker)` (one class). `MangaTrack` = `tachiyomi.domain.track.manga.model.MangaTrack`. | Manga counterpart of #3. | Same as #3 (merged into one `TrackItem`). | `crates/kuro-track/src/item.rs` | P0 | T-27b-001, T-27b-002, T-27b-003 |
| 5 | app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt | 37 | `abstract class BaseOAuthLoginActivity : BaseActivity` with `internal val trackerManager: TrackerManager` (injectLazy), `abstract fun handleResult(data: Uri?)`, overrides `onCreate(savedInstanceState: Bundle?)` (sets `LoadingScreen` compose content, then calls `handleResult(intent.data)`), `internal fun returnToSettings()` (finish + start `MainActivity` with `FLAG_ACTIVITY_CLEAR_TOP or FLAG_ACTIVITY_SINGLE_TOP`). | Android Activity that receives the `aniyomi://<tracker>-auth` browser redirect and finishes the OAuth step. | Replace-with-platform: the Activity is an Android intent-filter target. Linux: a URI-scheme handler (`x-scheme-handler/aniyomi`) delivered to the single running GTK `Application` (`open` signal) and routed by a pure Rust parser. The Activity lifecycle, `LoadingScreen`, `finish()` and the `MainActivity` relaunch have no Linux equivalent and are dropped (the main window is simply focused). | `kuro-platform` (URI handler trait `UriSchemeHandler`, `SingleInstance`) + `crates/kuro-track/src/oauth.rs` + `src/track/login.rs` | P0 | T-27b-019, T-27b-020, T-27b-023, T-27b-033 |
| 6 | app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt | 84 | `class TrackLoginActivity : BaseOAuthLoginActivity` with `override fun handleResult(data: Uri?)` and private functions `handleAnilist(data: Uri)`, `handleBangumi(data: Uri)`, `handleMyAnimeList(data: Uri)`, `handleShikimori(data: Uri)`, `handleSimkl(data: Uri?)`. Host switch: `"anilist-auth"`, `"bangumi-auth"`, `"myanimelist-auth"`, `"shikimori-auth"`, `"simkl-auth"`. | Concrete callback router: per host, extracts `access_token` from the URL fragment (AniList implicit flow) or `code` from the query (the four authorization-code trackers), then logs the tracker in or logs it out. | Rewrite: the routing table and the login/logout decision are the behavior to keep (see behavior doc sections 13.x). Implementation moves to pure Rust parsing plus `Tracker::login`/`logout` calls from part 1. | `crates/kuro-track/src/oauth.rs` (parser, routing table, `CallbackOutcome` enum), `src/track/login.rs` (UI feedback) | P0 | T-27b-019, T-27b-021 .. T-27b-029, T-27b-032, T-27b-038 |

## Symbols this part cites from part 1 (owned by part 1 p1-inventory.md, listed here only so the references resolve)

- `eu.kanade.tachiyomi.data.track.Tracker` (interface, `Tracker.kt`), `BaseTracker`, `AnimeTracker`, `MangaTracker`, `EnhancedAnimeTracker`, `EnhancedMangaTracker`, `DeletableAnimeTracker`, `DeletableMangaTracker`, `TrackerManager` (`TrackerManager.kt`), `model.AnimeTrackSearch`, `model.MangaTrackSearch`.
- `eu.kanade.domain.track.anime.model.toDbTrack()` / `toDomainTrack()` and the manga pair, `copyPersonalFrom` (`AnimeTrack.kt` in domain track model folder).
- `eu.kanade.domain.track.anime.interactor.RefreshAnimeTracks` / manga pair; `tachiyomi.domain.track.anime.interactor.GetAnimeTracks`, `DeleteAnimeTrack` (domain interactors, read for this part).
- `eu.kanade.presentation.track.TrackInfoDialogSelector.kt` (`TrackStatusSelector`, `TrackItemSelector`, `TrackScoreSelector`, `TrackDateSelector`, `BaseSelector`), `eu.kanade.presentation.track.anime.AnimeTrackInfoDialogHome`, `eu.kanade.presentation.track.anime.AnimeTrackerSearch`, and the manga pairs. Read by part 2 only to confirm how the dialogs are driven (gating of rows, selector confirm semantics).
- `eu.kanade.domain.track.service.TrackPreferences` (private keys used by login, see data doc).
- `eu.kanade.tachiyomi.util.PkceUtil` (`app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt`), used by the MAL login URL.

## Row-by-row detail

### 1. AnimeTrackInfoDialog.kt (874 LOC)
- Lines 92-285 `AnimeTrackInfoDialogHomeScreen` (Content 97-182, `openTrackerInBrowser` 187-192, `copyTrackerLink` 194-199, `Model` 201-284).
- Lines 287-340 `TrackStatusSelectorScreen`.
- Lines 342-403 `TrackEpisodeSelectorScreen`.
- Lines 405-457 `TrackScoreSelectorScreen`.
- Lines 459-589 `TrackDateSelectorScreen` (anonymous `SelectableDates` object 465-520).
- Lines 591-677 `TrackDateRemoverScreen`.
- Lines 679-770 `TrackServiceSearchScreen`.
- Lines 772-874 `TrackerAnimeRemoveScreen`.

### 2. MangaTrackInfoDialog.kt (874 LOC)
Same line numbers as #1 for every shared construct (the diff produced no line shifts). Differences are listed in behavior doc section 17.

### 3-4. AnimeTrackItem.kt / MangaTrackItem.kt (6 LOC each)
Single data class each, no imports beyond `Tracker` and the domain track class.

### 5. BaseOAuthLoginActivity.kt (37 LOC)
Lines 25-49. Imports: `android.content.Intent`, `android.net.Uri`, `android.os.Bundle`, `TrackerManager`, `BaseActivity`, `MainActivity`, `setComposeContent`, `LoadingScreen`, `injectLazy`.

### 6. TrackLoginActivity.kt (84 LOC)
Lines 56-133. Host switch 58-66; `handleAnilist` 68-80; `handleBangumi` 82-93; `handleMyAnimeList` 95-106; `handleShikimori` 108-119; `handleSimkl` 121-132.

## Files with no tests in scope
No `*Test.kt` file exists under any of the six paths. Test cases to port or write are in `p2-edge-cases-and-tests.md`.
