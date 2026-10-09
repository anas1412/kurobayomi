# Area 49 (part 2): UI parity

Scope: the visible behaviour of the presentation-core components, screens and modifiers in `p2-inventory.md`, plus the theme picker that exposes the theme tokens (owned by part 1; described here because the look of every screen depends on it). Android sources are repo-relative under `/home/blackbox/aniyomi-src/`. Desktop equivalents are PROPOSALS.

Mockup reference: `/home/blackbox/kurobayomi/src/ui.rs`, `src/main.rs`, `src/settings.rs`, `src/screens.rs`, `spec/settings.json`, `spec/app.json`.

---

## 1. Visible elements, state by state

### 1.1 Theme appearance (settings: Appearance > Theme)
Source: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt` (`getThemeGroup`, lines 50 to 99), `app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemeModePreferenceWidget.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt`.

Elements, top to bottom:
1. Category title `pref_category_theme`.
2. Row title `pref_app_theme`, containing two sub-widgets in one column:
   a. Theme mode row (`AppThemeModePreferenceWidget`): three choices `theme_system`, `theme_light`, `theme_dark`. Selecting one saves `pref_theme_mode_key` and calls `setAppCompatDelegateThemeMode` at once (no restart).
   b. Theme card row (`AppThemePreferenceWidget`): a horizontal `LazyRow`, horizontal padding `PrefsHorizontalPadding`, item spacing 8 dp (`MaterialTheme.padding.small`). One item per `AppTheme.entries` whose `titleRes != null`, and `MONET` only if `DeviceUtil.isDynamicColorAvailable`. Items are keyed by `AppTheme.name`.
3. Each card:
   - Column width 114 dp, top padding 8 dp.
   - Preview: `Column` with `fillMaxWidth().aspectRatio(9f / 16f)`; border 4 dp (`primary` colour when selected, `DividerDefaults.color` otherwise); outer corner radius 17 dp; inner padding 4 dp; clip radius 13 dp; background `background` of that theme (the preview is wrapped in `TachiyomiTheme(appTheme = appTheme, amoled = amoled)`, so each card shows its own palette and the AMOLED flag).
   - Preview contents, top to bottom: app bar row (height 40 dp, padding 8 dp): a bar of weight 0.7 with `fillMaxHeight(0.8f)`, end padding 4 dp, `onSurface` colour, small corner radius; and a 0.3-weight box that shows `Icons.Filled.CheckCircle` (tint `primary`, content description `MR.strings.selected`) only when selected. Then a cover placeholder: `DividerDefaults.color`, `fillMaxWidth(0.5f)`, aspect ratio `ItemCover.Book.ratio`, start padding 8 dp, top 2 dp. Then a 24 x 16 dp row of two 12 dp boxes (cover text lines). Then a bottom bar (fills, weight 1) with a `surfaceContainer` surface, height 32 dp, horizontal padding 8 dp: a 17 dp `primary` circle, then a 17 dp tall `onSurface` pill at alpha 0.6 with start padding 8 dp and weight 1.
   - Label below: `Spacer` 8 dp, then the title text (`stringResource(titleRes)`), `bodyMedium`, centred, `maxLines = 2`, `minLines = 2`, alpha 0.78.
   - Click: `onItemClick(appTheme)` then `ActivityCompat.recreate(activity)` (full UI recreation at once).
4. Pure-black switch (`pref_dark_theme_pure_black`), disabled when mode is Light; toggling recreates the activity.

State matrix for the theme row:
| State | Look |
|---|---|
| Selected card | primary border, check icon in the app bar of the mini preview |
| Unselected card | divider-coloured border, no icon |
| Dynamic card absent | not rendered at all when the device lacks dynamic colour |
| Mode = Light | pure-black switch greyed out |

### 1.2 Scaffold (every screen with a bottom bar, FAB or snackbar)
Source: `Scaffold.kt`.
- Content fills the screen; its padding is top = top bar height (or inset), bottom = max(bottom bar height, FAB offset) (or inset), start = max(inset, start bar width).
- A floating action button (FAB) sits 16 dp above the bottom bar (or above the bottom inset), at the end edge (16 dp from the right in LTR), width/height from its content.
- The snackbar host is centred horizontally on the content width and sits above the FAB when a FAB is present (`snackbarOffsetFromBottom = snackbarHeight + fabOffset`), otherwise above the bottom bar.
- Z-order: content < start bar < top bar < snackbar < bottom bar < FAB.
- Top bar height may exceed the layout height (infinite max height constraint), used for expanded app bars.

Desktop (PROPOSAL): `gtk::Overlay` with the content page as the main child; the FAB is a `gtk::Button` with `halign = end`, `valign = end`, `margin-end = 16`, `margin-bottom = 16 + bottom bar height`; the snackbar is a `gtk::Revealer` (`transition-type = slide-up`) at bottom centre with the same offset rule. The FAB is not used on desktop unless a screen needs it (Aniyomi uses it for "Start reading" and similar; keep it where the mockup has it).

### 1.3 FAB expansion
Source: `presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt`.
- FAB shows its label (extended) when: first visible item index is 0 and its scroll offset is 0, OR the last scroll direction was backward, OR the list cannot scroll forward.
- Otherwise it collapses to the icon only.

Desktop: the same rule over a `gtk::ScrolledWindow` vadjustment: expanded when `value <= 0`, or when the value decreased since the last change, or when `value + page_size >= upper`. Collapse transition: width animation 120 ms (proposal, matches the hover timing in the behavior file).

### 1.4 Slider
Source: `Slider.kt`. Integer values only, snapped to integers.
- Thumb moves in whole steps; the value label (if shown) is the integer.
- Keyboard: arrow keys move by 1 (M3 default behaviour for discrete sliders). Desktop: `gtk::Scale` with step 1, `draw-value` shown as the integer, `round-digits = 0`.
- `onValueChangeFinished` fires on release (mouse up or key up after change).

Usage: reader and player settings (`ChapterNavigator.kt`, `SliderItem.kt`, `TintedSliderItem.kt`, `SettingsItems.kt`).

### 1.5 Surface (clickable container)
Source: `Surface.kt`.
- Minimum touch target 48 dp (visual size unchanged).
- Ripple on press (`ripple()` indication) and long-press callback.
- Elevated surfaces (tonal elevation > 0) get the tinted background (see behavior file section 3.2). Hover and press change the elevation via `animateElevation` (120 ms in, 150 ms out).

Desktop: hover and press states from the CSS (`.kb-surface:hover`, `:active`). A long press uses the GTK `GestureLongPress` default delay, or right click when the surface opts in (proposal).

### 1.6 Tab with count pill
Source: `Tabs.kt`.
- Label `bodyMedium`-style text, one line, ellipsis when too long.
- If a count is given, a pill with the number in 10 sp, background `onBackground` at 12% (dark) or 8% (light), padding 6 dp horizontal and 1 dp vertical, start margin 4 dp.
- Count `0` is still shown (the pill is shown when the count is non-null).

Mockup gap: the mockup tab strip is a `gtk::Stack` switcher in `src/ui.rs` with no count pills. Add `kb-pill` in the tab label builder.

### 1.7 Empty screen
Source: `EmptyScreen.kt`.
- A large kaomoji face (display medium, 45 sp in M3 defaults, faint at alpha 0.78), then the message (body medium, centred, 24 dp below the face), then an optional row of equal-width buttons (24 dp below the message, 8 dp gaps).
- The face is random per screen instance (12 choices, see behavior file section 9.1).
- The column scrolls if the content does not fit.
- Horizontal padding 24 dp.

Desktop: a centred `gtk::Box` (vertical) with the face in a `kb-display` label (font size large, same faint alpha), the message label, and a horizontal `gtk::Box` of `gtk::Button`s. The face uses a font that has the glyphs (`Noto Sans Symbols 2` or the system fallback). Add a fallback to ASCII `:(` when the face glyphs are missing (checked once at startup with Pango).

Mockup gap: the mockup uses `status(icon, text)` (`src/ui.rs` line 166) with a 96 px symbolic icon. Replace with the face layout for the "no results" and empty-library cases.

### 1.8 Info screen (full page with bottom buttons)
Source: `InfoScreen.kt`.
- Top: 48 dp gap, 48 dp icon in `primary`, headline (`headlineLarge`), subtitle (`titleSmall`, alpha 0.78, 8 dp vertical padding), then the content.
- Bottom bar: a hairline divider on the top edge (colour `outline`), a filled accept button (full width, disabled when `canAccept` is false), and an outlined reject button when reject text and handler are given.
- The bottom bar sits above the navigation bar insets (not applicable on desktop).

Desktop: a `gtk::Box` page with the same vertical order, the bottom button row as a `gtk::Box` with `homogeneous = true`, hairline via CSS `border-top: 1px solid @borders`.

### 1.9 Loading screen
Source: `LoadingScreen.kt`. Centred `CircularProgressIndicator` filling the screen. No text, no timeout, no cancel.

Desktop: centred `gtk::Spinner` with `spinning = true`, filling the page. Show after 300 ms (proposal) to avoid flashing on fast loads; the Android version shows immediately.

### 1.10 Pull to refresh
Source: `PullRefresh.kt`. Gesture plus a spinner at the top centre, indicator background `surfaceVariant`, indicator colour `onSurfaceVariant`.

Desktop (PROPOSAL, T-49b-019): a refresh button in the page header (icon `view-refresh-symbolic`, tooltip "Refresh"), F5 and Ctrl+R accelerators, spinner in place of the icon while refreshing, button disabled while refreshing. No overscroll gesture.

### 1.11 Navigation rail / bar
Source: `NavigationRail.kt`, `NavigationBar.kt`. Not used by any screen (no call sites). The desktop sidebar in the mockup is the navigation. Keep the sidebar's selected-row look; the rail's min width 80 px is a reference only.

### 1.12 Selected background
Source: `Modifier.kt` `selectedBackground`. A full-row tint behind the row: `secondary` at 16% alpha (dark) or 22% (light). Used on 8 call sites (list rows in library, browse, updates and selection mode).

Desktop: `.kb-selected` class on the `ListBoxRow`, background colour computed in `css.rs`. The GTK `:selected` state is not used because it looks different per theme.

### 1.13 Active filter tint
Source: `theme/Color.kt`. The filter icon in the toolbar uses `active` (`#FFEB3B` dark, `#FFC107` light) when filters are on, otherwise the normal content colour. Usage: `EntryToolbar.kt` (line 80), `ChapterSettingsDialog.kt` (line 182), `LibraryToolbar.kt`.

Desktop: `.kb-active` on the icon image in the header bar when a filter is set.

### 1.14 Section header text
Source: `theme/Typography.kt`. Semi-bold body text, colour `onSurfaceVariant`, used for list group headers (8 call sites). Desktop: `.kb-header` (weight 600, colour `onSurfaceVariant` token). The mockup uses `heading` (stock Adwaita style) for the same role; keep `heading` for stock look, or switch to `.kb-header` when a custom theme is active. Decision: keep stock `heading` when the theme is GTK theme (no override), use `.kb-header` otherwise.

### 1.15 Scrollbars
Source: `Scrollbar.kt` (dropped). Desktop: GTK overlay scrollbars (they appear on hover or scroll and fade). The Compose thumb is `onSurface` at 36.4% alpha, with the platform scroll bar thickness (`ViewConfiguration.scaledScrollBarSize`, framework value). The mockup already uses GTK scrollbars through `ui::scrolled`.

### 1.16 Cover placeholder
Source: `values/colors.xml` `cover_placeholder` `#1F888888`, used by `cover_error.xml` (area 34). A grey tint over the cover area when a cover failed to load. Desktop: `.kb-cover-error` background `rgba(136,136,136,0.12)`, same as `#1F888888` (alpha 31/255).

### 1.17 Divider
Source: `values/colors.xml` `divider_default` = `md_black_1000_12` (`#1F000000`, light) and `md_white_1000_12` (`#1FFFFFFF`, dark). Used by list dividers (`styles.xml` `dividerColor`). Desktop: `separator` CSS with the matching alpha (GTK `borders` colour at 12%).

### 1.18 Accent colour in notifications
Source: `values/colors.xml` `accent_blue` = `#54759E`, used by `NotificationExtensions.kt` line 71 (`setColor`). Desktop notifications (area 35) use the same accent via the portal notification icon colour only if supported; otherwise no colour.

---

## 2. Keyboard and mouse equivalents

| Android interaction | Desktop equivalent (PROPOSAL) |
|---|---|
| Tap | primary click |
| Long press | long press (500 ms) or secondary click when a long-press action exists |
| Pull to refresh | button plus F5, Ctrl+R |
| Hardware Enter in single-line field | `activate` (Enter, KP_Enter) via key controller in capture phase |
| Focus on open | `grab_focus()` once per dialog open |
| Soft keyboard dismiss | not applicable |
| Slider drag | drag on `gtk::Scale`; arrows move 1 step |
| FAB tap | click on the FAB; keyboard shortcut shown in its tooltip if one exists |
| Theme card tap | click; applies the theme immediately (no restart) |
| Pure black switch | switch row; greyed out in Light mode |

---

## 3. Responsive behaviour
- Compose: `isMediumWidthWindow()` (> 600 dp) and `isExpandedWidthWindow()` (> 840 dp) (`mihon/core/designsystem/utils/WindowSize.kt`).
- Desktop: the window is resizable from 600 px to any width. Use the same breakpoints on the window width (`window.width()`) through a `notify::default-width` handler: below 600 px hide the sidebar labels (icons only); between 600 and 840 px show the sidebar; above 840 px show the sidebar and a second pane where the screen supports one.
- Theme cards: the picker row scrolls horizontally on narrow windows; cards keep their 114 px width.
- Empty screen and info screen: content centred, max content width 640 px (existing `ui::clamp` in `src/clamp.rs`).

---

## 4. Mockup gaps (what `src/` and `spec/` lack for this area)

| Area | Mockup today | Needed |
|---|---|---|
| Theme colours | none (stock Adwaita classes only) | `theme::Theme` plus `CssProvider` (T-49b-006, T-49b-008) |
| Theme picker | `chips` list in `spec/settings.json` (names only) | mode row (3 options), 19 preview cards with mini mockups and check badge, pure-black switch greyed in Light, immediate apply (T-49b-014, T-49b-015) |
| Default theme | `spec/settings.json` default `Dynamic`; `spec/app.json` default `Default` | one default (decision T-49b-006) |
| Scaffold / FAB / snackbar | `gtk::Box` layouts | `gtk::Overlay` with FAB and snackbar offsets (T-49b-016) |
| Empty state | `status()` icon plus text | face plus message plus buttons (T-49b-025) |
| Loading | none specific | centred spinner, 300 ms delay (T-49b-027) |
| Info screen | none | full page with bottom buttons (T-49b-026) |
| Slider | `gtk::Scale` (presumed) | integer snapping, finished event (T-49b-020) |
| Tab count pill | none | `kb-pill` label (T-49b-022) |
| Selected row tint | stock selection | `.kb-selected` tint (T-49b-029) |
| Active filter tint | none | `.kb-active` amber (T-49b-010) |
| Refresh | none | header button plus F5 (T-49b-019) |
| Responsive breakpoints | none | 600 and 840 px handlers (T-49b-042) |

---

## 5. Strings the UI shows (keys)
Theme area: `pref_category_theme`, `pref_app_theme`, `theme_system`, `theme_light`, `theme_dark`, `pref_dark_theme_pure_black`, `selected` (check icon description), and the 19 theme titles (`label_default`, `theme_monet`, `theme_cloudflare`, `theme_cottoncandy`, `theme_doom`, `theme_greenapple`, `theme_lavender`, `theme_matrix`, `theme_midnightdusk`, `theme_mocha`, `theme_sapphire`, `theme_nord`, `theme_strawberrydaiquiri`, `theme_tako`, `theme_tealturquoise`, `theme_tidalwave`, `theme_yinyang`, `theme_yotsuba`, `theme_monochrome`). Full English values in `p2-data-and-config.md` section 3.

Scope-owned keys: none (components take their text from callers). The empty-screen face strings are fixed kaomoji, not translated.

---

## 6. Dev-only UI (not ported)
`ThemePreviews` annotation, `InfoScaffoldPreview`, `LazyListScrollbarPreview`, `LazyListHorizontalScrollbarPreview`, `AppThemesListPreview` (in part 1). Replaced by the `kurobayomi` dev gallery (proposal; one screen listing each component with each theme) and the snapshot harness (`plans/_tools/snap`).
