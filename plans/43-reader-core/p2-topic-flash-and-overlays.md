# Topic: e-ink flash, loading indicator, chapter transition host, retry button (43-reader-core part 2)

Scope files: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderProgressIndicator.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderTransitionView.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderButton.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/MissingChapters.kt`. Consumer outside the scope read for exact behavior: `app/src/main/java/eu/kanade/presentation/reader/DisplayRefreshHost.kt` (the flash implementation; part 1 presentation, specified here because it is driven entirely by this scope's preferences).

## 1. E-ink flash

### 1.1 Trigger
`ReaderActivity.kt:225`: `ReaderViewModel.Event.PageChanged -> displayRefreshHost.flash()`. The host is composed only when `flashOnPageChange` is true (`ReaderActivity.kt:433-437`).

### 1.2 Counter rule
```
flash():
    if (timesCalled % flashInterval == 0) currentDisplayRefresh = true
    timesCalled += 1
setInterval(n): flashInterval = n; timesCalled = 0
```
With interval 1 every page flashes; with interval 3 pages 1, 4, 7, … flash (the first page change always flashes, since 0 % n == 0).

### 1.3 Color sequence (`DisplayRefreshHost.kt:52-71` composable)
```
on currentDisplayRefresh == true:
    half = flashMillis / 2
    color = (mode == BLACK) ? Black : White
    delay(half)
    if (mode == WHITE_BLACK) color = Black
    delay(half)
    currentDisplayRefresh = false   -> color = null (nothing drawn)
```
Durations are integers in ms from `pref_reader_flash_duration` (default 100, slider 100..1500 ms). `half` is `refreshDuration.milliseconds / 2` (Duration, so odd values truncate by Duration arithmetic).

### 1.4 Preferences and their settings controls
| Key | Default | Control |
|---|---|---|
| pref_reader_flash | false | switch "Flash on page change" |
| pref_reader_flash_duration | 100 (ms) | slider 1..15 that stores value × 100; subtitle "%1$s ms" |
| pref_reader_flash_interval | 1 | slider 1..10; subtitle plural "pref_pages" |
| pref_reader_flash_mode | BLACK | list: Black, White, White and Black |

All controls except the switch are disabled when the switch is off (`SettingsReaderScreen.kt` getEInkGroup).

### 1.5 Edge behaviors
- A page change while a flash is running: `currentDisplayRefresh` is already true, so the effect does not restart; the second page's flash is absorbed (counted, no extra flash).
- Interval 0 is impossible through the UI (slider min 1) but would throw on `%`; Rust clamps to ≥ 1.
- The canvas is `fillMaxSize`, drawn on top of the reader while active; it does not consume touch in Kotlin (no pointer input modifier). Rust: `can-target = false`.

### 1.6 Rust
`src/reader/flash.rs::FlashHost` (P2, T-43b-043, T-43b-044): `flash()`, `set_interval()`, `is_active()`; drawing with `gtk::Snapshot::append_color` in a `DrawingArea`/overlay child. Color steps use `glib::timeout_add_local_once` for half durations; a flag prevents restart while active (matches Kotlin).

## 2. Loading indicator (`ReaderProgressIndicator`)

- Class: `ReaderProgressIndicator : AbstractComposeView`. Layout params `WRAP_CONTENT` both ways, gravity center.
- Progress state: `progress` (Float 0..1), set from `setProgress(Int)` with `@IntRange(0..100)`, `progress = value / 100f`. Visibility from `show()`/`hide()` (`isVisible`).
- Composable: `TachiyomiTheme { CombinedCircularProgressIndicator(progress = { progress }) }`. The indicator "always rotates" (class KDoc) so the user sees the reader is busy.
- Owner: `ReaderActivity.kt:137` field, `:559` construction (part 1).
- Rust: `LoadingIndicator` = `gtk::Box` with `gtk::Spinner` (indeterminate) + optional percentage label; `set_progress` clamps 0..=100. Visible on load, hidden on ready.

## 3. Chapter transition host (`ReaderTransitionView`)

- Class: `ReaderTransitionView : AbstractComposeView`. Embedded by `PagerTransitionHolder.kt:61` (pager) and the webtoon viewer for chapter boundaries (area 44).
- `bind(transition, downloadManager, manga)`:
  1. `manga == null` → `data = null`, nothing renders.
  2. `currChapterDownloaded = transition.from.pageLoader?.isLocal == true`.
  3. `goingToChapterDownloaded = manga.isLocal() || (transition.to?.chapter?.let { downloadManager.isChapterDownloaded(chapterName = …name, chapterScanlator = …scanlator, mangaTitle = manga.title, sourceId = manga.source, skipCache = true) } ?: false)`.
- Rendering: `TachiyomiTheme { CompositionLocalProvider(LocalTextStyle = typography.bodySmall, LocalContentColor = colorScheme.onBackground) { ChapterTransition(...) } }`.
- Note: `ChapterTransition` (the composable, `presentation/reader/ChapterTransition.kt`, part 1) and `ChapterTransition` (the model, `ui/reader/model`) share a name; both are imported in `ReaderTransitionView.kt`.
- `skipCache = true` means the download manager checks the filesystem for each bind (no cached answer). Cost: one filesystem check per transition bind.
- Rust: `ChapterTransitionView::bind(&transition, manga, &dl_index)` computes the same two booleans. Proposal: `dl_index` answers from the in-memory index and a fresh check on bind (matching `skipCache = true`); updates when a download finishes for the target chapter.

## 4. Chapter gap (`MissingChapters.kt`)

- Wrapper: `calculateChapterGap(higher: ReaderChapter?, lower: ReaderChapter?)` → domain `calculateChapterGap(Chapter?, Chapter?)` (`domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt:40-49`).
- Domain rules: null or unrecognized number (`chapterNumber < 0`) → 0; either number < 0 → 0; else `floor(higher) - floor(lower) - 1`.
- Shown by the chapter transition only when `> 0` (`presentation/reader/ChapterTransition.kt:120`). Values of -1 (e.g. 5.0 and 5.5) and 0 are hidden.
- Used also by the manga details screen (`MangaScreenModel.kt:1180`, area 38) with the same function; the Rust function is shared (`src/reader/chapter_gap.rs`, re-exported to details).

## 5. Retry button (`ReaderButton`)

- `MaterialButton` subclass with a `viewer: PagerViewer?` back-reference. `onTouchEvent` disables the pager gesture detector on every event and re-enables it on `ACTION_UP` (`ReaderButton.kt:109-115`).
- Used by `PagerTransitionHolder.kt:124` for the retry action on a failed page or chapter load.
- Known gap: `ACTION_CANCEL` does not re-enable the pager gesture detector. Rust: plain `gtk::Button` (T-43b-028); no coupling is needed because GTK routes the click to the button.

## 6. Constants

| Name | Value | Source |
|---|---|---|
| flash duration default | 100 ms (`MILLI_CONVERSION`) | ReaderPreferences.kt:20, :209 |
| flash duration slider | 1..15 (×100 ms) | SettingsReaderScreen.kt (getEInkGroup) |
| flash interval slider | 1..10 | SettingsReaderScreen.kt (getEInkGroup) |
| flash color steps | BLACK; WHITE; WHITE then BLACK (half each) | DisplayRefreshHost.kt |
| progress range | 0..100 (IntRange) | ReaderProgressIndicator.kt:171 |
| chapter gap display | > 0 | ChapterTransition.kt:120 |
