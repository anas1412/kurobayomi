# Area 47b — Topic: search highlight jump and download-queue state

Two small subsystems that cross threads and timers. Source behaviour: `p1-behavior.md §C` (highlight) and `§M.3` (queue). Design: `p1-rust-design.md §6` and `§8.3`. Tests: `p1-edge-cases-and-tests.md` (T-E01 to T-E07, T-E13, T-E14, T-E53 to T-E57, T-R03, T-R04, T-R08).

## Part A. Search highlight jump

### A.1 Participants
- Writers of the request: the settings search screen (area:47, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSearchScreen.kt`) and the onboarding restore path (`app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt:45-49`).
- Holder: `SearchableSettings.highlightKey` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt:37-42`), a global `var String?`, documented as a hack.
- Reader: `PreferenceScreen` (`app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt:32-41`).
- Visual: `Modifier.highlightBackground` (`app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt:89-120`).

### A.2 Sequence (Kotlin, OBSERVED)
```
t=0      writer sets highlightKey = K, then navigates to the destination page
t=0      PreferenceScreen composes: highlightKey captured = K
t=0      LaunchedEffect: i = indexOfFirst(title == K) over the flattened list (A.4 in behavior)
           if i < 0: skip scroll
t=0..    LaunchedEffect delays 500 ms
t=500    animateScrollToItem(i)
t=500    highlightKey = null                          (always, even if i < 0)
t=500+   target row composes into view -> LaunchedEffect(Unit) in highlightBackground:
           flag = true; delay 3000 ms; flag = false
         colour animates: 600 ms start delay, 5 reverse pulses of 200 ms, then fade-out 200 ms at 3000 ms
```

Notes:
- The captured `highlightKey` is a local for the composition, so the rows keep their highlight state even after the global is reset at 500 ms.
- A row that is scrolled out and back in re-runs its `LaunchedEffect(Unit)` and flashes again (Compose restart semantics). Kurobayomi flashes once per reveal (T-E02 and §6.2 of the rust design).

### A.3 Kurobayomi sequence
```
build page:   key = highlight::take()                          (clears always: T-E01, T-R04)
              map = title -> first ListBoxRow for rows that were built (disabled groups not built)
if key in map:
  500 ms later: scroll row into view (compute_point + vadjustment), flash(row)
flash(row):   t=0 add .pref-flash; t=600 ms start 10 toggles of .pref-flash-off at 200 ms; t=3000 ms remove both
```

Differences from Kotlin, all deliberate:
- Index arithmetic replaced by widget lookup. This fixes E-07 (wrong slot when a group is disabled).
- Duplicate titles flash only the first row (E-02).
- Slider and tracker rows can flash (E-08, E-17). Tracker rows are matched by tracker name, not by an empty title.
- Unmapped rows are not flashed (E-14), because a collapsed row has no visible area.

### A.4 Threading
- `take()` and `flash()` run on the GTK main thread only. Storage is `thread_local!` (rust-design §6.1), so a non-GTK thread cannot read the request.
- Timers: `glib::timeout_add_local_once` and `glib::timeout_add_local` on the main context. No tokio timers are used for UI.
- Cancellation: each row keeps its timer `SourceId`s; a new `flash` removes the previous ones (P-flash-idempotent). Destroying the page drops the timers because the closures hold weak references to rows (test T-R08).

### A.5 Failure modes
- Request set but no page built (navigation cancelled): the next page build clears it (T-R04). Kotlin's global would leak into the next page unless it composes; Kurobayomi clears on any build.
- Request names a title that does not exist: nothing happens, request cleared (T-E01).
- Request set twice before a build: last value wins (single slot).

## Part B. Download-queue state (More tab row)

### B.1 Inputs
Four values, each from the Kotlin download managers:
- `MangaDownloadManager.isDownloaderRunning` (Boolean), `MangaDownloadManager.queueState` (list, only `size` used).
- `AnimeDownloadManager.isDownloaderRunning`, `AnimeDownloadManager.queueState`.

Sources: `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:64-71`, `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt:62-69`.

### B.2 Derivation (Kotlin, OBSERVED)
```
size      = mangaQueue.size + animeQueue.size
isRunning = mangaRunning || animeRunning
state     = if size == 0       -> Stopped
            else if !isRunning -> Paused(size)
            else               -> Downloading(size)
```
Kotlin computes it in two nested `collectLatest` blocks (`app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt:102-130`). Each change restarts the inner collection. The final value is the same as the flat formula.

### B.3 Kurobayomi
- One `tokio::select!` over four `watch::Receiver::changed()` futures; on any change, recompute `derive(...)` from the latest four values and `send` if changed (`watch::Sender::send_if_modified`, Proposal to avoid redundant GTK updates).
- The GTK side consumes the state with `glib::spawn_future_local` and updates the row subtitle (`queue_state::subtitle`).
- Truth table and subtitle formats: T-E54, T-E53, T-E55, T-E56 in the edge-case file.

### B.4 Subtitle strings (English, exact)
- `Stopped` → no subtitle.
- `Paused{0}` → `Paused`.
- `Paused{N}` → `Paused • N remaining` (U+2022, single spaces).
- `Downloading{N}` → `N remaining`.
- Plural forms for English are identical (`plurals.xml:33-36`).

### B.5 Edge cases
- Queue size changes while paused: the state stays `Paused` with the new size (derive recomputed).
- Manager shuts down: the watch senders drop; the combiner keeps the last value (Kotlin behaviour is undefined here; Proposal: publish `Stopped` when both senders drop).
- A download runs with an empty queue (the last item is in flight): `size` counts queued items only. Kotlin shows `Stopped`; Kurobayomi keeps that rule for parity (T-E54 row 2). Review note: a running download with no queued items is invisible in this row.

### B.6 Acceptance
- The row subtitle changes within one GTK frame after a queue change (manual check, `p1-edge-cases-and-tests.md §5`).
- No polling: the combiner wakes only on watch changes.
