# Area 34 — Images, covers and cache: UI parity

Scope: the presentation surfaces of the 23 scope files, plus the cover and background surfaces that the scope's image code feeds. Behavior details are in `p1-behavior.md` (cited "behavior §N"); data in `p1-data-and-config.md`; the Rust side in `p1-rust-design.md`. Source paths are repo-relative to `/home/blackbox/aniyomi-src`.

Mockup references are repo-relative to `/home/blackbox/kurobayomi` (`src/ui.rs`, `src/entry.rs`, `src/screens.rs`, `spec/entries.json`).

---

## 1. Surfaces

| # | Surface | Kotlin composable or view | Scope file that drives it | Where it appears |
|---|---|---|---|---|
| S1 | Cover tile | `ItemCover` (`app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt`) | `Resources.kt`, `MangaCoverKeyer.kt`, `AnimeCoverKeyer.kt`, fetchers | library grid and list, browse, history, updates, migration, detail header |
| S2 | Browse cover error icon | `BrowseMangaIcons.kt:94`, `BrowseAnimeIcons.kt:94` | `Resources.kt` | browse source cards |
| S3 | Full cover viewer (manga) | `MangaCoverDialog` (`app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt`) | fetchers, `ImageSaver`, `ViewExtensions` (`ReaderPageImageView`) | opened from manga detail cover tap (`MangaScreen.kt` FullCover dialog) |
| S4 | Full cover and background viewer (anime) | `AnimeImagesDialog` (`app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt`) | same, with `useBackground` | anime detail cover tap; background via pager |
| S5 | Edit / delete cover menu | `DropdownMenu` inside S3/S4 | `EntryScreenConstants.kt` (`EditCoverAction`) | top-right pill |
| S6 | Image picker | `ActivityResultContracts.GetContent` (`MangaScreen.kt:262`, `AnimeScreen.kt:333`) | — | after Edit |
| S7 | Snackbars for save, share, cover update | `SnackbarHost` in S3/S4; `MangaCoverScreenModel`, `AnimeImageScreenModel` | `ImageSaver` callers | bottom of viewer |
| S8 | Save notification | `SaveImageNotifier` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt`) | `ImageSaver` | system notification tray |
| S9 | Storage usage row and clear action | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (~290-335) | `ChapterCache` | Settings > Data and storage |
| S10 | Auto-clear switch | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (`libraryPreferences.autoClearItemCache()`) | `ChapterCache` | Settings > Data and storage |
| S11 | Download item popup menu | `app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt:89`, `app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt:99` | `ViewExtensions.popupMenu` | download queue rows |
| S12 | Stats duration text | `app/src/main/java/eu/kanade/presentation/more/stats/MangaStatsScreenContent.kt:64`, `app/src/main/java/eu/kanade/presentation/more/stats/AnimeStatsScreenContent.kt:68` | `TimeUtils.toDurationString` | Stats screen |
| S13 | Last update text | `AnimeUpdatesUiItem.kt:68`, `MangaUpdatesUiItem.kt:62` | `relativeTimeSpanString` | updates screen header |
| S14 | Backup last-run text | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:280` | `relativeTimeSpanString` | Settings > Data and storage |
| S15 | Chapter and episode numbers | `MangaScreen.kt:793`, `AnimeScreen.kt:598,931,1085`, history items | `ItemNumberFormatter` | lists and headers |
| S16 | Error text for failed loads | `app/src/main/java/eu/kanade/presentation/browse/manga/BrowseMangaSourceScreen.kt:61`, `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt:61` | `ExceptionFormatter.formattedMessage` | browse source error states |
| S17 | Settings back press | `SearchableSettings.kt`, `SettingsMainScreen.kt`, `PlayerSettingsMainScreen.kt`, `AboutScreen.kt` | `Navigator.kt` `LocalBackPress` | settings screens |
| S18 | Tablet layout switch | `app/src/main/java/eu/kanade/presentation/components/AdaptiveSheet.kt:75`, `app/src/main/java/eu/kanade/presentation/reader/components/ChapterNavigator.kt:56` | `WindowSize.kt` | sheets and reader navigator |
| S19 | Page-screen transitions | `app/src/main/java/eu/kanade/presentation/components/AdaptiveSheet.kt:34` (`ScreenTransition`), main navigator | `Navigator.kt` | all screen changes |
| S20 | Install-permission banner | `app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt:54`, `*ExtensionsScreen.kt` | `Permissions.kt` | onboarding and extension screens (dropped on Linux, §12) |
| S21 | Secure-screen effect | none visible (window flag) | `WindowExtensions.kt` | everywhere (dropped on Linux, §12) |

---

## 2. Navigation transitions (S19) and back behaviour (S17)

### 2.1 Observed
- Forward and back transitions: `materialSharedAxisX(forward = navigator.lastEvent != StackEvent.Pop, slideDistance = rememberSlideDistance())`, inside `AnimatedContent(label = "transition")` (`Navigator.kt:68-102`). Forward slides right-to-left, back slides left-to-right, with a shared-axis fade.
- Each screen's state is kept with `navigator.saveableState("transition", screen)`.
- Back press: the composition local `LocalBackPress` holds the lambda that pops; settings screens read it and call it.

### 2.2 Linux behaviour (proposal)
- A `gtk::Stack` with `transition_type = Crossfade` and `transition_duration = 300` for every push and pop. The shared-axis slide is dropped (no equivalent in GTK without custom widgets; the crossfade is the GTK-native replacement). Decision: user-visible change, recorded here. The forward/back distinction is kept only for the animation direction: `SlideLeft` for push and `SlideRight` for pop, if the transition spike (T-34a-060) shows it is cheap; otherwise crossfade only.
- Stack state is kept per screen instance (the Voyager `saveableState` analogue): each pushed page widget stays alive while on the stack.
- Back: `Escape` and `Alt+Left` (window-level shortcut, mockup `src/main.rs:79` already implements both for the back action). Mouse back button (button 8) also pops.
- Reselecting a sidebar tab: scrolls its content to the top (Kotlin `Tab.onReselect`, `Navigator.kt:39-44`).

### 2.3 Reduced motion
When `gtk-enable-animations` is false, the transition duration is 0 (behavior §2.4).

---

## 3. Cover tiles (S1, S2)

### 3.1 Geometry
- Ratios: `Square` 1:1, `Book` 2:3, `Thumb` 16:9 (behavior §25.1).
- The tile is width-driven: the image fills the width and the height follows the ratio (`Modifier.aspectRatio(ratio)`).
- Corner shape: `MaterialTheme.shapes.extraSmall` (the theme's small radius; data §10). Mockup equivalent: the grey frame in `src/ui.rs:176-182` has no radius; the Rust tile should use a CSS class with `border-radius` from the theme token.
- Scaling: `ContentScale.Crop` (fill the box, crop the overflow centred). GTK: `gtk::Picture` `content_fit = Cover`, with the picture aligned centre.

### 3.2 States
| State | Visual | Trigger | Source |
|---|---|---|---|
| Loading | flat placeholder colour `#1F888888` (12 percent grey) | request in flight or `data == null` | `ItemCover.kt` `ColorPainter(CoverPlaceholderColor)` |
| Loaded | the image, crossfade 300 ms | decode finished | behavior §2.4 |
| Error | `cover_error` vector icon centred on the placeholder colour | any error except cancellation | `Resources.kt` (`rememberResourceBitmapPainter(R.drawable.cover_error)`) |
| Empty (no URL) | same as Error, since `Coil` reports `No cover specified` | `thumbnail_url` null | behavior §11.3 |
| Custom cover | the custom image is shown instead of the thumbnail; no visual label | custom file present | behavior §12 |

The error icon is the same asset in S1 and S2 (`cover_error.xml`). It is a plain icon, not a text message.

### 3.3 Interaction
- Click on a tile: calls the screen's `onClick` (opens the entry). In `ItemCover`, the tile is `clickable(role = Role.Button)` only when `onClick != null`.
- Desktop additions (new, not in Kotlin): hover shows a subtle outline (CSS `:hover`), focus ring on keyboard focus (`gtk::FlowBox` focus), `Enter` activates.
- Right-click on a library tile: opens the same selection or context menu that long-press opens on Android. The menu itself is owned by area:37-ui-library; this area only supplies the tile. Right-click is the desktop equivalent of long-press.

### 3.4 Sizes used by callers (Kotlin)
- Browse source card: cover width from the grid (`BrowseMangaIcons.kt`).
- Library grid: column width from the grid settings (area:37-ui-library).
- Detail header: `ItemCover` with `Book` ratio (area:38-ui-entry-details-manga/area:39-ui-entry-details-anime).
Mockup equivalents: `src/entry.rs:67` (`cover(w, w * 10 / 7)`), `src/entry.rs:91` (`cover(130, 190)`), `src/entry.rs:398` (`cover_grid(24, 130, ...)`), `src/screens.rs:99`, `src/screens.rs:184` (`cover(140, 200)`). Replace each with `CoverImage::new(ratio)` (design §14.2).

---

## 4. Full cover and background viewer (S3, S4)

### 4.1 Layout (observed; dimensions from Compose)
- Modal dialog that covers the whole screen: `usePlatformDefaultWidth = false`, `decorFitsSystemWindows = false`.
- Background: transparent `Scaffold`; the image covers the full area behind the bars.
- Bottom bar: `Row` with `fillMaxWidth()`, `padding(4.dp)`, `navigationBarsPadding()`.
  - Left pill (anime only has the switch): `IconButton` Close (`Icons.Outlined.Close`, label `action_close`); anime also has a switch button (`KeyboardArrowRight` when on cover, `KeyboardArrowLeft` when on background; label `action_edit_background` on cover and `action_edit_cover` on background).
  - `Spacer(weight 1f)`.
  - Right pill: `AppBarActions` with Share (`Icons.Outlined.Share`, `action_share`) and Save (`Icons.Outlined.Save`, `action_save`); then the Edit button (`Icons.Outlined.Edit`, label `action_edit_cover` or `action_edit_background`) when editing is allowed.
- Pills: rounded (`MaterialTheme.shapes.extraLarge`), background `colorScheme.background` at 95 percent opacity.
- Snackbar host above the bottom bar with dismiss action.
- Image area: `ReaderPageImageView` (pinch zoom and pan), `updatePadding(top = statusBarPx, bottom = bottomPx)` so the image is not hidden under the bars when it is scrolled to the edges.

### 4.2 Gestures (observed)
- Tap on the image (or the empty area): dismisses the dialog (`onViewClicked = onDismissRequest`; the outer `Box` is `clickableNoIndication(onClick = onDismissRequest)`).
- Pinch: zoom. Double-tap zoom: handled by SubsamplingScaleImageView defaults.
- Pan: drag.
- Zoom animation when an image loads: `ReaderPageImageView.Config(zoomDuration = 500)` (500 ms).
- Anime: horizontal swipe between cover (page 0) and background (page 1) through `HorizontalPager(pagerState)`.
- Back press (Android) dismisses the dialog.

### 4.3 Desktop equivalents (new)
| Android gesture | Desktop equivalent |
|---|---|
| tap image to dismiss | click on the dark backdrop, or `Escape` |
| pinch zoom | `Ctrl` + scroll wheel, `Ctrl` + `+`/`-`, `Ctrl` + `0` resets |
| double-tap zoom | double-click toggles fit / 100 percent |
| pan | drag with left button; arrow keys pan when zoomed |
| horizontal swipe (anime pages) | `Left` / `Right` arrow keys, or the switch button |
| back press | `Escape` (closes the viewer, not the app) |
| share, save, edit buttons | same buttons, plus keyboard shortcuts `Ctrl+S` (save), `Ctrl+Shift+C` (copy image), `E` (edit) |

### 4.4 States
| State | Visual |
|---|---|
| Loading | Empty dark backdrop; no spinner in Kotlin (the request has no placeholder). Rust: a centred `gtk::Spinner` (new, to avoid a blank screen). |
| Loaded | Image fitted to the window (SubsamplingScaleImageView "fit" default). |
| Error | Kotlin: no error UI (the view stays empty). Rust: the `cover_error` icon centred, no text. Deviation U-2: Rust adds the icon so a failed load is not mistaken for a blank cover. |
| Save in progress | none (Kotlin does not show progress for saves). |

### 4.5 Edit menu (S5)
- Shown when the Edit button is pressed and a custom image exists (cover: `isCustomCover`; background: `isCustomBackground`). Items in order: `Edit` (`action_edit`), then `Delete` (`action_delete`).
- If no custom image exists, pressing Edit opens the picker directly (`EditCoverAction.EDIT`).
- The menu is a `DropdownMenu` with `DpOffset(8.dp, 0.dp)`. Rust: `gtk::PopoverMenu` anchored to the Edit button, same two items, same order.
- Picker (S6): image files only (`image/*`). Rust: `gtk::FileDialog::open` with a `gtk::FileFilter` containing the MIME types `image/jpeg`, `image/png`, `image/gif`, `image/webp`, `image/avif`, `image/jxl`, `image/heif`, plus the patterns `*.jpg`, `*.jpeg`, `*.png`, `*.gif`, `*.webp`, `*.avif`, `*.jxl`, `*.heic`, `*.heif`. Cancel does nothing.

---

## 5. Cover and background edit flow (S5-S7)

| Step | Kotlin | Rust |
|---|---|---|
| Choose file | picker (S6) | `gtk::FileDialog` |
| Copy into cache | `editCover` / `editBackground` (behavior §19.3) | same logic, on the tokio runtime |
| Success | snackbar `cover_updated` / `background_updated` | toast-like label in the viewer's snackbar overlay (same text) |
| Failure | snackbar `notification_cover_update_failed` / `notification_background_update_failed` | same text |
| Delete | `deleteCustomCover` / `deleteCustomImage` (behavior §24.4) | same |
| Refresh | the composable reloads because `coverLastModified` changed (key change) | the tile rebinds on the entry change signal |
| Non-library anime edit | silently does nothing and still shows success (behavior §19.3, E-IMG-14) | Deviation U-1: show the failure text instead of a false success |

Deviation U-1 (new): the Kotlin code shows "Cover updated" for a non-favourite anime even though nothing was written. Rust reports `notification_cover_update_failed` for that case, because the user would otherwise see a change that is not there.

---

## 6. Save, share and copy (S7, S8)

### 6.1 Snackbars (viewer)
| Action | Success text | Failure text |
|---|---|---|
| Save manga cover | `Cover saved` | `Error saving cover` |
| Share manga cover | (opens share sheet; no text) | `Error sharing cover` |
| Save anime cover or background | `Cover saved` (cover) or `Background saved` | `Error saving cover` or `Error saving background` |
| Share anime image | (none) | `Error sharing cover` or `Error sharing background` |
Source: behavior §23.1 (strings in `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`).

### 6.2 Save notification (S8)
- Title `Picture saved`, preview image on the right (large icon), expanded big-picture style showing the image.
- Clicking the notification opens the image in the default viewer (`openImagePendingActivity`).
- Action button `Share` (Linux: opens the file in the default viewer and copies its URI, see design §10).
- Error notification: title `Error`, body the error text or `Unknown error`.
- One notification at a time: a new save replaces the old one (same id).
- Rust: `gio::Notification` with default action and button.

### 6.3 Reader and player (behavior §23.3)
Reader and player show their own result events (`SavedImage`, `ShareImage`, `CopyImage`); these are rendered by areas 43 and 45. This area supplies only the file and the notification.

---

## 7. Storage settings (S9, S10)

### 7.1 Observed layout (Settings > Data and storage)
- Group title: `Storage usage` (`pref_storage_usage`). Shows the storage info widget (`StorageInfo`).
- Text preference `Storage` (`label_storage`) with a chevron. Opens the storage tab.
- Text preference `Clear chapter and episode cache` (`pref_clear_chapter_cache`), subtitle `Used: %1$s` (`used_cache`) with the human-readable size of the chapter cache directory.
- Switch `Clear episode/chapter cache on app launch` (`pref_auto_clear_chapter_cache`), default off.

### 7.2 Clear action
- Tap: runs the clear in the background (`launchNonCancellable`).
- Success toast: `Cache cleared, %1$d files deleted` (`cache_deleted`), where the number is the count from `clear()`; the subtitle refreshes.
- Failure toast: `Error occurred while clearing` (`cache_delete_error`).
- Clear does not affect covers (they are not in the chapter cache).

### 7.3 Rust version
Same rows in the Kurobayomi settings spec (`spec/settings.json`): one `info` row for storage usage (computed string), one `action` row `Clear chapter and episode cache` with subtitle `Used: <size>`, one `switch` row for auto-clear. Toast: `adw`-free `gtk::Overlay` label, auto-dismiss 3 s (proposal; Android toast length is short, about 3.5 s).

### 7.4 Size label format
Decimal units from `Formatter.formatFileSize` (behavior §20.1). Rust: `humansize` decimal formatting (design §15) with one decimal place for values under 10 units; the exact Android rounding is not reproduced (O-7 family).

---

## 8. Download item popup menu (S11)

Items: `move_to_top` (visible only when the row position is greater than 1), `move_to_bottom` (hidden on the last row), plus the other items of `R.menu.download_single` (area:23-downloads-manga/area:24-downloads-anime). Opened from the row's overflow button (Android) or right-click (desktop, new). Rust: `gtk::PopoverMenu` built from a `gio::Menu` with the same item order; hidden items are removed from the menu model when the condition is false.

---

## 9. Numbers, durations and times on screen (S12-S15)

| Display | Rule | Example | Source |
|---|---|---|---|
| Chapter number | `#.###` HALF_EVEN, ASCII digits (Deviation D-15) | `12`, `12.5`, `12.333` | behavior §27.1 |
| Episode number | same | `3`, `3.5` | behavior §27.1 |
| Stats duration | `%dd %dh %dm %ds` rules | `1d 1h`, `5m 30s`, `1h` | behavior §27.2 |
| Last update | `Just now`, `Never`, relative phrase | `Just now`, `5 minutes ago` | behavior §27.3 |
| Last auto backup | same relative rule | `Never` | behavior §27.3 |

---

## 10. Responsive behaviour (S18)

- Tablet layout when the window's shorter side is at least 720 logical pixels (behavior §30.4). Android uses the smallest screen width in dp; Linux uses the window's shorter side (Proposal, design §14).
- Cover tile counts per row follow the grid width (area:37-ui-library), not the breakpoint.
- Viewer: fills the window at every size; the bottom bar wraps its pills to a second row below 480 px (new; Android uses the window insets only).
- Narrow windows (below 360 px): the viewer's right pill drops the Share button into the overflow menu (new).
- Desktop window resizing re-fits the viewer image (SubsamplingScaleImageView behaviour).

---

## 11. Keyboard and mouse map (new for desktop)

| Action | Key or mouse | Surface |
|---|---|---|
| Open a cover viewer | click the cover | S1 → S3/S4 |
| Close viewer | `Escape`, click backdrop | S3/S4 |
| Switch cover / background | `Left`, `Right`, switch button | S4 |
| Save image | `Ctrl+S`, Save button | S3/S4 |
| Share image | Share button (no shortcut) | S3/S4 |
| Copy image | `Ctrl+Shift+C` | S3/S4 (new; Kotlin copies only from reader) |
| Edit cover | `E`, Edit button | S3/S4 |
| Open download row menu | right-click, `Menu` key | S11 |
| Clear chapter cache | Settings row activation (`Enter`) | S9 |

---

Deviation U-2 (new): the viewer shows the `cover_error` icon when the load fails (§4.4). Kotlin shows nothing.

## 12. Dropped Android UI and the replacement decision

| Kotlin surface | Decision | User-visible effect |
|---|---|---|
| Secure screen (`FLAG_SECURE`, S21) | Dropped (Deviation D-16): no compositor-level screenshot block on Wayland or X11 | Incognito and "always secure" screenshot blocking is gone; the setting remains and shows the note "Not available on Linux" |
| Install-unknown-apps banner (S20) | Dropped (the extension install model differs, area:18-extensions-manga/area:19-extensions-anime) | No banner |
| Fast-scroll item animation (`FastScrollAnimateItem.kt`) | Dropped | No effect |
| `setComposeContent` / Compose hosting | Dropped | No effect |
| Assist content URL (`AssistContentScreen`) | Dropped (Android assist gesture) | No effect |
| Hardware-bitmap copy for the viewer | Replaced by a plain RGBA copy | No visible change |
| Display profile (ICC) setting | Deferred (P2, Deviation D-3) | The setting does nothing until the `icc` feature exists |

---

## 13. States and interactions matrix

| Surface | Loading | Empty | Error | Selected or focused | Disabled |
|---|---|---|---|---|---|
| S1 cover tile | placeholder colour | `No cover specified` shows error icon | `cover_error` icon | outline (new) | not applicable |
| S3/S4 viewer | dark backdrop (Rust: spinner) | no custom image: Edit opens picker directly | silent (Kotlin) / icon (Rust, U-2) | not applicable | Edit always enabled (Kotlin; Rust keeps it enabled) |
| S3/S4 pill buttons | enabled | enabled | enabled | focus ring | not applicable |
| S7 snackbar | auto-dismiss by action | not applicable | error text | not applicable | not applicable |
| S8 notification | none | none | `Error` + text | not applicable | not applicable |
| S9 clear row | subtitle shows the last size | `Used: 0 B` (decimal formatter, zero) | toast `Error occurred while clearing` | focus ring | not applicable |

---

## 14. Accessibility and labels

- Every icon-only button has a label (`contentDescription` in Kotlin; `tooltip_text` plus accessible label in GTK). Labels are the strings listed in behavior §23.1 and §24.1.
- Image content description: the Kotlin `ItemCover` uses `contentDescription = ""` by default (decorative). Rust: the tile has an accessible name from the entry title, so screen readers announce the entry.
- Colour: the placeholder colour and the error icon do not depend on colour alone (the icon is shown on error).
- Focus order in the viewer: Close, switch (anime), Share, Save, Edit, then the image.
- Reduced motion: transitions and zoom animation follow `gtk-enable-animations` (behavior §2.4, §26.2).

---

## 15. Mockup gaps (what `/home/blackbox/kurobayomi` lacks today)

Mockup inventory against the surfaces above:
1. `src/ui.rs:176-182` `cover(w, h)`: a grey frame with a generic icon. No image loading, no placeholder colour, no error state, no crossfade, no radius. (S1, S2)
2. `src/entry.rs:91-93`: the detail header uses `cover(130, 190)` and opens `full_images` or `full_cover`, but there is no viewer behind those ids. (S3, S4)
3. `spec/entries.json` `full_cover` (line 235) and `full_images` (line 849): the labels exist (`Close`, `Share`, `Edit cover`, `Edit background`, `Delete`, `Custom cover`, `Custom background`) but the viewer itself (pager, zoom, pills, snackbars, edit menu) is not implemented. (S3-S7)
4. No `Save`/`Share`/`Copy` behaviour; no file dialog for cover edits; no custom cover or background deletion. (S5, S6)
5. No save notification. (S8)
6. Storage settings (S9, S10): the mockup has no storage row or auto-clear switch; `spec/settings.json` should gain them (ui §7.3 rows).
7. No download row popup menu behaviour. (S11)
8. Duration, relative time and number formatting are not present in the mockup. (S12-S15)
9. No tablet breakpoint and no window-width handling for covers. (S18)
10. No keyboard map for viewers (§11). (S3, S4)
11. The `ponytail:` note at `src/ui.rs:176` documents the placeholder; remove it when the loader exists (T-34a-052).

---

## 16. Acceptance checklist for the UI (for the tasks in `p1-tasks.md`)

- Every row of §1 has a visible implementation or an explicit drop in §12.
- Every snackbar and notification text in §6 matches behavior §23.1 exactly.
- Viewer dismiss, zoom, pan and switch work with mouse and keyboard (§4.3, §11).
- Edit menu order is Edit then Delete; the Edit button opens the picker when no custom image exists (§4.5).
- The desktop-only additions in §3.3, §4.3, §10 and §11 are listed as new in the task acceptance (T-34a-052, T-34a-053).
