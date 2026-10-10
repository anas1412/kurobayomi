# Area 49b (part 1): UI parity for the Android resources in scope

This area renders UI only through the resources it owns: icons, the splash screen, the surface and text styles, the tab icons and the animations between their states. The Compose screens that consume them are cited by path. Desktop equivalents are given for each element. "Mockup" means the existing Kurobayomi code in `/home/blackbox/kurobayomi/src/` and its `spec/*.json`.

Layout constants used below: phone margin 16 px, tablet margin 24 px (threshold: window width at least 720 px, the GTK equivalent of `sw720dp`).

---

## 1. Splash screen (first launch of the process)

Visible elements (OBSERVED, `values/themes.xml:659-668`, `MainActivity.kt:149-175, 316-322, 418-456`):
- Full-window background `splash` colour: `#54759E` (light) or `#202125` (dark).
- Centred logo `ic_ani` at 108 dp (the same logo as the More-tab header).
- No text, no spinner.

States:
- Loading: the splash is visible until `elapsed > 500 ms` AND `ready`, or until `elapsed > 5000 ms` regardless of `ready`. Condition: keep while `elapsed <= 500 || (!ready && elapsed <= 5000)`.
- Ready: `ready` becomes true when the Browse tab first composes (`BrowseTab.kt:103-104`).
- Exit: 400 ms. Pre-Android-12 only: content slides up 16 dp and the splash fades out together. Android 12+ uses the system splash exit.
- Configuration change: no splash (`isLaunch == false`).

Desktop equivalent (PROPOSAL):
- Window opens with a `gtk::Stack` page named `splash` (`.kb-splash`, logo 108 px, `Icon::LogoSymbolic` at the logo size or the full-colour app icon via `gtk::Image::from_resource`).
- Ready signal = the first main-screen page (the library) has been built. Keep min 500 ms, max 5000 ms, then `Stack::set_visible_child_name("main")` with `Crossfade` and `transition-duration` 400.
- Dark mode: the splash page gets class `kb-dark` when the window is dark, so `#202125` applies.
- Keyboard and mouse: the splash ignores input; there is no interaction to lose.
- Responsive: the logo stays 108 px at any window size (Android keeps a fixed size).

Mockup gap: the mockup opens straight to the main window (`src/main.rs:22-74`) with no splash page and no ready gating.

---

## 2. Navigation tabs (icons and their animation)

Visible elements (OBSERVED from the Compose tabs; each tab's `TabOptions.icon` is a `rememberAnimatedVectorPainter` over one AVD):

| Tab | Android index (`TabOptions.index`) | Icon resource | Selected state | Android label key |
|---|---|---|---|---|
| Anime library | 0 (`AnimeLibraryTab.kt`) | `anim_animelibrary_leave` (base state: `ic_animelibrary_*`) | `isSelected` | `AYMR.strings.label_anime_library` |
| Manga library | 1 (`MangaLibraryTab.kt:78`; 5 when moved to More) | `anim_library_enter` | `isSelected` | manga library label |
| Updates | (`UpdatesTab.kt`) | `anim_updates_enter` | `isSelected` | recent updates label |
| Browse | 3 (`BrowseTab.kt:43-47`) | `anim_browse_enter` | `current is BrowseTab` | `MR.strings.browse` |
| History | 3 or 5 (`HistoriesTab.kt:35-44`, moved by `NavStyle`) | `anim_history_enter` | `isSelected` | recent manga label |
| More | (`MoreTab.kt:49-53`) | `anim_more_enter` | `isSelected` | `label_more` |
| Downloads | (`DownloadsTab.kt:84-88`) | `anim_history_enter` | `isSelected` | download queue label |
| Categories | (`CategoriesTab.kt:38-42`) | `anim_updates_enter` | `isSelected` | categories label |
| Stats / Storage | (`StatsTab.kt:28-32`, `StorageTab.kt:28-32`) | `anim_updates_enter` | `isSelected` | stats / storage label |

Icon states:
- Selected and unselected states of the anime library entry: filled frame with play (selected) vs outline frame with play (unselected) (`ic_animelibrary_selector_24dp.xml`).
- The Updates glyph is a star burst; the More glyph is three dots; History is a clock with an arrow; Browse is a compass; Library is a bookmark stack.
- Icons are 24 dp in the Android nav bar.

Transitions:
- Selecting a tab plays its AVD forward over 300 ms (500 ms for History). Deselecting plays it in reverse.
- Navigation style (`NavStyle.kt:27-38`): `MOVE_UPDATES_TO_MORE` puts Updates into More with `ic_updates_outline_24dp`; `MOVE_HISTORY_TO_MORE` and `MOVE_BROWSE_TO_MORE` remove those tabs from the bar; `MOVE_MANGA_TO_MORE` uses `Icons.Outlined.CollectionsBookmark` (Material, not in scope).

Desktop equivalent (PROPOSAL):
- Left sidebar (wide) or bottom switcher (narrow) with one `gtk::ToggleButton` per tab, each containing an `Image` from `Icon::*`:
  - Anime library: `AnimeLibraryOutline` (unselected) / `AnimeLibraryFilled` (selected), crossfade 300 ms.
  - Manga library: `LibraryManga` (static), rotate none; selected state via `.checked` class.
  - Updates: `UpdatesTab` (static; decorative animation dropped to keep one icon per tab).
  - Browse: `Compass`; selected state rotates 180 deg (`Rotation::Half`, 300 ms).
  - History: `HistoryTab` (`document-open-recent-symbolic`); selected state rotates (`Rotation::FullBackSlow`, 500 ms).
  - More: `MoreHorizontal`.
- Keyboard: Alt+1..5 (Proposal) activate tabs; Tab/Shift+Tab move focus. Tooltips show the tab label.
- Mouse: click selects; middle click not used.
- Responsive: at 720 px and wider show a sidebar with icon and label; below, a bottom bar with icon only plus the label under it (label always visible per `NavigationBar` behavior in `spec/app.json:40`).

Mockup gap:
- Mockup tab icons are generic: `view-grid-symbolic` (Library), `view-refresh-symbolic` (Updates), `document-open-recent-symbolic` (History), `system-search-symbolic` (Browse), `view-more-horizontal-symbolic` (More) (`spec/app.json:1-30`). Replace with the mapping above.
- Mockup has no anime/manga split in the nav bar (it uses top tabs). That is a navigation decision outside this area; flag for the screens area.
- No selected-state animation; no tab icon changes.

---

## 3. Entry details: info header and expand/collapse

Visible elements (OBSERVED, `AnimeInfoHeader.kt:637-650`, `MangaInfoHeader.kt:605`):
- A bottom-centred circular gradient scrim with a caret icon (`anim_caret_down`) and a content description `MR.strings.manga_info_collapse` (when expanded) or `MR.strings.manga_info_expand` (when collapsed).
- The caret is tinted `onBackground`.
- Release-time row uses `ic_calendar_clock_24dp` (anime info header).

States:
- Expanded: caret points up. Collapsed: caret points down. The AVD plays on each change over 300 ms (drive: `!expanded`).

Desktop equivalent (PROPOSAL):
- A `gtk::Button` (`.flat`, `.circular`) with `Image` of `Icon::CaretDown` / `Icon::CaretUp`. Tooltip = the expand or collapse label. Activation toggles the header; the image swap is instant, the rotation uses `Rotation` classes (optional, P2).
- Keyboard: Space/Enter activate the button; focus ring from the theme.
- Mouse: click; also the whole header area toggles on double-click (Proposal, not in Android).
- Responsive: the caret is centred under the header at all widths.

Mockup gap: mockup's entry header (`src/entry.rs`) has no collapse caret.

---

## 4. Covers and placeholders

Visible elements (OBSERVED, `cover_error.xml`; `ItemCover.kt:35`):
- While a cover loads: the Coil placeholder (not from this area).
- On error: background `#1F888888` with a 24 dp image glyph inset 48 dp, tinted `cover_placeholder` (same colour).

States:
- Loading: placeholder; error: error glyph; loaded: image.

Desktop equivalent:
- `ui::cover()` (mockup `src/ui.rs:177-181`) already shows `image-x-generic-symbolic` at `w/3` pixel size with `dim-label`. Change its CSS class to `.kb-cover-placeholder` (`rgba(136,136,136,0.1216)`) so it matches Android's error look. Use `Icon::Cover`.
- Keyboard and mouse: none (decorative).
- Responsive: icon size scales with cover width (`w/3`), as in the mockup.

Mockup gap: mockup uses the theme's default frame colour; no `.kb-cover-placeholder` exists.

---

## 5. Library and downloads: queue rows, drag, overflow, indicators

Visible elements (OBSERVED, `download_header.xml`, `download_item.xml`, `EntryBottomActionMenu.kt`, `ChapterDownloadIndicator.kt`, `EpisodeDownloadIndicator.kt`):
- Download header row: drag handle (`ic_drag_handle_24dp`), section header text (`TextAppearance.Tachiyomi.SectionHeader`, label-large 14 px, medium weight, secondary text colour).
- Download item row: card with `cardForegroundColor = draggable_card_foreground`; drag handle on the left; overflow menu button (`ic_overflow_24dp`) on the right.
- Chapter and episode rows: download indicator `ic_download_item_24dp` (only when downloaded), plus the downloading state (other area).
- Bottom action menu: `ic_done_prev_24dp` "mark previous as read" (`EntryBottomActionMenu.kt`).

States:
- Idle: card transparent foreground.
- Dragged (`state_dragged`) or activated (`state_activated`): overlay with `colorPrimary` at alpha 0.08.
- Downloaded: indicator visible; not downloaded: hidden.

Gestures and keyboard/mouse equivalents (desktop):
- Drag reorder: press on the handle, drag vertically; the card gets `.kb-dragged` (alpha 0.08 of `theme_selected_bg_color`) while moving. Keyboard: select the row and use Ctrl+Up / Ctrl+Down to move it (Proposal, Android uses touch only).
- Overflow button opens a `gtk::PopoverMenu` (menu items from the screen area).
- Downloaded indicator: `Icon::DownloadCircle`, tooltip "Downloaded". Not clickable.
- Mark-previous action: `Icon::DoneMarkPrevious` as a button in the bottom bar of the entry screen (P1).

Responsive:
- Phone: the handle sits at the left edge with the 16 px margin; the overflow menu sits at the right edge.
- Wide: the same layout; the section header stays full width.

Mockup gap:
- Mockup `src/settings.rs:142-144` uses `list-drag-handle-symbolic` and `tag-symbolic` (close to `ic_drag_handle`, but no overflow menu icon, no card foreground tint, no download indicator).
- No keyboard reorder, no `.kb-dragged` state.

---

## 6. Reader: bottom bar crop toggle and reading mode selection

Visible elements (OBSERVED):
- `BottomReaderBar.kt:57-60`: crop-borders icon button. Icon: `ic_crop_24dp` when crop is on, `ic_crop_off_24dp` when off; content description `MR.strings.pref_crop_borders`. Toast on toggle (spec `reader_player.json:75`: "On" / "Off").
- Reading mode dialog (`ReadingMode.kt`): one row per reading mode with its icon: default, LTR, RTL, vertical, continuous vertical, webtoon (six `ic_reader_*`).

States:
- Crop: on/off icon. Reading mode: the selected mode is marked (other area's selection widget).

Desktop equivalent (PROPOSAL):
- Crop button: `gtk::ToggleButton` with `Icon::CropOn` when active? Choose: the toggle icon shows the state that is applied. Android: crop on = `ic_crop_24dp`; to match, when active show `Icon::CropOn`, else `Icon::CropOff`. Tooltip "Crop borders". Keyboard: `c` (Proposal). Mouse: click.
- Reading mode list: a `gtk::ListBox` with one row per mode: `Image` of `Icon::ReaderDefault`, `ReaderLtr`, `ReaderRtl`, `ReaderVertical`, `ReaderContinuousVertical`, `ReaderWebtoon` plus a label. Checkmark on the selected row (`object-select-symbolic`, `Icon::Done`).
- Responsive: the reading-mode list is a dialog on phone widths and a popover on wide windows (Proposal).

Mockup gap:
- Mockup crop button uses `edit-cut-symbolic` (`src/player.rs:100`), which does not match Android's crop glyph.
- Mockup reading-mode dialog (`spec/reader_player.json:549`, id `reading_mode_select`) has no icons. Add `Icon::Reader*` per row.

---

## 7. Player: centre play/pause, double-tap seek triangles, PiP (dropped)

Visible elements (OBSERVED, `MiddlePlayerControls.kt:86-116`, `DoubleTapSeekTriangles.kt`):
- Centre button: 96 dp square image, circular clip, ripple (`playerRippleConfiguration`, alpha 0.1 for pressed/focused/hovered/dragged), shown when controls are visible and not locked. Icon: `anim_play_to_pause` driven by `!paused`.
- Gesture seek indicator text when a horizontal seek gesture is active (`AYMR.strings.player_gesture_seek_indicator`, not from this area).
- Double-tap: three white 16x20 dp triangles in a row, rotated 180 deg for backward seeks, animated with the 900 ms cycle (behavior 4.5).
- Loading: a 96 dp progress circle replaces the icon (other area).

States:
- Paused: play triangle. Playing: two bars. Locked controls: the button is not shown (`areControlsLocked`).
- Double-tap: triangles fade in and out in sequence; the cycle restarts while the seek is active.

Desktop equivalent (PROPOSAL):
- Play/pause: `gtk::Button` (`.circular`) with a 96 px `Overlay` of two `Image`s (`Icon::Play`, `Icon::Pause`), crossfade 300 ms on state change. Keyboard: Space toggles play/pause (mpv default; confirm with area:45-player-core). Mouse: click toggles; double-click in video area toggles fullscreen (other area).
- Double-tap seek triangles: a `gtk::DrawingArea` or three `Image`s with `Icon::SeekTriangle`, opacity from `anim::seek_alpha`, `rotate 180deg` for backward. Shown on double-click (or the Linux equivalent gesture) at the click side, for the duration of the seek.
- PiP: not available on Linux. Its five transport icons are reused only as Play/Pause/SkipNext/SkipPrevious in the transport bar (Replace). `ic_forward_10` is available for an optional 10 s seek button (Proposal).
- Responsive: the centre button stays 96 px at all sizes; on phone widths the triangle row scales to the window (keep 16x20 aspect).

Mockup gap:
- Mockup centre play button: `media-playback-pause-symbolic` at 48 px inside a `.circular` button (`src/player.rs:203`). Android uses 96 dp; mismatch in size.
- Mockup has no double-tap seek triangles.
- Mockup flips the icon by name (`src/player.rs:206-207`): correct, keep; add the crossfade.

---

## 8. Subtitle settings: typography icons

Visible elements (OBSERVED, `SubtitleSettingsTypographyCard.kt`):
- Font family row: `outline_brand_family_24` glyph.
- Shadow row: `sharp_shadow_24` glyph (line 348).
- Both are 24 dp, tinted with the content colour.

Desktop equivalent: `Icon::FontFamily` and `Icon::TextShadow` next to the labels of the subtitle typography rows in the player settings sheet (area:48-settings-player-and-scripts). No states, no gestures.

Mockup gap: none of the subtitle typography rows exist in the mockup yet (`spec/reader_player.json` does not list them).

---

## 9. Dialogs, popups and modal surfaces

Visible elements (OBSERVED, `values/styles.xml:16-20`, `material_popup_background.xml`, `values/themes.xml:71`):
- Alert dialogs: surface `colorSurface`, corner radius `@dimen/m3_alert_dialog_corner_size` (Material library value), layout `@layout/m3_alert_dialog`.
- Popup menus: same surface with an overlay tint layer (`m3_popupmenu_overlay_color`).
- Modal scrim: `android:backgroundDimAmount` 0.32.
- Toolbar overflow icon: tinted `colorOnSurface` (`Theme.Tachiyomi.ActionButton.Overflow`).

States:
- Shown: the surface and scrim. Dismissed: removed.

Desktop equivalent (PROPOSAL):
- Dialogs: `gtk::Window` with `.dialog` styling from GTK's theme (keep the theme's radius). Do not hard-code 28 dp; the Material value is not in this repo.
- Popovers: `gtk::PopoverMenu` (theme-styled). Drop the overlay tint layer (GTK surfaces are opaque).
- Scrim: `.kb-scrim` (`rgba(0,0,0,0.32)`) on the window overlay while a modal is up.
- Keyboard: the mockup binds Escape to back navigation on the window (`src/main.rs:77-82`, `ui::pop()`). A modal dialog must consume Escape itself so it closes the dialog, not the page underneath (Proposal).
- Overflow icon: the GTK header bar menu button follows the foreground colour automatically.

Mockup gap: mockup dialogs (`src/ui.rs:406-431`) use GTK's default modal look with no scrim class; no `.kb-scrim`.

---

## 10. Preference widgets: switches, secondary text, category titles

Visible elements (OBSERVED, `values/styles.xml:50-69`, `values/themes.xml:85-86`):
- Switch preference: `Widget.Tachiyomi.Switch` (no elevation overlay; `elevationOverlayEnabled` false in light, true in night, see behavior 1.7 and the dropped bools).
- Preference secondary text (summary): `TextAppearance.Tachiyomi.Preference.Secondary` = bodySmall (12 px), secondary text colour.
- Preference category title: `preferenceCategoryTitleTextAppearance` = bodyMedium.
- Section header text: labelLarge + `textColorSecondary`.

States: enabled and disabled switch states come from the GTK switch (no Android-specific state here).

Desktop equivalent:
- Use GTK `gtk::Switch` (theme-styled). Summary label: `.kb-preference-secondary` class. Category title: `.heading` (GTK) plus `.kb-section-header` for uppercase headers (size 14 px, medium).
- Keyboard: Space toggles the switch (GTK default).
- Responsive: the settings list uses the same 16/24 px margins as elsewhere.

Mockup gap: mockup `src/settings.rs` uses GTK's default `.heading` and `.dim-label` (`src/ui.rs:125-131`). Adding `.kb-preference-secondary` makes the summary text size match Android.

---

## 11. Theme picker and AMOLED toggle (settings screen; visible effect of this area)

Visible elements (OBSERVED from `spec/settings.json:124-221` and `AppThemePreferenceWidget.kt`):
- App theme row with a horizontally scrolling list of theme preview cards (labels from AppTheme titleRes keys). MONET is hidden when dynamic colour is unavailable (`AppThemePreferenceWidget.kt:85`).
- AMOLED toggle (pref `pref_theme_dark_amoled_key`).

States:
- Selected card: highlighted. Unselected: plain.
- AMOLED: effective only in dark mode (behavior 1.4).

Desktop equivalent (PROPOSAL):
- Theme row: a `gtk::FlowBox` or horizontal `gtk::ScrolledWindow` with one `gtk::ToggleButton` per choice: Default (follows GTK theme), CottonCandy, Mocha. Show the palette's primary as a swatch (`.kb-swatch` with `background-color: #...`). Choices with no palette in this area are hidden (not greyed out), matching Android's hiding of null-titled entries.
- AMOLED: `gtk::Switch` row; disabled (insensitive) when the window is light.
- Keyboard: arrows move between cards; Enter selects.

Mockup gap: the mockup has no theme row.

---

## 12. Logo header (More tab, About)

Visible elements (OBSERVED, `LogoHeader.kt:17-40`):
- Centred `Icon` 64 dp, tinted `onSurface`, vertical padding 56 dp top and bottom (`padding(vertical = 56.dp)`), then a full-width `HorizontalDivider`.

Desktop equivalent: `gtk::Image` with `Icon::LogoSymbolic` at 64 px, `.dim-label` not applied (tinted with the window foreground), 56 px top and bottom margins, then a `gtk::Separator`. Keyboard/mouse: none.

Mockup gap: spec `app.json:729` describes the logo header, not yet built.

---

## 13. Launcher shortcuts (desktop actions)

Visible elements (OBSERVED, `app/shortcuts.xml`): five entries with icons and labels (behavior section 5).

Desktop equivalent: `.desktop` `[Desktop Action]` groups (rust design section 7). They appear in the desktop launcher's right-click menu (KDE Plasma shows them in the application menu). No in-app UI.

Mockup gap: no `.desktop` file and no `--action=` handling.

---

## 14. Notifications (desktop)

Visible elements (OBSERVED): every notifier uses a small icon from this area, and the notification accent colour `accent_blue` (`#54759E`). GNotification has no accent colour.

Desktop equivalent:
- Use `gio::Notification::set_icon(&gio::ThemedIcon::new(name))` with the standard names from the inventory (`dialog-warning-symbolic`, `object-select-symbolic`, `view-refresh-symbolic`, `folder-symbolic`, `application-x-addon-symbolic`, `network-server-symbolic`, `software-update-available-symbolic`, `image-x-generic-symbolic`, `accessories-dictionary-symbolic`, `dialog-information-symbolic`, `window-close-symbolic`, `document-send-symbolic`, `media-playback-start-symbolic`, `media-playback-pause-symbolic`).
- The app notifications use `io.github.anas1412.Kurobayomi` (`Icon::Logo`) as the default icon, matching `ic_ani` in Android.
- Accent colour: not applicable; the desktop environment decides.

Mockup gap: the mockup does not send notifications yet.

---

## 15. Responsive summary

| Element | Phone (under 720 px) | Tablet/wide (720 px and up) |
|---|---|---|
| Screen side margin | 16 px | 24 px (`screen_edge_margin`) |
| Nav | bottom bar, icon + label | sidebar, icon + label |
| Splash logo | 108 px | 108 px |
| Centre play button | 96 px | 96 px |
| Double-tap triangles | 16x20 px each | 16x20 px each |
| Cover placeholder icon | cover width / 3 | cover width / 3 |
| Reader crop/reading-mode dialogs | dialog | popover |
| Widget radii | (not applicable) | (not applicable) |

Source for the breakpoints: `values/dimens.xml:2` and `values-sw720dp/dimens.xml:2`.

---

## 16. What the mockup lacks (consolidated)

1. No splash page or ready gating (section 1).
2. Tab icons are generic and not per tab (section 2); no selected-state animation.
3. No caret on the entry header (section 3).
4. Cover placeholder colour and class (section 4).
5. No download indicator, no overflow icon on download rows, no drag highlight, no keyboard reorder (section 5).
6. Crop button uses the wrong icon (`edit-cut-symbolic`); reading-mode dialog has no icons (section 6).
7. Centre play button size and no double-tap triangles (section 7).
8. Subtitle typography icons (section 8; the rows themselves are missing).
9. No scrim, no palette/theme row, no AMOLED switch (sections 9 and 11).
10. No preference summary text class (section 10).
11. No logo header (section 12).
12. No desktop actions (section 13) and no notification icons (section 14).
13. No CSS provider at all (`grep -rn CssProvider src` is empty).
