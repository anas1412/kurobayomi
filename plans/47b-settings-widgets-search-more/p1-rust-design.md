# Area 47b — Rust design (Kurobayomi, gtk4-rs 0.11.5, GTK 4.16 features, no libadwaita)

Context from the repo: `Cargo.toml` (edition 2024) depends on `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde`, `serde_json`. The mockup has single-file modules `src/main.rs`, `src/ui.rs` (525 lines), `src/screens.rs` (545), `src/settings.rs` (264), `src/spec.rs` (191), `src/entry.rs`, `src/player.rs`, `src/clamp.rs`, and loads `spec/app.json`, `spec/settings.json`, `spec/entries.json`, `spec/reader_player.json`. The mockup has no tokio, no rusqlite, no prefs store yet; those arrive from areas 02, 05, 12, etc. This area adds only the modules listed in §1.

## 0. Decisions at a glance
1. Settings pages are data (`settings::model::Preference` tree) rendered by one function per widget kind. No stringly-typed widgets.
2. Preference bindings are typed handles (`Pref<T>`) provided by area:02. Scope widgets never touch the store directly.
3. Dialogs are `gtk::Window` (transient, modal) built by hand so that OK/Cancel and validation rules match the Compose dialogs. `gtk::AlertDialog` is used only for the 2-button confirmations elsewhere (not in this area).
4. The highlight flash is driven by `glib::timeout_add_local` toggling a CSS class with a CSS `transition`. GTK 4 CSS has no `@keyframes`; the timing is reproduced with timers (§6).
5. Everything in this area runs on the GTK main thread. Background work (download queue, update download) is a tokio task that publishes `tokio::sync::watch` values consumed by `glib::spawn_future_local`.
6. Markdown changelog rendering uses `pulldown-cmark` to Pango markup for a `gtk::Label` (no WebView, no libadwaita).

## 1. Crate and module layout (this area's files only)

```
src/
  settings/
    mod.rs            pub use; SettingsPage registry (ids from spec/settings.json)
    model.rs          Preference, PreferenceItem, PreferenceGroup (§2)
    page.rs           build a page: header + ScrolledWindow + ListBox per group; PrefPage scaffold (replaces PreferenceScaffold/PreferenceScreen)
    render.rs         PreferenceItem -> gtk::Widget dispatch (replaces PreferenceItem.kt)
    highlight.rs      HighlightRequest handoff + flash timers (§6)
    widgets/
      mod.rs          constants (§9), shared helpers, PREF_MIN_HEIGHT
      row.rs          PrefRow base (replaces BasePreferenceWidget)
      text.rs         TextRow (replaces TextPreferenceWidget)
      switch.rs       SwitchRow
      list.rs         ListDialog single choice (replaces ListPreferenceWidget)
      multi_select.rs MultiSelectDialog (replaces MultiSelectListPreferenceWidget)
      edit_text.rs    EditTextDialog + variants (replaces EditTextPreferenceWidget)
      slider.rs       SliderRow (replaces BaseSliderItem wrapper)
      tracker.rs      TrackerRow (replaces TrackingPreferenceWidget)
      info.rs         InfoRow (replaces InfoWidget)
      group.rs        GroupHeader (replaces PreferenceGroupHeader)
      tri_state.rs    TriStateDialog (replaces TriStateListDialog)
      theme_mode.rs   ThemeModeRow (replaces AppThemeModePreferenceWidget)
      app_theme.rs    AppThemeStrip (replaces AppThemePreferenceWidget)
  more/
    mod.rs            MoreTab nav entry + page builder (replaces MoreTab.kt, MoreScreen.kt, LogoHeader.kt)
    queue_state.rs    DownloadQueueState + combiner (replaces MoreScreenModel)
    update_dialog.rs  UpdateDialog view + controller (replaces NewUpdateScreen x2)
  onboarding/
    mod.rs            back-blocking gate + finish + restore-highlight handoff (replaces ui/more/OnboardingScreen.kt)
```

Note: `src/settings.rs` (mockup, 264 lines) becomes `src/settings/mod.rs` with the mockup's `body(id)` router kept as-is for the non-scope pages (area:47). Moving the file is a refactor the first task (T-47ba-002) performs; no behaviour changes.

## 2. Core types (`src/settings/model.rs`)

```rust
use std::sync::Arc;

/// Top-level entry of a settings page: a standalone item or a titled group.
pub enum Preference {
    Item(PreferenceItem),
    Group(PreferenceGroup),
}

pub struct PreferenceGroup {
    pub title: String,
    pub enabled: bool,               // default true; disabled groups are not rendered (behavior A.3.3)
    pub items: Vec<PreferenceItem>,
}

/// Common fields of every item (Preference.kt: title, enabled, subtitle, icon, onValueChanged).
pub struct ItemCommon {
    pub title: String,               // TrackerPreference uses ""
    pub enabled: bool,               // default true
    pub subtitle: Option<String>,    // default Some("%s") for bound lists/edit fields, None for Text/Info
    pub icon: Option<&'static str>,  // GTK symbolic icon name (see data-and-config §8); None = no icon
}

pub enum PreferenceItem {
    Text    { common: ItemCommon, on_click: Option<Rc<dyn Fn()>> },
    Switch  { common: ItemCommon, pref: Pref<bool>, veto: Veto<bool> },
    Slider  { common: ItemCommon, value: i32, range: std::ops::RangeInclusive<i32>, steps: u32, on_change: Veto<i32> },
    List    { common: ItemCommon, pref: Pref<String>, entries: Vec<(String, String)>, veto: Veto<String> },
    BasicList { common: ItemCommon, value: String, entries: Vec<(String, String)>, on_change: Veto<String> },
    MultiSelect { common: ItemCommon, pref: Pref<BTreeSet<String>>, entries: Vec<(String, String)>, veto: Veto<BTreeSet<String>> },
    EditText { common: ItemCommon, pref: Pref<String>, dialog_subtitle: Option<String>, can_be_blank: bool, single_line: bool,
               format_subtitle: bool, validate: Option<Rc<dyn Fn(&str) -> bool>>, error_message: Option<Rc<dyn Fn(&str) -> String>>,
               input_purpose: gtk::InputPurpose, veto: Veto<String> },
    MpvConf { common: ItemCommon, pref: Pref<String>, file_name: Option<String>, can_be_blank: bool, veto: Veto<String> },
    Tracker { tracker: TrackerHandle, on_login: Rc<dyn Fn()>, on_logout: Rc<dyn Fn()> },
    Info    { title: String, enabled: bool },
    Custom  { title: String, build: Rc<dyn Fn() -> gtk::Widget> },
}

/// Synchronous or async veto. Returning false cancels the write (PreferenceItem.kt semantics).
pub type Veto<T> = Rc<dyn Fn(T) -> VetoFuture>;
pub type VetoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = bool>>>;
```

Notes:
- `Rc` and `!Send` types: settings widgets are GTK main-thread only. `Pref<T>` (area:02) is `Send + Sync` internally; the `Rc` only wraps closures.
- `subtitle` default for lists and edit fields is `Some("%s".into())`; render replaces `%s` with the formatted value (`Preference.kt` default). Rust helper: `fn fmt_subtitle(tpl: &str, arg: &str) -> String { tpl.replacen("%s", arg, 1) }`. Kotlin `String.format` with one `%s` is equivalent for these templates. Behavior ported: `subtitle?.format(value)`.
- The `ListPreference` generic `T` becomes `String` in Rust because every bound list in scope stores an enum name or a numeric string. Enum-typed lists convert with `ToString`/`FromStr` at the binding (area:02 provides `Pref<E: EnumName>`).
- Slider has no `Pref`; the caller owns `value` (behavior B.7).
- `steps` for sliders is `(last - first) - 1` as an unsigned count, clamped to 0 (`IntRange(from = 0)`).

### 2.1 Disabled item behaviour
- Groups with `enabled == false` are removed from the tree at build time (not rendered), matching A.3.3.
- Items with `enabled == false` are rendered but `Revealer::set_reveal_child(false)` (slide 150 ms) and `set_sensitive(false)`. Keep the row in the `ListBox` so it stays findable by title for the highlight lookup (§6.2).

## 3. Widget builders (`src/settings/widgets/*`)

All builders are `fn(&ItemCommon, ...) -> gtk::ListBoxRow` (rows) or `fn(...) -> adw-free gtk::Window` (dialogs). Example for the base row:

```rust
// src/settings/widgets/row.rs  (signature only; layout is specified in behavior §B.1)
pub struct PrefRow {
    pub row: gtk::ListBoxRow,
    pub title: gtk::Label,
    pub subtitle: gtk::Label,
    pub trailing: gtk::Box,   // empty box when no trailing widget
}

/// H-box: [icon, start margin 16, end margin 8] [v-box: title, subtitle] [trailing box, end margin 16]
/// Row min height PREF_MIN_HEIGHT (56). Title: ellipsize END, wrap, max 2 lines, css class "pref-title" (16px).
/// Subtitle: css class "dim-label" with opacity SECONDARY_ALPHA (0.78), max 10 lines.
pub fn build(common: &ItemCommon, trailing: Option<&gtk::Widget>, activatable: bool) -> PrefRow;
```

### 3.1 Dialog pattern
All OK/Cancel dialogs share one helper so that button order, enable rules and Escape handling are identical:

```rust
// src/settings/widgets/mod.rs
pub struct DialogSpec<'a> {
    pub parent: &'a gtk::Window,
    pub title: &'a str,
    pub body: &'a gtk::Widget,
    pub confirm_label: Option<&'a str>,  // None for list dialog (Cancel only)
    pub confirm_enabled: Rc<dyn Fn() -> bool>,
    pub on_confirm: Rc<dyn Fn() -> VetoFuture>,
    pub on_cancel: Rc<dyn Fn()>,
}
/// Transient, modal window for `spec.parent`; default width 360 px, not resizable (behavior §B.6, §B.11).
pub fn show_dialog(spec: DialogSpec) -> gtk::Window;
```

Rules:
- Escape and window close request map to Cancel (`on_cancel`).
- The confirm button is disabled by `confirm_enabled()` re-evaluated on every change signal (behavior B.6 rule).
- `on_confirm` runs the async veto on `glib::spawn_future_local`; the window closes only if it returns true (behavior A.5, B.6).
- Dialog width: GTK does not use `usePlatformDefaultWidth`; Proposal: `set_default_width(360)` and `set_resizable(false)`, matching Material's default alert width band.

### 3.2 Row kinds map
| Kotlin widget | Rust builder | Trailing | Activation |
|---|---|---|---|
| TextPreferenceWidget | `text::build` | optional | `on_click` if Some |
| SwitchPreferenceWidget | `switch::build` | `gtk::Switch` (`set_can_target(false)`, `set_active(checked)`) | whole row toggles |
| ListPreferenceWidget | `list::build` | none | opens `list::dialog` |
| MultiSelectListPreferenceWidget | `multi_select::build` | none | opens dialog |
| EditTextPreferenceWidget | `edit_text::build` | none | opens dialog |
| BaseSliderItem | `slider::build` | `gtk::Scale` inline, `set_draw_value(false)`, `Pill` label as `gtk::Label` with css `pill` | none; scale changes call `on_change` |
| TrackingPreferenceWidget | `tracker::build` | check icon when logged in | login/logout |
| InfoWidget | `info::build` | none | none |
| PreferenceGroupHeader | `group::build` | none | none |
| CustomPreference | caller's widget | | |

Inline slider vs dialog: Aniyomi uses an inline Compose `Slider` with a value pill. GTK `gtk::Scale` supports the same inline layout (`set_draw_value(false)`, pill label beside it). Decision recorded here.

## 4. Preference binding and threading

### 4.1 `Pref<T>` (provided by area:02; this area consumes it)
Expected surface (signature proposal, to be agreed in area:02):

```rust
pub struct Pref<T> { /* key, default, store handle */ }
impl<T: Clone + Send + Sync + 'static> Pref<T> {
    pub fn get(&self) -> T;                          // synchronous read from in-memory cache
    pub fn set(&self, value: T);                     // write-through, persists on the store's writer task
    pub fn watch(&self) -> tokio::sync::watch::Receiver<T>;  // changes from any writer
}
```

### 4.2 Threads
- GTK main thread: all widget creation, dialog handling, veto execution (`glib::spawn_future_local`).
- Tokio runtime (created once in `main`, area:36): preference persistence, download-queue producers, update download.
- Handoff: `Pref::watch()` receivers are polled from the GTK thread with `glib::spawn_future_local(async move { while rx.changed().await.is_ok() { ... } })`. `tokio::sync::watch` works from a non-tokio executor because `changed()` is runtime-agnostic.
- Vetoes that do I/O (e.g. MPV file write, §8) run as `tokio::task::spawn_blocking` from the veto future; the GTK thread awaits the join handle through `glib::spawn_future_local`.

### 4.3 Veto runner
```rust
pub fn run_veto(fut: VetoFuture, on_ok: impl FnOnce() + 'static) {
    glib::spawn_future_local(async move {
        if fut.await { on_ok(); }
    });
}
```
Failure path: if the future panics, the `glib` task aborts and logs; Proposal (behavior A.5 gap): wrap in `std::panic::AssertUnwindSafe(...).catch_unwind()` and show an error label in the dialog ("Could not save this setting") if an app string is added in area:51.

## 5. Page scaffold (`src/settings/page.rs`)
- `build(screen: &SettingsScreen) -> gtk::Widget` returns: `gtk::Box` (vertical) with a `gtk::HeaderBar` (title, back button if `can_pop`, actions from `SearchableSettings.AppBarAction` equivalents) and a `gtk::ScrolledWindow` (`propagate_natural_height(false)`, `hscrollbar_policy(Never)`).
- Body: one `gtk::ListBox` per group (css class `boxed-list`), with the group header label above it; standalone items live in an unnamed `ListBox`. `set_selection_mode(None)`.
- Group spacing: 12 px spacer `gtk::Box` between groups, omitted after the last group (behavior A.3.3).
- Header bar: the app's shared header (area:36). This area does not define the window chrome.

## 6. Highlight flash (`src/settings/highlight.rs`)

### 6.1 Handoff (replaces the `SearchableSettings.highlightKey` global)
```rust
thread_local! {
    static PENDING: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}
pub fn request(title: impl Into<String>) { PENDING.with(|p| *p.borrow_mut() = Some(title.into())); }
pub fn take() -> Option<String> { PENDING.with(|p| p.borrow_mut().take()) }
```
- Writers: onboarding restore (§13), settings search (area:47).
- Reader: `page::build` calls `take()` once when the page is built, and keeps the value for the scroll step. `take()` clears the request even when no row matches (behavior A.3.2).

### 6.2 Target lookup and scroll (replaces Kotlin index arithmetic)
Kotlin scrolls to a LazyColumn slot index computed by `findHighlightedIndex` (behavior A.4). Rust does not use a flat index. During page build, every `ItemCommon` with a non-empty title that renders a row is registered in `HashMap<String, gtk::ListBoxRow>` (first registration wins, matching `indexOfFirst`). Disabled groups are not built, so they cannot be targets; disabled items are built but insensitive, and they can still be found, matching Kotlin.

```rust
/// Called once, 500 ms after page build (behavior A.3.2, C.3). Scrolls the page to `row` and starts the flash.
pub fn reveal(scrolled: &gtk::ScrolledWindow, row: gtk::ListBoxRow) {
    glib::timeout_add_local_once(Duration::from_millis(HIGHLIGHT_SCROLL_DELAY_MS), move || {
        if let Some(p) = row.compute_point(scrolled, &gtk::graphene::Point::new(0.0, 0.0)) {
            let vadj = scrolled.vadjustment();
            vadj.set_value(vadj.value() + f64::from(p.y()));   // Proposal: animate 250 ms ease-out
        }
        flash(&row);
    });
}
```
- The flash starts when the target is revealed, which matches Kotlin, where the row flashes after it scrolls into view.

### 6.3 Edge: a title appearing twice
- Kotlin highlights every row whose title equals the key (A.5). Rust registers only the first row in the lookup map, so one row flashes. This is a deliberate fix (E-02). Scroll target equals the first row, same as Kotlin's `indexOfFirst`.

### 6.4 Flash timeline (behavior C.2)
Implemented with a CSS class and timers, because GTK4 CSS cannot run `@keyframes`:

```
t = 0 ms      add css class "pref-flash"       (background: alpha(@accent_bg_color, 0.12); transition: background-color 200ms)
t = 600 ms    pulse: toggle class "pref-flash-off" every 200 ms, FLASH_TOGGLES = 10 toggles
t = 3000 ms   remove "pref-flash" and "pref-flash-off"; colour fades back over 200 ms
```

```rust
pub fn flash(row: &gtk::ListBoxRow) {
    row.add_css_class("pref-flash");
    let r = row.clone();
    glib::timeout_add_local_once(Duration::from_millis(FLASH_DELAY_MS), move || {
        let toggles = Rc::new(Cell::new(0u32));
        let r2 = r.clone();
        glib::timeout_add_local(Duration::from_millis(FLASH_STEP_MS), move || {
            if r2.has_css_class("pref-flash-off") { r2.remove_css_class("pref-flash-off"); }
            else { r2.add_css_class("pref-flash-off"); }
            toggles.set(toggles.get() + 1);
            if toggles.get() >= FLASH_TOGGLES { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
        });
    });
    let r3 = row.clone();
    glib::timeout_add_local_once(Duration::from_millis(FLASH_TOTAL_MS), move || {
        r3.remove_css_class("pref-flash");
        r3.remove_css_class("pref-flash-off");
    });
}
```
- CSS (`resources/style.css`, loaded by area:49): `.pref-flash { background-color: alpha(@accent_bg_color, 0.12); transition: background-color 200ms ease; }` followed by `.pref-flash-off { background-color: transparent; }`. The later rule wins at equal specificity.
- Colour: Compose `surfaceTint` at alpha 0.12 maps to `alpha(@accent_bg_color, 0.12)` (theme-following, area:49).
- Constants: `FLASH_DELAY_MS = 600`, `FLASH_STEP_MS = 200`, `FLASH_TOGGLES = 10`, `FLASH_TOTAL_MS = 3000`.
- Timing vs Kotlin: Kotlin's 5 `Reverse` iterations of 200 ms produce 10 half-steps; `FLASH_TOGGLES = 10` matches. The Kotlin flag ends at 3 s, `FLASH_TOTAL_MS` matches.
- Re-entrancy: a second `flash()` on the same row while the first is running must cancel the first timers (pitfall P-flash-idempotent, §14).

## 7. Theme pickers and colour scheme (`src/settings/widgets/theme_mode.rs`, `app_theme.rs`)
- Theme mode: a `gtk::Box` with css class `linked` containing three `gtk::ToggleButton` set to one group (`set_group`). Labels `System`, `Light`, `Dark` (exact English keys from area:51). Click sets `UiPrefs.theme_mode` (`Pref<ThemeMode>`, key `pref_theme_mode_key`, default `System`). Application: `gtk::Settings::default().set_gtk_application_prefer_dark_theme(Some(true)/Some(false)/None)`. The "System" value passes `None` so the desktop decides (KDE Plasma colour scheme via the GTK theme).
- App theme strip: a horizontal `gtk::ScrolledWindow` (`vscrollbar_policy(Never)`) with a `gtk::Box(Horizontal, 8)` of cards. Card = `gtk::Button` (css class `flat`) with a `gtk::DrawingArea` mini preview sized 114 px wide and 9:16 frame, plus a 2-line label. Selected card gets css class `selected` (border 4 px, primary colour).
- Theme list = `AppTheme::ALL` minus deprecated (`DARK_BLUE`, `HOT_PINK`, `BLUE`) minus `MONET` on Linux. 18 cards (behavior T.2).
- Applying: `theme::apply(theme, amoled)` (area:49) swaps a `gtk::CssProvider` registered with `gtk::style_context_add_provider_for_display` at `PRIORITY_USER`. No restart. Preview cards are NOT real themes: each card paints its own palette from the constants in area:49 `theme::palette(AppTheme)`.
- Amoled flag: `Pref<bool>` key `pref_theme_dark_amoled_key`, default false; applies only in dark mode.

## 8. More tab (`src/more/*`)

### 8.1 Entry
- Sidebar entry declared in `spec/app.json` `nav` (order and icon are data). Tab index 4 in Compose has no Linux meaning.
- `open("more")` (mockup `src/ui.rs`) builds the page with `more::page::build()`.

### 8.2 Page
- Header: `LogoHeader` as a `gtk::Box` with `gtk::Image` 64 px (`set_pixel_size(64)`), margins top and bottom 56 px, then `gtk::Separator`.
- Rows, in the exact order of behavior §M.2: two switches (`Downloaded only`, `Incognito mode`), separator, alt-nav row, download queue row, Categories, Statistics, Data and storage, separator, Settings, Player settings, About, Help.
- Mockup difference: the mockup removes the History link (`src/screens.rs:344-347`). Proposal: keep the removal only when the active nav style is `MOVE_HISTORY_TO_MORE` false. Rust rule: show the alt-nav row when `nav_style` moves a tab (always true in Kotlin); mockup can hide History only when History is a sidebar entry. Both values are in data.
- Help row: `gtk::UriLauncher::new("https://aniyomi.org/docs/guides/troubleshooting/").launch(...)` (Constants.URL_HELP).

### 8.3 Queue state (`src/more/queue_state.rs`)
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadQueueState {
    Stopped,
    Paused { pending: usize },
    Downloading { pending: usize },
}

/// Pure function, used by the combiner and by unit tests (behavior M.3).
pub fn derive(manga_running: bool, manga_len: usize, anime_running: bool, anime_len: usize) -> DownloadQueueState {
    let size = manga_len + anime_len;
    let is_downloading = manga_running || anime_running;
    if size == 0 { DownloadQueueState::Stopped }
    else if !is_downloading { DownloadQueueState::Paused { pending: size } }
    else { DownloadQueueState::Downloading { pending: size } }
}

pub fn subtitle(state: DownloadQueueState) -> Option<String> {
    match state {
        DownloadQueueState::Stopped => None,
        DownloadQueueState::Paused { pending: 0 } => Some("Paused".into()),
        DownloadQueueState::Paused { pending } => Some(format!("Paused • {pending} remaining")),
        DownloadQueueState::Downloading { pending } => Some(format!("{pending} remaining")),
    }
}
```
- Inputs: four `tokio::sync::watch::Receiver`s: manga running `bool`, manga queue `usize`, anime running `bool`, anime queue `usize`. The combiner uses `tokio::select!` (or `futures::future::select_all`) over the four `changed()` futures, recomputes `derive` on every change, and publishes via a `watch::Sender<DownloadQueueState>`. The GTK side consumes it with `glib::spawn_future_local`.
- Using one `select!` instead of the Kotlin nested `collectLatest` removes the restart semantics; the result is identical because each change recomputes from the latest four values (behavior M.3 step 1-6).

### 8.4 Settings bindings in the More page
- `downloaded_only` and `incognito_mode` switches bind `Pref<bool>` (`__APP_STATE_pref_downloaded_only`, `__APP_STATE_incognito_mode`) through `switch::build` with no veto.
- The switches update in both directions: `pref.watch()` also flips the switch (guard against feedback loop with `set_active` only when different).

## 9. Constants (`src/settings/widgets/mod.rs`)
```rust
pub const PREF_HORIZONTAL_PADDING: i32 = 16;
pub const PREF_VERTICAL_PADDING: i32 = 16;
pub const TRAILING_WIDGET_BUFFER: i32 = 16;
pub const PREF_MIN_HEIGHT: i32 = 56;
pub const GROUP_SPACER: i32 = 12;
pub const GROUP_HEADER_TOP: i32 = 14;
pub const GROUP_HEADER_BOTTOM: i32 = 8;
pub const SUBTITLE_MAX_LINES: i32 = 10;
pub const TITLE_MAX_LINES: i32 = 2;
pub const TRI_STATE_ROW_MIN: i32 = 48;
pub const CHECK_ROW_MIN: i32 = 48;
pub const THEME_CARD_WIDTH: i32 = 114;
pub const SECONDARY_ALPHA: f64 = 0.78;
pub const DISABLED_ALPHA: f64 = 0.38;
pub const FLASH_DELAY_MS: u64 = 600;
pub const FLASH_STEP_MS: u64 = 200;
pub const FLASH_TOGGLES: u32 = 10;
pub const FLASH_TOTAL_MS: u64 = 3000;
pub const HIGHLIGHT_SCROLL_DELAY_MS: u64 = 500;
pub const TRACKER_CHECK_COLOR: &str = "#4CAF50";
```
Font size note: 16 px title is expressed in CSS as `font-size: 16px` (GTK), not sp. Dimension conversion: 1 dp = 1 px at scale 1; GTK applies the desktop scale factor automatically.

## 10. Android API to Linux mapping (this area)

| Android / Compose | Linux / Rust replacement | Notes |
|---|---|---|
| `AlertDialog` | `gtk::Window` transient modal with `show_dialog` helper (§3.1) | |
| `AnimatedVisibility` (expand/fade) | `gtk::Revealer` slide-down, 150 ms | |
| `animateColorAsState` + `repeatable` | CSS `transition` + `glib::timeout_add_local` (§6.4) | No @keyframes in GTK4 CSS |
| `Switch` | `gtk::Switch` | |
| `Slider` | `gtk::Scale` | haptics dropped |
| `RadioButton` | `gtk::CheckButton` with `set_group` | |
| `Checkbox` | `gtk::CheckButton` | |
| `SegmentedButton` | `gtk::ToggleButton` group + css `linked` | |
| `LazyColumn` / `ScrollbarLazyColumn` | `gtk::ScrolledWindow` + `gtk::ListBox` | GTK overlay scrollbars |
| `LazyRow` | horizontal `gtk::ScrolledWindow` + `gtk::Box` | |
| `rememberLazyListState().animateScrollToItem` | `gtk::ScrolledWindow::vadjustment` set with a 250 ms ease | |
| `rememberCoroutineScope` | `glib::spawn_future_local` | |
| `collectAsState` on `Preference` | `Pref::watch()` + `glib::spawn_future_local` | |
| `BackHandler` | `gtk::Window::connect_close_request` returning `Propagation::Stop` | onboarding only |
| `UriHandler.openUri` / `openInBrowser` | `gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gio::Cancellable>, cb)` | |
| `ActivityCompat.recreate` | not needed: CSS swap | |
| `HapticFeedback` | dropped | |
| `Icon(painterResource)` | `gtk::Image::from_icon_name` | verify names |
| `AnimatedImageVector` (`anim_more_enter`) | static icon | animation dropped |
| `DeviceUtil.isDynamicColorAvailable` | constant `false` on Linux | hides MONET |
| `Environment.isExternalStorageManager` | not applicable | MPV write path §8 of behavior |
| `Build.VERSION.SDK_INT` | not applicable | |
| Compose `@Preview` | dropped; a `cargo run -- --shot settings_appearance` screenshot path (mockup has `KUROBAYOMI_SHOT`) replaces it | |

## 11. Crates
- `gtk4` 0.11.5 (already). Features: `v4_16` (already). `gtk::UriLauncher` (4.10+), `gtk::Revealer`, `gtk::AlertDialog` available.
- `glib` (transitive, use `glib::timeout_add_local`, `glib::spawn_future_local`, `glib::ControlFlow`).
- `tokio` (features `rt-multi-thread`, `sync`, `macros`, `time`): watch channels, select, spawn_blocking.
- `pulldown-cmark` (markdown changelog parse, no default features beyond `html`-off). Used only in `more/update_dialog.rs` and any other markdown view (area:38-41 may reuse).
- `serde`, `serde_json` (already): for `spec/*.json` loading. Not used by this area beyond spec labels.
- No `libadwaita`, no `relm4`, no `async-std`.
- Tests: `#[test]` plain Rust for `row_index_for`, `queue_state::derive`, `queue_state::subtitle`, `changelog_strip`, `theme list filter`. No GTK needed for these (keep pure functions outside GTK types). GTK widget tests are out of scope (run via the `KUROBAYOMI_SHOT` path).

## 12. Changelog strip (`more/update_dialog.rs`)
```rust
/// Equivalent of ui/more/NewUpdateScreen.kt:25 `"---(\\R|.)*Checksums(\\R|.)*"` replaced with "".
pub fn strip_checksums(changelog: &str) -> String {
    // Find the first "---" that is followed (anywhere later) by "Checksums"; remove from there to end of string.
    let mut search_from = 0;
    while let Some(rel) = changelog[search_from..].find("---") {
        let start = search_from + rel;
        if changelog[start..].contains("Checksums") {
            return changelog[..start].to_string();
        }
        search_from = start + 3;
    }
    changelog.to_string()
}
```
- Kotlin `replace(regex, "")` replaces every match; there is exactly one match because the greedy tail consumes everything to the end, so returning the prefix is equivalent. Regex semantics: `(\R|.)*` covers `\n`, `\r\n`, and any other line break; the Rust version works on the whole string so newlines are included by construction.
- Multi-byte safety: `find` returns byte offsets on char boundaries (`"---"` is ASCII), so slicing is safe.

## 13. Onboarding gate (`src/onboarding/mod.rs`)
- `shown_onboarding_flow: Pref<bool>` key `__APP_STATE_onboarding_complete`, default false (area:02).
- Window close request: if `!shown`, return `glib::Propagation::Stop` (blocks close and back, behavior §O).
- Finish: `shown.set(true)` then pop the onboarding page.
- Restore backup: finish, then `highlight::request(label_backup_title)` and push the Data-and-storage page. Title string = "Backup and restore" (area:51 key `label_backup`).

## 14. Pitfalls
- P-index: Do not compute scroll positions from row indices; look up the row widget and use `compute_point` (§6.2). Spacers are margins, never rows.
- P-flash-idempotent: `flash()` run twice on one row leaves two toggling timers. Keep the three `glib::SourceId`s (`Option<SourceId>` each) in an `Rc<RefCell<...>>` captured by the page, and call `SourceId::remove()` on them before starting a new flash.
- P-rc-dialog: dialog windows hold `Rc` closures; close them with `window.destroy()` to avoid leaks.
- P-spawn-local: `glib::spawn_future_local` must be called on the GTK thread. Veto futures that call `tokio::spawn` need a `Handle` captured at startup (`tokio::runtime::Handle::current()` stored in a `OnceLock`).
- P-icon: verify every icon name from data-and-config §8 with `gtk4-icon-browser` on the target machine. Missing names render a broken-image placeholder silently.
- P-enum-storage: enum prefs are stored as strings (`ThemeMode`, `AppTheme`, `NavStyle`); deprecated `AppTheme` names must map to `Default` on read (behavior §3.2).
- P-css-scale: px values above assume scale factor 1. Do not hard-code device pixels; use CSS for spacing so HiDPI works.
- P-theme-dark: `set_gtk_application_prefer_dark_theme(None)` is required for the System choice, otherwise the app stays forced dark or light after the user picks System.
- P-dialog-modal: transient-for must be set to the main window, or the dialog opens behind the window on KDE.
- P-list-reselect: radio list dialog closes only on a different selection; re-clicking the current row must do nothing (behavior B.4).

## 15. Mockup integration points (exact)
- `src/ui.rs` `item_row` (line 221): replace the match arms with calls into `settings::render::item_row`. The mockup's `Item::Multi` MenuButton popover (lines 245-255) becomes the dialog in §3.
- `src/ui.rs` `item_row` `Item::List` uses `gtk::DropDown`. Decision: switch to the dialog (`list::dialog`) for parity with Kotlin, OR keep DropDown as a documented divergence. Decision in this design: dialog (behavior B.4). The Kotlin list is a modal radio dialog that closes on selection and has no inline control, and an inline DropDown cannot reproduce the re-select no-op (E-24). This is a deliberate change from the mockup.
- `src/screens.rs` `more()` (lines 336-352): keep, but drop the hard-coded History removal and derive it from `nav_style` (§8.2).
- `src/screens.rs` `info_screen("onboarding")` (lines 492-535): keep the layout; replace its hard-coded button labels with the onboarding flow from area:36 and its `Get started` text. Back-blocking goes into §13.
- `src/screens.rs` `info_screen("new_update")` (line 497-498): buttons `Download` and `Not now` match behavior §U.1. Subtitle must be the real version string, not `v0.1.0`.
- `src/settings.rs` `search()` (line 47) is area:47. This area provides the highlight handoff only.
- `src/settings.rs` `main_menu()` (line 26-45): area:47.

## 16. Work sequence (for the tasks file)
1. Model + pure functions (`model.rs`, `row_index_for`, `queue_state::derive`, `strip_checksums`, theme filter) with unit tests.
2. Row builders and dialog helper.
3. Page scaffold, group rendering, highlight handoff and flash.
4. More tab page and queue combiner.
5. Theme pickers (depends on area:49 palette API).
6. Update dialog and onboarding gate (depend on area:35 and area:36).
7. Mockup switch-over and removal of the mockup-only paths.
