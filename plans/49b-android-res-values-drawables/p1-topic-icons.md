# Topic: icon catalogue, export procedure and GTK icon names

Source drawables: `app/src/main/res/drawable/` in `/home/blackbox/aniyomi-src` (repo-relative). Output: `data/icons/` in Kurobayomi. Every icon in this topic is used by the Kotlin code cited in the inventory. Viewports come from each file's `android:viewportWidth`/`viewportHeight`.

## 1. Export procedure (run once; output is committed)

1. Read the source file with `xml.etree.ElementTree` (namespace `http://schemas.android.com/apk/res/android`).
2. Take `viewBox = "0 0 <viewportWidth> <viewportHeight>"`. Use the square size `width="16" height="16"` for symbolic icons (GTK's symbolic size), keeping the viewBox.
3. For each `<path>` (in document order) with `android:pathData`, emit `<path d="<pathData verbatim>" fill="#bebebe"/>`. Add `fill-rule="evenOdd"` only when the source sets `android:fillType="evenOdd"`. Drop `android:fillColor`, `android:fillAlpha`, `android:strokeWidth` (symbolic icons are filled only).
4. Skip paths whose `android:fillColor` is `@android:color/transparent`, and skip paths with `android:strokeAlpha="0"` (invisible strokes).
5. Save as `data/icons/hicolor/scalable/actions/<name>.svg` (or `apps/` for the logo, see section 4).
6. Check: `grep -L 'fill="#bebebe"' data/icons/hicolor/scalable/actions/kb-*-symbolic.svg` prints nothing (T7).
7. Check: render at 16 px with `rsvg-convert -w 16 -h 16` and compare to the Android render at the same size.

Aspect ratio rule. The source drawable may have a non-square `android:width`/`android:height` (the seek triangle is 16dp x 20dp on a 24 viewport). Compose draws vector drawables with `ContentScale.Fit` (aspect preserved), so the drawn square is `min(w/vw, h/vh) * vw` centred in the box. For the seek triangle that is a 16 x 16 px square centred vertically in the 16 x 20 box. The export always uses a square viewBox and the widget displays the icon at 16 px, centred. This reproduces the Android layout.

Viewport notes:
- Most drawables use a 24 viewport. Exceptions: `outline_brand_family_24` (960), `sharp_shadow_24` (960), `ic_dns_24dp` (960, replaced, not exported), `ic_calendar_clock_24dp` (960, replaced, not exported), `ic_animelibrary_*` (24), `ic_updates_outline_24dp` (24), `ic_ani.xml` (512), `ic_ani_monochrome_launcher.xml` (108), `ic_launcher_foreground.xml` (108), `anim_library_enter.xml` (1080, base vector), `anim_more_enter.xml` (1080, not exported).
- The export keeps the viewBox from the source; no rescaling is applied (the SVG is scale-free).

## 2. Icons exported (symbolic, `kb-*`)

Each row: Kurobayomi icon name (file `data/icons/hicolor/scalable/actions/<name>.svg`) | Android source | viewport | Kotlin users | Notes.

| Kurobayomi name | Android source | viewport | Kotlin users (repo-relative) | Notes |
|---|---|---|---|---|
| kb-animelibrary-filled-symbolic | app/src/main/res/drawable/ic_animelibrary_filled_24dp.xml | 24 | selector (`ic_animelibrary_selector_24dp.xml`) | selected anime library tab |
| kb-animelibrary-outline-symbolic | app/src/main/res/drawable/ic_animelibrary_outline_24dp.xml | 24 | selector | unselected anime library tab |
| kb-crop-symbolic | app/src/main/res/drawable/ic_crop_24dp.xml | 24 | app/src/main/java/eu/kanade/presentation/reader/appbars/BottomReaderBar.kt | crop on |
| kb-crop-off-symbolic | app/src/main/res/drawable/ic_crop_off_24dp.xml | 24 | BottomReaderBar.kt | crop off (4 paths) |
| kb-done-previous-symbolic | app/src/main/res/drawable/ic_done_prev_24dp.xml | 24 | app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt | mark previous as read |
| kb-download-circle-symbolic | app/src/main/res/drawable/ic_download_item_24dp.xml | 24 | app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt | downloaded indicator |
| kb-forward-10-symbolic | app/src/main/res/drawable/ic_forward_10_24dp.xml | 24 | app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt (reused, Proposal) | 3 paths |
| kb-glasses-symbolic | app/src/main/res/drawable/ic_glasses_24dp.xml | 24 | AnimeExtensionDetailsScreen.kt, MangaExtensionDetailsScreen.kt, MoreScreen.kt | |
| kb-overflow-vertical-symbolic | app/src/main/res/drawable/ic_overflow_24dp.xml | 24 | app/src/main/res/layout/download_item.xml | vertical three dots |
| kb-seek-triangle-symbolic | app/src/main/res/drawable/ic_play_seek_triangle.xml | 24 (16dp x 20dp box) | app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/DoubleTapSeekTriangles.kt | see aspect rule |
| kb-reader-default-symbolic | app/src/main/res/drawable/ic_reader_default_24dp.xml | 24 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt | |
| kb-reader-ltr-symbolic | app/src/main/res/drawable/ic_reader_ltr_24dp.xml | 24 | ReadingMode.kt | never mirrored |
| kb-reader-rtl-symbolic | app/src/main/res/drawable/ic_reader_rtl_24dp.xml | 24 | ReadingMode.kt | never mirrored |
| kb-reader-vertical-symbolic | app/src/main/res/drawable/ic_reader_vertical_24dp.xml | 24 | ReadingMode.kt | |
| kb-reader-continuous-vertical-symbolic | app/src/main/res/drawable/ic_reader_continuous_vertical_24dp.xml | 24 | ReadingMode.kt | |
| kb-reader-webtoon-symbolic | app/src/main/res/drawable/ic_reader_webtoon_24dp.xml | 24 | ReadingMode.kt | |
| kb-updates-outline-symbolic | app/src/main/res/drawable/ic_updates_outline_24dp.xml | 24 | app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt | Updates in More |
| kb-font-family-symbolic | app/src/main/res/drawable/outline_brand_family_24.xml | 960 | app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt | |
| kb-text-shadow-symbolic | app/src/main/res/drawable/sharp_shadow_24.xml | 960 | SubtitleSettingsTypographyCard.kt:348 | |

Custom icons with no Android drawable of their own (built from the base state of an AVD):

| Kurobayomi name | Source | Construction |
|---|---|---|
| kb-library-symbolic | app/src/main/res/drawable/anim_library_enter.xml, target `bookmark` (base vector path `bookmark`, path `angle`) | Export the two paths of the base `<vector>` (viewport 1080) as in the algorithm. The base vector of this AVD has no transform. Used for the manga library tab and the desktop action "Library". |
| kb-compass-symbolic | app/src/main/res/drawable/anim_browse_enter.xml, base vector | Export paths `dot` (fill `#000000`, inside group `group` with no transform), `compass` (fill), and `ring` (fill, inside group `ring_parent` with `scaleX=0.833`, `scaleY=0.833`, pivot 12,12). Apply `transform="translate(12 12) scale(0.833) translate(-12 -12)"` to the `ring` path only. Viewport 24. Used for the Browse tab. |

Notes on the custom icons:
- The compass `ring` path data contains its own inner subpath (the hole, starting `M 12 21.6 ...`). Keep the pathData verbatim. Android's default fill type is `nonZero`, and the export uses the same default, so the hole renders as in Android.
- The bookmark path in the 1080 viewport is taken from the base state (valueFrom of the AVD is the animation start, so use the `<vector>` path data, not `valueFrom`).

## 3. Standard names (Replace decisions; no file exported)

These Android drawables are replaced by a freedesktop/GTK icon name. The name is looked up in the user's icon theme with GTK's fallback chain. Table: Android drawable (inventory row) | GTK name | Kotlin users.

| Android drawable | GTK icon name | Kotlin users |
|---|---|---|
| ic_book_24dp | accessories-dictionary-symbolic | MangaDownloadNotifier.kt, AnimeDownloadNotifier.kt, MangaLibraryUpdateNotifier.kt, AnimeLibraryUpdateNotifier.kt |
| ic_calendar_clock_24dp | x-office-calendar-symbolic | AnimeInfoHeader.kt |
| ic_close_24dp | window-close-symbolic | notifiers (5 files), HttpServerService.kt, TorrentServerService.kt, AppUpdateNotifier.kt, BackupNotifier.kt |
| ic_dns_24dp | network-server-symbolic | HttpServerService.kt |
| ic_done_24dp | object-select-symbolic | MangaLibraryUpdateNotifier.kt, AnimeLibraryUpdateNotifier.kt |
| ic_drag_handle_24dp | list-drag-handle-symbolic | download_header.xml, download_item.xml |
| ic_extension_24dp | application-x-addon-symbolic | ExtensionUpdateNotifier.kt |
| ic_folder_24dp | folder-symbolic | BackupNotifier.kt |
| ic_info_24dp | dialog-information-symbolic | AppUpdateNotifier.kt |
| ic_pause_24dp | media-playback-pause-symbolic | AnimeDownloadNotifier.kt, MangaDownloadNotifier.kt, PipActions.kt |
| ic_photo_24dp | image-x-generic-symbolic | SaveImageNotifier.kt |
| ic_play_arrow_24dp | media-playback-start-symbolic | AnimeDownloadNotifier.kt, MangaDownloadNotifier.kt, PipActions.kt |
| ic_refresh_24dp | view-refresh-symbolic | AnimeLibraryUpdateNotifier.kt, MangaLibraryUpdateNotifier.kt, AppUpdateNotifier.kt |
| ic_share_24dp | document-send-symbolic | BackupNotifier.kt, SaveImageNotifier.kt |
| ic_skip_next_24dp | media-skip-forward-symbolic | PipActions.kt |
| ic_skip_previous_24dp | media-skip-backward-symbolic | PipActions.kt |
| ic_system_update_alt_white_24dp | software-update-available-symbolic | AppUpdateNotifier.kt |
| ic_warning_white_24dp | dialog-warning-symbolic | AnimeDownloadNotifier.kt, MangaDownloadNotifier.kt, AnimeLibraryUpdateNotifier.kt, MangaLibraryUpdateNotifier.kt, AppUpdateNotifier.kt |
| ic_ani (logo, see section 4) | io.github.blackbox.Kurobayomi | 8 files (section 4) |
| ic_animelibrary_selector_24dp | two-state: `kb-animelibrary-outline-symbolic` / `kb-animelibrary-filled-symbolic` (section 2) | app/src/main/res/menu/main_nav.xml (no Kotlin user) |
| anim_caret_down (static states) | pan-down-symbolic / pan-up-symbolic | AnimeInfoHeader.kt, MangaInfoHeader.kt |
| anim_history_enter (static) | document-open-recent-symbolic | HistoriesTab.kt, DownloadsTab.kt |
| anim_updates_enter (static) | software-update-available-symbolic | UpdatesTab.kt, CategoriesTab.kt, StatsTab.kt, StorageTab.kt |
| anim_more_enter (static) | view-more-horizontal-symbolic | MoreTab.kt |
| cover_error (glyph) | image-x-generic-symbolic | ItemCover.kt, BrowseMangaIcons.kt, BrowseAnimeIcons.kt |
| ic_tachi_splash | (logo, section 4) | values/themes.xml |
| sc_* shortcut icons | desktop action names (design section 7) | app/shortcuts.xml |

Note: `ic_video_chapter_20dp.xml` has no row here because it is unused in Kotlin and is dropped (inventory, T-49ba-041); it needs no replacement.

## 4. Logo and launcher icons

- In-app logo (`ic_ani.xml`, 512 viewport, two paths, both `#FFFFFF` in Android): export as `data/icons/hicolor/symbolic/apps/io.github.blackbox.Kurobayomi-symbolic.svg` with `#bebebe` fills (the Compose LogoHeader tints it `onSurface`, so symbolic is the correct form). Name is the `Icon::LogoSymbolic` stem.
- Full-colour app icon (`ic_launcher_foreground.xml`, 108 viewport, colours kept): export as `data/icons/hicolor/scalable/apps/io.github.blackbox.Kurobayomi.svg`. Keep `android:fillColor` values (`#000`, `#2E84BF`, `#69A3CB`, `#C92824`, `#FFFFFF`, `#FF000000`); do not set `#bebebe`. The two circle paths are `M54,54m-25,0a25,25 0,1 1,50 0a25,25 0,1 1,-50 0` and the r=20.051 circle; keep the arc commands verbatim. The adaptive background is not exported (the mipmap background is transparent).
- Monochrome launcher (`ic_ani_monochrome_launcher.xml`, 108 viewport): not installed on Linux; its only use is the monochrome adaptive icon. Export as `symbolic/apps/io.github.blackbox.Kurobayomi-monochrome-symbolic.svg` if a tray icon is added later (Proposal; the file is kept, not required).
- The splash logo (`ic_tachi_splash`) is `ic_ani` at 108 dp, centred: use the symbolic logo at 108 px in the splash page; no separate export.

## 5. Shipping and loading

- GResource: `data/kurobayomi.gresource.xml` lists the `data/icons/hicolor/...` files under the prefix `/io/github/blackbox/Kurobayomi/icons/` (so `icons::ICON_RESOURCE_PATH` is the root of a theme directory).
- `IconTheme::add_resource_path("/io/github/blackbox/Kurobayomi/icons")` makes the files resolvable by name; the directory must follow the theme layout `scalable/actions/`, `scalable/apps/`, `symbolic/apps/`.
- For the `.desktop` file to show the custom action icons outside the app, also install the same SVGs under `share/icons/hicolor/scalable/actions/` (Proposal, see design section 7).

## 6. Pixel checks

- The symbolic export at 16 px: check the fine details of the reader icons (the page outlines and arrows) stay legible at 16 px; if a detail disappears, the icon is still correct at 24 px and the 16 px size is the only exception to note.
- Check a dark theme (Tokyo Night) and a light theme: the recoloured icon must match the widget foreground in both.
- Check that `kb-reader-ltr` and `kb-reader-rtl` are not mirrored under an Arabic locale (edge case I3).
