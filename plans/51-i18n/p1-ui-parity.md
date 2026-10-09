# 51-i18n: UI parity

This area renders one screen of its own (the language picker) and provides the text, direction and date rules for every other screen. The rest of the UI is owned by the UI and settings areas; this file records only what i18n changes there.

## 1. Where the i18n UI lives in Aniyomi (OBSERVED)

| Element | Source | Key |
|---|---|---|
| Settings row "App language" | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt:116-121`: `Preference.PreferenceItem.TextPreference(title = …, onClick = { navigator.push(AppLanguageScreen()) })` | `pref_app_language` ("App language") |
| Row group heading | same file, `PreferenceGroup(title = …)` | `pref_category_display` ("Display") |
| Row has no subtitle | same file | none |
| Language picker screen | `app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt` | title `pref_app_language` |
| Back button | `AppBar(navigateUp = navigator::pop)` | `action_bar_up_description` ("Navigate up") |
| "Default" row | `getLangs()` inserts `Language("", label_default, null)` | `label_default` ("Default") |
| Locale rows | generated `locales_config.xml` | display name from `LocaleHelper` |
| Date format row | `SettingsAppearanceScreen.kt` (`pref_date_format`, "Date format") | options show `"Default (<date>)"` where `label_default` is used for the empty value |
| Relative timestamps row | same file | `pref_relative_format` ("Relative timestamps"), subtitle `pref_relative_format_summary` with `relative_time_today` and a formatted date |

## 2. Language picker: elements, states, interactions

### 2.1 Layout (OBSERVED)

- Scaffold with a top app bar. Title "App language". Scroll behaviour: `scrollBehavior` passed to `AppBar` (the bar collapses or changes elevation on scroll; Material 3 behaviour).
- Body: a vertical `LazyColumn` with one `ListItem` per language, no dividers, no section headers.
- Each `ListItem`:
  - headline: the display name (`Language.displayName`), for example `Deutsch`.
  - supporting text: `Language.localizedDisplayName` (the second name, in the default locale), shown only when not null. The Default row has no supporting text.
  - trailing content: a check icon (`Icons.Default.Check`, 24 dp in Material, tint `colorScheme.primary`) only on the row whose tag equals the current language.
- The whole row is clickable.

### 2.2 Ordering (OBSERVED)

1. Default row first (tag `""`, always index 0).
2. All other rows sorted by `displayName` ascending (`sortBy`).

### 2.3 States

| State | Observed behaviour | Kurobayomi |
|---|---|---|
| Loading | none (list is built synchronously in `remember { getLangs(context) }`) | build the list synchronously from the static table |
| Empty | impossible: the Default row always exists | same |
| Error | none: XML parse errors are not caught in `getLangs` | none needed (the table is compiled in) |
| Selected | check icon on the current row | same, see 2.5 |
| Unknown stored tag | no row shows the check | show Default as the checked row if the preference is not in the list (behaviour correction) |
| Changing | selection is applied immediately (`LaunchedEffect` calls `setApplicationLocales`) and the Android activities recreate | selection writes `app_language`; the UI shows the restart note (section 2.6) |

### 2.4 Interactions

- Tap or click a row: selects it. There is no confirmation dialog and no snackbar.
- Back (navigate up): pops the screen. The selection is not reverted.
- Android-only: none. There is no swipe gesture on this screen.

### 2.5 Desktop equivalent (PROPOSAL)

- Use `gtk::ListBox` with `selection_mode(SelectionMode::Single)` and `activate-on-single-click` (the default for `gtk::ListBox` rows), inside a `gtk::ScrolledWindow`.
- Each row is a `gtk::ListBoxRow` containing a horizontal `gtk::Box`:
  - left: vertical `gtk::Box` with `gtk::Label` (headline, `css_classes: ["heading"]` is optional) and a dimmed `gtk::Label` (supporting text, `css_classes: ["dim-label"]`).
  - right: `gtk::Image::from_icon_name("object-select-symbolic")` visible only on the current row. The icon name is a standard freedesktop icon, so it follows the user's icon theme and is not a Material glyph.
- Keyboard: Up/Down move the focus (default `ListBox` behaviour); Enter or Space activate the row (`row-activated`); Escape goes back (the window-level back shortcut owned by area:36).
- Mouse: single click activates (`row-activated`). Hover highlight comes from the GTK theme.
- Accessibility: the row accessible label is `"<headline>, <supporting>"`. The check is announced through the row's `accessible-role` `ListBoxRow` selected state (GTK sets this when the row is selected).
- Scrolling: the whole list fits the screen height on desktop (about 67 rows), so the scrolled window needs no special handling.
- Width: constrain the list to a readable width with the mockup's existing `clamp` helper (`src/clamp.rs`), not a full-window stretch.

### 2.6 Restart note (PROPOSAL, text from the catalog)

Android applies the new language immediately. GTK cannot re-translate built widgets, so Kurobayomi applies the language on the next start. The picker shows the existing message `requires_app_restart` ("Requires app restart to take effect") as an inline label under the list, revealed after the first change. Implement with a `gtk::Revealer` and a `gtk::Label`. Do not use `AdwToastOverlay` (no libadwaita in this project). The label stays until the screen is left.

Alternative considered: rebuild the main window immediately with the new language. Rejected for the first release (every label in the app would need to re-read its key; the mockup builds widgets once).

### 2.7 Settings row (PROPOSAL)

- Title: `pref_app_language`.
- Subtitle: the native name of the saved language, or `label_default` when unset. Aniyomi shows no subtitle; the subtitle is added because the desktop row has room and the current value is otherwise hidden.
- Activation: opens the picker screen.

## 3. Text direction behaviour in the UI

- RTL languages: `ar`, `fa`, `he`, `ur`. In RTL the whole UI mirrors (navigation sidebar on the right, leading icons on the right, text right-aligned). GTK mirrors widgets automatically once the default direction is RTL.
- Icons whose meaning depends on direction (back arrow, forward arrow, chevrons, progress bars) need a check in each UI area. GTK mirrors some symbolic icons automatically in RTL when the icon theme provides a direction variant (for example `go-previous-symbolic`). Icons without such a variant must be listed in the UI area's task and flipped explicitly. This area does not own the icon list.
- Settings labels that describe swipe direction keep their Aniyomi wording and meaning: the swipe-to-right setting is labelled "Chapter on swipe to right" in LTR and sits on the left in RTL (behaviour §9.4 and topic locale §7).

## 4. Date and number presentation in the UI

- The Appearance page shows a live preview of every date-format option: `"Default (<formatted>)"`, `"MM/dd/yy (<formatted>)"` etc. Desktop: compute once per screen open, not per frame.
- Relative timestamp row subtitle: `"Today" instead of "<date>"`, with `relative_time_today` and the formatted current date (topic locale §8).
- Chapter and entry dates in lists use the rules from behaviour §6.2. Those rules belong to the UI areas that show dates; this area provides the function.

## 5. What the current mockup lacks

Mockup state (OBSERVED in `/home/blackbox/kurobayomi/src/*.rs`): no i18n module; about 756 quoted string literals; English text in `spec/*.json` (`title` and `subtitle` fields); no language row, no picker, no restart note, no RTL handling, no plural forms, no date format preview.

| Missing | Required by | Where |
|---|---|---|
| `src/i18n/` module and generated `MsgKey` | every screen | new |
| Language row in the Appearance page | section 2.7 | `spec/settings.json` (Appearance group) and `src/settings.rs` |
| Language picker screen | section 2.5 | new screen, `src/screens.rs` pattern |
| Restart note | section 2.6 | picker screen |
| RTL default direction | section 3 | `src/main.rs` `build_ui` and `app.connect_startup` |
| Plural-aware labels (for example "N new chapters") | behaviour §2 | entry and update screens |
| Date-format preview list | section 4 | `src/settings.rs` |
| Key-based spec text | topic rust design §11 | `spec/*.json`, `src/spec.rs` |
| Language display names | topic locale §3 | `src/i18n/names.rs` |

## 6. Visual rules (no change from Aniyomi's meaning)

- No colour or typography is defined by this area. The theme and design areas own colours. The picker uses only theme-provided classes (`heading`, `dim-label`) and the standard selected-row style.
- The user's desktop theme is Tokyo Night (per the user's memory notes). The picker must not hard-code colours.

## 7. Empty, loading, error and offline states for this area

None. The picker reads a static table. No network. No file read beyond the preference.
