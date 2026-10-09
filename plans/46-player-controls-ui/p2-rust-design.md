# 46-player-controls-ui (part 2 of 2): Rust design

Target: Kurobayomi (`/home/blackbox/kurobayomi`, crate `kurobayomi`, edition 2024, `gtk` = gtk4-rs 0.11.5 with feature `v4_16`, `serde`, `serde_json`). Part 1 of the player (area:45-player-core, `plans/45-player-core/p2-rust-design.md`) owns `src/player/mpv.rs` (the `libmpv2` wrapper), `src/player/model.rs` (`PlayerModel`), and the `GtkGLArea` surface. This part provides the sheets, panels and the small pure logic they need. Read `p2-behavior.md` for every rule referenced here.

Crates used by this part (in addition to what area:45 already declares):

| Need | Crate | Why |
|---|---|---|
| Font family names from TTF/OTF | `ttf-parser` | replaces `com.yubyf.truetypeparser.TTFFile`; read the `name` table, family name id 1 (see §7) |
| Error type derive | `thiserror` | `MpvError` in §2 (not yet in `Cargo.toml`; add at implementation) |
| mpv properties and commands | `libmpv2` (via area:45 `mpv.rs`) | replaces `is.xyz.mpv.MPVLib` |
| Main-thread results from threads | `async-channel` + `glib::spawn_future_local` | same mechanism as area:45 |
| Timers (20 ms tap-sync tick, 350 ms animations) | `glib::timeout_add_local`, `std::time::Instant` | replaces coroutine `delay(20)` and `System.currentTimeMillis()` |
| File pickers | `gtk4::FileDialog` (GTK 4.10+) | replaces `ActivityResultContracts.OpenDocument` |
| Color picker (optional extra) | `gtk4::ColorDialogButton` (GTK 4.10+) | optional, RGBA ↔ ARGB at the boundary |
| XDG share (optional) | `ashpd` (portal `OpenURI`) | replaces Android share sheet; verify crate version at implementation time |

Not used: libadwaita (forbidden), any Android shim, `chrono` (only epoch millis are needed; `std::time` is enough).

---

## 1. Crate and module layout

```
src/
  player.rs                         (area:45) mod facade; this part adds the modules below
  player/
    ui/
      mod.rs                        re-exports SheetHost, SheetKind, PanelKind
      host.rs                       SheetHost: overlay container, scrim, open/close, Escape handling (§3)
      widgets.rs                    SliderInt, SliderFloat, SwitchRow, NumericChooser, ExpandableCard, TrackRow helpers (§4)
      sheets/
        mod.rs
        track_list.rs               generic track list skeleton (GenericTracksSheet)
        tracks.rs                   AudioTracks + Subtitles sheets (uses track_list)
        chapters.rs                 ChaptersSheet
        quality.rs                  QualitySheet + HosterRow + VideoRow
        speed.rs                    PlaybackSpeedSheet
        more.rs                     MoreSheet (decoder, stats page, custom buttons, audio channels, timer entry)
        screenshot.rs               ScreenshotSheet
        sleep_timer.rs              TimePicker replacement dialog
      panels/
        mod.rs
        subtitle_settings.rs        SubtitleSettingsPanel container (tabs)
        subtitle_typography.rs      SubtitleSettingsTypographyCard
        subtitle_colors.rs          SubtitleSettingsColorsCard
        subtitle_misc.rs            SubtitlesMiscellaneousCard
        subtitle_delay.rs           SubtitleDelayPanel
        delay_card.rs               DelayCard widget (shared with audio delay, part 1)
        video_filters.rs            VideoFiltersPanel
    logic/                          PURE, no GTK, unit-tested (§8)
      mod.rs
      track_label.rs                getTrackTitle rules
      delay_measure.rs              tap-to-sync state machine (§5)
      hoster_groups.rs              hoster partition (QualitySheet §5.4)
      hoster_state.rs               HosterState enum + get_changed_at
      number.rs                     to_fixed, round_half_even_3, pretty_time, format_elapsed
      color.rs                      copy_argb, color_hex (ARGB ↔ "#AARRGGBB")
      speed_presets.rs              preset parse/format/sort/add/remove
      audio_channels.rs             AudioChannels mapping table (label ↔ mpv value)
      stats_page.rs                 statistics page toggle decision (xor truth table)
      fonts.rs                      TTF family scan (ttf-parser), regex-equivalent filename test
      decoder.rs                    Decoder enum + mpv value mapping (Linux values), unknown-value fallback
    settings_view.rs                typed reads/writes for keys in this part (§6), built on area:02 store
    mpvprops.rs                     typed property helpers over `MpvProps` trait (§2)
```

Mockup integration (existing files): `src/player.rs` calls `sheet("player", id)` and `dialog("player", id)` (lines 7-15, 184-191, 227-229, 330). After this part, the player screen calls `SheetHost::open(SheetKind::…)` and `SheetHost::open_panel(PanelKind::…)` directly; the spec-driven `Sheet` path (`src/spec.rs`, `src/ui.rs::show_sheet`) remains only for the reader screen. The mockup spec entries `screens[player].sheets` and `panels` are kept as static documentation until the port replaces them; see p2-ui-parity.md for the gap list.

---

## 2. Core trait and the mpv boundary

`src/player/mpvprops.rs` defines a trait so the logic and widgets can be tested without libmpv:

```rust
/// Property access used by this part. Implemented for the area:45 `Mpv` wrapper
/// and for an in-memory fake in tests. All calls happen on the GTK main thread.
pub trait MpvProps {
    fn get_f64(&self, name: &str) -> Option<f64>;
    fn set_f64(&self, name: &str, v: f64) -> Result<(), MpvError>;
    fn get_i64(&self, name: &str) -> Option<i64>;
    fn set_i64(&self, name: &str, v: i64) -> Result<(), MpvError>;
    fn get_bool(&self, name: &str) -> Option<bool>;
    fn set_bool(&self, name: &str, v: bool) -> Result<(), MpvError>;
    fn get_str(&self, name: &str) -> Option<String>;
    fn set_str(&self, name: &str, v: &str) -> Result<(), MpvError>;
    fn command(&self, args: &[&str]) -> Result<(), MpvError>;
}

#[derive(Debug, thiserror::Error)]
pub enum MpvError {
    #[error("mpv rejected property {name}: {reason}")]
    Property { name: String, reason: String },
    #[error("mpv command failed: {0}")]
    Command(String),
}
```

Rules:
- `get_*` returns `None` on any mpv error. The caller then falls back to the **stored preference** (not to a hard-coded value). This replaces the Kotlin `!!` and platform-type NPEs (behavior §13.2, §14.1).
- `set_*` errors are logged with `tracing::warn!` and do not interrupt the UI. Kotlin ignored return values; the Rust port reports them in the UI only for the decoder chip (see §9).

Typed helper functions (in `mpvprops.rs`), each mapping one Kotlin call site:

```rust
pub fn sub_delay_secs(m: &dyn MpvProps) -> Option<f64>                 // "sub-delay"
pub fn set_sub_delay_secs(m: &dyn MpvProps, secs: f64) -> Result<(), MpvError>
pub fn set_sub_style_bool(m: &dyn MpvProps, prop: SubBoolProp, on: bool) -> Result<(), MpvError>   // "sub-bold", "sub-italic", "sub-ass-justify"
pub fn set_argb_property(m: &dyn MpvProps, prop: SubColorProp, argb: i32) -> Result<(), MpvError>  // writes "#AARRGGBB"
pub fn set_audio_channels(m: &dyn MpvProps, mode: AudioChannelsMode) -> Result<(), MpvError>   // §10
pub fn set_hwdec(m: &dyn MpvProps, mode: DecoderMode) -> Result<(), MpvError>                  // "hwdec"
pub fn stats_toggle(m: &dyn MpvProps) -> Result<(), MpvError>                                  // command script-binding stats/display-stats-toggle
pub fn stats_page(m: &dyn MpvProps, page: u8) -> Result<(), MpvError>                          // script-binding stats/display-page-N
pub fn custom_button(m: &dyn MpvProps, id: i64, long: bool) -> Result<(), MpvError>            // script-message call_button_<id>[_long]
pub fn screenshot_to_file(m: &dyn MpvProps, path: &str, with_subs: bool) -> Result<(), MpvError>  // screenshot-to-file <path> subtitles|video
```

Enum-to-string sub-property enums:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubBoolProp { Bold, Italic, AssJustify }
```

Note: `sub-ass-override` is a **string** (`force`/`scale`), not a bool. It is written with `set_str`. `SubBoolProp` covers only `Bold`, `Italic`, `AssJustify`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubColorProp { Text, Border, Background }
impl SubColorProp {
    pub fn property(self) -> &'static str { match self {
        Self::Text => "sub-color", Self::Border => "sub-border-color", Self::Background => "sub-back-color" } }
}
```

---

## 3. Sheet and panel host (`src/player/ui/host.rs`)

Replaces `PlayerSheet` (behavior §0.1) and the `Sheets` / `Panels` routing (behavior §0.3).

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetKind { None, PlaybackSpeed, SubtitleTracks, AudioTracks, QualityTracks, Chapters, More, Screenshot }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind { None, SubtitleSettings, SubtitleDelay, AudioDelay, VideoFilters }

pub struct SheetHost {
    overlay: gtk::Overlay,          // parent: player surface overlay (area:45)
    scrim: gtk::Box,                // full size, background rgba(0,0,0,0.5), click -> close()
    card: gtk::Box,                 // bottom-aligned, max-width 420 px (720 px when window width >= 900), max-height 95%
    current: std::cell::Cell<SheetKind>,
    panel: std::cell::Cell<PanelKind>,
    on_close: std::rc::Rc<dyn Fn()>,
}

impl SheetHost {
    pub fn new(overlay: &gtk::Overlay, on_close: impl Fn() + 'static) -> Self;
    pub fn open(&self, kind: SheetKind, content: &impl glib::IsA<gtk::Widget>);
    pub fn open_panel(&self, kind: PanelKind, content: &impl glib::IsA<gtk::Widget>);
    /// Equivalent of PlayerSheet's dismiss: 350 ms fade-out, then `on_close` exactly once.
    pub fn close(&self);
    pub fn is_open(&self) -> bool;
}
```

Behavior mapping:
- `open` while another sheet is open replaces it (the Kotlin `when (sheetShown)` only renders one).
- Scrim alpha animates 0 → 0.5 over 350 ms on open (`gtk::Revealer` with `transition-duration 350`, or CSS transition on opacity; the `Revealer` is preferred).
- `close()` reveals to 0 over 350 ms, then removes the child and calls `on_close` once (guard with a `Cell<bool>`).
- Escape key: `gtk::EventControllerKey` on the overlay; `Escape` → `close()` (replaces `BackHandler`).
- Clicks inside the card are consumed by the card (`gtk::GestureClick` that claims the sequence), replacing `clickable(onClick = {})`.
- No drag-to-dismiss. The scrim click and Escape are the equivalents. Parity note in p2-ui-parity.md.
- Width: `card.set_width_request` is not used; use `set_size_request(420, -1)` for portrait-like windows and `720` when the window is ≥ 900 px wide, adjusted on `notify::default-width` (replaces `LocalConfiguration.orientation`).

Panels (`PanelKind`) use the same host, with `card` sized to `CARDS_MAX_WIDTH = 420` and aligned to the right edge (`halign = End`) with `linkTo bias 0.8`, i.e. `valign` at 80% of the height (`gtk::Box` with `margin-top` computed as `0.8 * (height - card_height)`; implemented by a `gtk::Fixed` or by `valign = Fill` plus `margin_top`). Behavior §11.2 and §16.1.

---

## 4. Shared widgets (`src/player/ui/widgets.rs`)

| Kotlin composable | Rust | Notes |
|---|---|---|
| `SliderItem` Int overload (behavior §0.4) | `SliderRow::int(label, valuer, max, min, icon, on_change)` | `gtk::Scale` with `set_range(min, max)`, `set_increments(1, 10)`, `set_round_digits(0)`; `on_change` only when the integer differs; value text label |
| `SliderItem` Float overload | `SliderRow::float(label, value, max, min, steps, on_change)` | `set_round_digits(-1)`; `steps` > 0 → `add_mark` at equal intervals (optional) |
| `OutlinedNumericChooser` | `NumericChooser::int(value, step, min, max, on_change)` / `NumericChooser::float(value, step, min, max, on_change)` | `gtk::SpinButton` with `set_adjustment` **without clamping**: set `set_range(f64::MIN, f64::MAX)` and show an error style (`error` CSS class) plus the same message keys when out of bounds; the Kotlin field does not clamp (behavior §0.5) |
| `SwitchPreference` | `SwitchRow::new(title, active, on_change)` | `gtk::Switch` in a row; the whole row toggles (`gtk::GestureClick` on the row) |
| `ExpandableCard` | `gtk::Expander` (title = header) | default expanded = true |
| `ExposedTextDropDownMenu` | `gtk::DropDown` with `gtk::StringList` | `enable-search(false)`; spinner `gtk::Spinner` as suffix while fonts load |
| `TintedSliderItem` (color channels) | `SliderRow::int` with `css_classes` `tint-red/green/blue/white` | tint via CSS `scale trough highlight` color |
| `ActionButton` (icon + title) | `ActionTile::new(icon, title)` | `gtk::Button` with vertical box |
| Track row (`RadioButton` / `Checkbox`) | `gtk::CheckButton` in a group (radio) / standalone (checkbox) | group for audio single-select |
| `AddTrackRow` | `gtk::Button` with `Add` icon, height 48 px | |
| Chip (`FilterChip`, `InputChip`) | `gtk::ToggleButton` with `css_classes ["chip"]`; `InputChip` = `gtk::Box` with label and a close `gtk::Button` | |
| `FlowRow` | `gtk::FlowBox` (`min-children-per-line 1`, `max-children-per-line` unlimited) | |
| `LazyColumn` / `LazyRow` | `gtk::ListBox` (short lists) or `gtk::ListView` with `gio::ListStore` for long lists (chapters, > 200) | |

Rows that use `Checkbox` and `RadioButton` must keep the exact italic and bold rules (behavior §2.2, §3.2): selected row labels use `font-style: italic; font-weight: 800` via a CSS class `selected-track`.

---

## 5. Tap-to-sync delay measurement (`src/player/logic/delay_measure.rs`)

Pure state machine. Implements behavior §11.5 exactly. The UI calls `press_a` / `press_b` and a 20 ms `tick`.

```rust
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayKind { Audio, Subtitle }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign { Positive, Negative }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button { A, B }

impl DelayKind {
    /// Start sign of button A and B (behavior §11.5).
    pub fn start_sign(self, b: Button) -> Sign {
        match (self, b) {
            (DelayKind::Audio, Button::A) => Sign::Positive,   // "Sound heard"
            (DelayKind::Audio, Button::B) => Sign::Negative,   // "Sound spotted"
            (DelayKind::Subtitle, Button::A) => Sign::Negative,// "Voice heard"
            (DelayKind::Subtitle, Button::B) => Sign::Positive,// "Text seen"
        }
    }
}

pub struct DelayMeasure {
    kind: DelayKind,
    run: Option<Run>,
    /// Value that is shown when the run ends. Kotlin: `finalDelay`.
    final_ms: i64,
}

struct Run { sign: Sign, started: Instant, start_ms: i64 }

pub enum Outcome {
    Started,            // a run began
    Stopped(i64),       // run ended; caller applies this delay (ms) to the parent value
    Ignored,            // button disabled
}

impl DelayMeasure {
    pub fn new(kind: DelayKind, current_ms: i64) -> Self;

    /// Button enabled iff not running in its own start direction (see §5.1).
    pub fn enabled(&self, b: Button) -> bool {
        match &self.run {
            None => true,
            Some(r) => r.sign != self.kind.start_sign(b),
        }
    }

    pub fn press(&mut self, b: Button, now: Instant, current_ms: i64) -> Outcome {
        if !self.enabled(b) { return Outcome::Ignored; }
        match self.run.take() {
            None => {
                let sign = self.kind.start_sign(b);
                self.final_ms = current_ms;
                self.run = Some(Run { sign, started: now, start_ms: current_ms });
                Outcome::Started
            }
            Some(r) => {
                // stop: final value is the last tick value
                let v = Self::value_at(&r, now);
                self.final_ms = v;
                Outcome::Stopped(v)
            }
        }
    }

    /// Live value while running (Kotlin does not display it; the port may, see p2-ui-parity).
    pub fn tick(&mut self, now: Instant) -> i64 {
        match &self.run {
            Some(r) => { self.final_ms = Self::value_at(r, now); self.final_ms }
            None => self.final_ms,
        }
    }

    fn value_at(r: &Run, now: Instant) -> i64 {
        let elapsed = now.saturating_duration_since(r.started).as_millis() as i64;
        match r.sign { Sign::Positive => r.start_ms + elapsed, Sign::Negative => r.start_ms - elapsed }
    }

    pub fn is_running(&self) -> bool { self.run.is_some() }
}
```

§5.1 Rule proof sketch (to be a unit test): Kotlin `enabled(A) = state != (type == Audio)`, `enabled(B) = state != (type == Subtitle)`. Substituting the two `start_sign` values gives `enabled(b) == (run.sign != start_sign(b))`. Verified for both kinds in `delay_measure` tests (§8).

Important behavior preserved:
- `start_ms` is the value at start (Kotlin `startingDelay = finalDelay = delay`).
- Elapsed is wall-clock; sampled at each 20 ms tick (the port uses `Instant`, so a late tick does not lose time).
- The Kotlin "stop" writes `finalDelay` to the parent. The port's `Outcome::Stopped(v)` is applied through the same `on_change` path so mpv is written once.
- `DelayMeasure::new` sets `final_ms` to the current value so a stop without a run is impossible (the Kotlin `LaunchedEffect(null)` initial write is a no-op).

Glib tick (concrete form):

```rust
let measure = std::rc::Rc::new(std::cell::RefCell::new(DelayMeasure::new(kind, current_ms)));
let tick_id: std::rc::Rc<std::cell::RefCell<Option<glib::SourceId>>> = Default::default();
// on press that returns Outcome::Started:
let (m, slot, apply) = (measure.clone(), tick_id.clone(), on_live.clone());
let id = glib::timeout_add_local(std::time::Duration::from_millis(20), move || {
    let mut m = m.borrow_mut();
    apply(m.tick(std::time::Instant::now()));   // live value; caller decides to display or not
    if m.is_running() { glib::ControlFlow::Continue } else { slot.borrow_mut().take(); glib::ControlFlow::Break }
});
*tick_id.borrow_mut() = Some(id);
```

`on_live` is an `Rc<dyn Fn(i64)>`. The Kotlin behavior (value not displayed during the run) is preserved by passing a no-op. The source is cancelled on stop, and `SheetHost::close` drops the `Rc` so no timer outlives the panel.

---

## 6. Settings (`src/player/settings_view.rs`)

Typed views over the area:02 store. The store trait comes from area:02; this part defines what it needs:

```rust
pub trait Store {
    fn get_i64(&self, key: &str, default: i64) -> i64;
    fn get_f32(&self, key: &str, default: f32) -> f32;
    fn get_bool(&self, key: &str, default: bool) -> bool;
    fn get_str(&self, key: &str, default: &str) -> String;
    fn get_str_set(&self, key: &str, default: &[&str]) -> Vec<String>;
    fn set_i64(&self, key: &str, v: i64);
    fn set_f32(&self, key: &str, v: f32);
    fn set_bool(&self, key: &str, v: bool);
    fn set_str(&self, key: &str, v: &str);
    fn set_str_set(&self, key: &str, v: &[String]);
    fn delete(&self, key: &str);   // next get returns default
}
```

Enum-typed preferences are stored by **variant name** (behavior §1.2, `PreferenceStore.kt:27-41`). The port uses the same stored form so existing backups restore correctly: store `"AutoSafe"`, `"OutlineAndShadow"`, `"Auto"` (the enum name, not the mpv value). Unknown names fall back to the default (`enumValueOf` catch in Kotlin).

```rust
pub struct SubtitlePrefs { /* one field per key in p2-data-and-config.md §3.1 */ }
impl SubtitlePrefs {
    pub fn load(s: &dyn Store) -> Self;
    pub fn reset_typography(&self, s: &dyn Store, m: &dyn MpvProps);   // behavior §13.8, with the bug fix (sub-ass-justify reset to false)
    pub fn reset_misc(&self, s: &dyn Store, m: &dyn MpvProps);         // behavior §15.1
    pub fn reset_colors(&self, s: &dyn Store, m: &dyn MpvProps, t: SubColorProp);  // behavior §14.4
}
```

Keys: exact Kotlin keys from `p2-data-and-config.md` §3, including the non-prefixed `sub_shadow_offset`.

Default enum for `AudioChannels` in the Rust port: the stored name is still `AutoSafe` (see §10).

---

## 7. Font scan (`src/player/logic/fonts.rs`)

```rust
pub fn is_font_file(name: &str) -> bool {
    // Kotlin: name.lowercase().matches(Regex(".*\\.[ot]tf$"))
    let n = name.to_lowercase();
    n.ends_with(".ttf") || n.ends_with(".otf")
}

pub fn family_name(bytes: &[u8]) -> Option<String> {
    // Kotlin: TTFFile.open(stream).families.values.first(). The port reads the name table
    // through ttf-parser, takes the first non-empty FAMILY (name id 1) record, and prefers a
    // Unicode-encoded record over a legacy one.
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let mut legacy: Option<String> = None;
    for name in face.names().into_iter().filter(|n| n.name_id == ttf_parser::name_id::FAMILY) {
        let Some(s) = name.to_string() else { continue };
        if s.trim().is_empty() {
            continue;
        }
        if name.is_unicode() {
            return Some(s);
        }
        legacy.get_or_insert(s);
    }
    legacy
}

pub const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;

pub fn scan_fonts_dir(dir: &std::path::Path) -> Vec<String> {
    // Entries are visited in file-name order so the result is deterministic
    // (Kotlin listFiles() order is unspecified). Duplicates keep the first occurrence (Kotlin distinct()).
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<std::fs::DirEntry> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    let mut out: Vec<String> = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_font_file(&name) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() > MAX_FONT_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else { continue };
        let Some(family) = family_name(&bytes) else { continue };
        if !out.contains(&family) {
            out.push(family);
        }
    }
    out
}

Threading: `scan_fonts_dir` runs on a `std::thread::spawn`; result goes back through `async_channel::Sender<Vec<String>>` and is applied in `glib::spawn_future_local`. Kotlin uses `Dispatchers.IO` (behavior §13.1).

The list is `["Sans Serif"]` first, then discovered families (behavior §13.1). The port **does not** duplicate `"Sans Serif"` if a discovered family equals it (deviation, flagged in p2-edge-cases-and-tests.md).

Fonts directory: `<app-data>/mpv-config/fonts` created on first use (area:13 storage helper `get_fonts_dir()`; it must create the directory once, not on every render, unlike `getFontsDirectory()` in Kotlin).

Size guard (proposal): files larger than `MAX_FONT_BYTES` (64 MiB) are skipped. CJK fonts are commonly 20-40 MB, so the cap leaves headroom while preventing a runaway read.

---

## 8. Pure logic functions (exact signatures and rules)

`src/player/logic/track_label.rs`

```rust
pub fn track_label(id: i64, name: &str, language: Option<&str>) -> String;
// id == -1 -> name verbatim
// language blank && name nonblank -> format!("#{}: {}", id, name)
// language nonblank && name nonblank -> format!("#{}: {} ({})", id, name, lang)
// language nonblank && name blank -> format!("#{}: {}", id, lang)
// else -> format!("#{}: {}", id, name)
// "blank" = trim().is_empty()
```

`src/player/logic/number.rs`

```rust
/// Kotlin Float.toFixed(precision): (x * 10^p).roundToInt() / 10^p, half-up (Math.round semantics).
pub fn to_fixed(x: f32, precision: u32) -> f32;
/// Canonical preset string for a speed: to_fixed(x, 2) formatted with Kotlin Float.toString rules
/// ("1.0", "0.25", "1.75"). Keep one fractional digit minimum.
pub fn preset_string(x: f32) -> String;
/// round(x * 1000) / 1000 with round-half-even (kotlin.math.round). Used by the subtitle speed chooser.
pub fn round3_half_even(x: f32) -> f32;   // f32::round_ties_even(x * 1000.0) / 1000.0
/// Kotlin DateUtils.formatElapsedTime(seconds): "MM:SS" under one hour, "H:MM:SS" otherwise.
pub fn format_elapsed(total_secs: i64) -> String;
/// Chapter time label, the Utils.prettyTime call site. Proposal format: same shape as format_elapsed
/// (see §11). Kotlin's implementation is in the external mpv-android artifact (not in repo).
pub fn pretty_time(total_secs: i64) -> String;
```

`src/player/logic/color.rs`

```rust
pub fn copy_argb(c: i32, a: Option<u8>, r: Option<u8>, g: Option<u8>, b: Option<u8>) -> i32;
// (alpha shl 24) or (red shl 16) or (green shl 8) or blue, on u32, cast to i32.
pub fn color_hex(c: i32) -> String;   // format!("#{:08X}", c as u32)
pub fn parse_mpv_color(s: &str) -> Option<i32>;
// accepts #RRGGBB (alpha ff) and #AARRGGBB; case-insensitive; returns None otherwise.
// Kotlin Color.parseColor throws; the port returns None and the caller keeps the stored preference.
```

`src/player/logic/speed_presets.rs`

```rust
pub const DEFAULT_PRESETS: [&str; 12] = ["0.25","0.5","0.75","1.0","1.25","1.5","1.75","2.0","2.5","3.0","3.5","4.0"];
pub fn parse_sorted(raw: &[String]) -> Vec<f32>;   // skips unparseable strings (logged), sorts ascending
pub fn add(raw: &mut Vec<String>, speed: f32);      // set semantics: no duplicate
pub fn remove(raw: &mut Vec<String>, speed: f32);   // removes preset_string(speed) only
```

`src/player/logic/hoster_groups.rs`

```rust
pub struct Groups<'a> { pub valid: Vec<(usize, &'a HosterState)>, pub failed: Vec<(usize, &'a HosterState)>, pub empty: Vec<(usize, &'a HosterState)> }
pub fn group_hosters<'a>(hosters: &'a [HosterState], show_failed: bool, show_empty: bool) -> Groups<'a>;
// valid: Idle | Loading | Ready with videos non-empty
// failed: Error
// empty: Ready with no videos
// display rule: failed rendered only if show_failed; empty only if show_empty (behavior §5.4)
```

`src/player/logic/audio_channels.rs`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioChannels { Auto, AutoSafe, Mono, Stereo, ReverseStereo }

impl AudioChannels {
    pub const ALL: [AudioChannels; 5] = [Self::Auto, Self::AutoSafe, Self::Mono, Self::Stereo, Self::ReverseStereo];
    pub fn label(self) -> &'static str;       // "Auto", "Auto-safe", "Mono", "Stereo", "Reverse stereo"
    pub fn stored_name(self) -> &'static str; // "Auto", "AutoSafe", "Mono", "Stereo", "ReverseStereo" (Kotlin enum names)
    /// (property, value) exactly as Kotlin's AudioChannels.property / value
    pub fn mpv_write(self) -> (&'static str, &'static str) {
        match self {
            Self::Auto => ("audio-channels", "auto-safe"),
            Self::AutoSafe => ("audio-channels", "auto"),
            Self::Mono => ("audio-channels", "mono"),
            Self::Stereo => ("audio-channels", "stereo"),
            Self::ReverseStereo => ("af", "pan=[stereo|c0=c1|c1=c0]"),
        }
    }
    pub fn from_stored(s: &str) -> AudioChannels; // unknown -> AutoSafe (Kotlin fallback)
}
```

Apply sequence (port, behavior §7.3 with the proposed af fix):
1. If `self == ReverseStereo`: `set audio-channels auto`, then `af` add pan filter (add the string `pan=[stereo|c0=c1|c1=c0]` to `af` with mpv's `af add` semantics; the port keeps the exact Kotlin value string).
2. Else: remove only the pan filter from `af` (proposal) instead of `af=""` (Kotlin clears all). Then `set audio-channels <value>`.

`src/player/logic/stats_page.rs`

```rust
/// Kotlin: if ((page == 0) xor (current == 0)) toggle; if (page != 0) display-page-N; then store page.
pub fn stats_actions(current: u8, page: u8) -> StatsActions;
pub struct StatsActions { pub toggle: bool, pub show_page: Option<u8> }
// toggle = (page == 0) != (current == 0); show_page = if page != 0 { Some(page) } else { None }
```

`src/player/logic/decoder.rs`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoderMode { AutoCopy, Auto, Sw, Hw, HwPlus }

pub struct DecoderChoice { pub label: &'static str, pub mpv_value: &'static str }
/// Chips shown in the More sheet: all except Auto, in declaration order.
pub const CHIPS: [DecoderMode; 4] = [DecoderMode::AutoCopy, DecoderMode::Sw, DecoderMode::Hw, DecoderMode::HwPlus];

impl DecoderMode {
    pub fn title(self) -> &'static str;       // "Auto", "Auto", "SW", "HW", "HW+"
    pub fn kotlin_value(self) -> &'static str;// "auto-copy", "auto", "no", "mediacodec-copy", "mediacodec"
}

/// Linux mapping (proposal, see p2-rust-design §8.1 table below)
pub fn linux_value(mode: DecoderMode, gpu: GpuKind) -> &'static str;
pub enum GpuKind { Intel, Amd, Nvidia, Other }

/// hwdec-current string -> chip; unknown -> None (the chip row shows no selection, no error)
pub fn from_hwdec_current(s: &str) -> Option<DecoderMode>;
```

Linux mapping table (proposal), used by `linux_value`:

| Chip | Intel / AMD | NVIDIA | Other |
|---|---|---|---|
| AutoCopy ("Auto") | `auto-copy` | `auto-copy` | `auto-copy` |
| Auto (hidden) | `auto` | `auto` | `auto` |
| SW | `no` | `no` | `no` |
| HW | `vaapi-copy` | `nvdec-copy` | `auto-copy` |
| HW+ | `vaapi` | `nvdec` | `auto` |

`from_hwdec_current`: `auto-copy` → AutoCopy; `no` → Sw; `vaapi-copy`, `vaapi`, `nvdec-copy`, `nvdec`, `vulkan-copy` and `mediacodec-copy` → Hw/HwPlus as appropriate (`*-copy` = Hw, non-copy = HwPlus); anything else → None. Fixes the Kotlin `first { }` throw (behavior §7.4).

---

## 9. Sheets: data flow and state

Shape (example, quality sheet):

```rust
pub struct QualityModel {
    pub is_loading: bool,
    pub hosters: Vec<HosterState>,      // from area:45 loader
    pub expanded: Vec<bool>,            // parallel to hosters
    pub selected: (usize, usize),       // (hoster idx, video idx); flat list uses hoster 0
    pub show_failed: bool,
    pub show_empty: bool,
    pub flat: bool,                     // hosters.len()==1 && name == NO_HOSTER_LIST && Ready
}
pub enum QualityEvent { ClickHoster(usize), ClickVideo(usize, usize) }
pub fn build_quality_sheet(model: &QualityModel, on: impl Fn(QualityEvent) + Clone + 'static) -> gtk::Box;
```

Events flow to area:45's `PlayerModel` (which owns state). Sheets are views that take a snapshot plus an event callback; they do not hold player state. The sheet is rebuilt when the model changes (`PlayerModel` emits `changed`), or updated in place with `gtk::ListBox` diffing (performance note in §12).

Decoder chip: `on_select(DecoderMode)` → `set_hwdec(m, mode, gpu)` (session only, no preference write, matching Kotlin; a parity note in p2-ui-parity.md).

Track sheets: `TrackSheet { kind: Audio | Subtitle, tracks: Vec<TrackInfo>, selected: SelectionModel }` where `SelectionModel` is `Single(i64)` for audio and `Slots([Option<i64>; 2])` for subtitles (primary, secondary).

Chapters: `ChapterRow { ordinal: usize, name: Option<String>, start_secs: f32 }`; ordinal computed at load (proposal §4.4 of behavior).

---

## 10. Concurrency model

- All GTK widgets and all `MpvProps` calls run on the GTK main thread.
- libmpv events arrive on libmpv's thread and are forwarded to the main context through area:45's `async_channel` (see area:45 §7). Property *changes* from mpv (for example the sub-delay changed by a keyboard shortcut) update the open panel through the same channel. Sheets subscribe to `PlayerModel` changes; they do not poll.
- Font scan: `std::thread::spawn` → `async_channel::Sender<Vec<String>>` → `glib::spawn_future_local` applies the list to the `StringList` of the font dropdown.
- Tap-sync: `glib::timeout_add_local(20 ms)`; ticks are cancelled on stop and when the panel closes (`SheetHost::close` drops the source).
- Animations: GTK's own frame clock (Revealer transitions, 350 ms). No manual timers for sheet animation.

---

## 11. Android API → Linux mapping (this part)

| Android / Compose | Linux / gtk4-rs | Notes |
|---|---|---|
| `PlayerSheet` (ModalBottomSheet-like with drag) | `SheetHost` overlay + `gtk::Revealer` + scrim | drag dismiss dropped (p2-ui-parity) |
| `AnchoredDraggableState`, `rememberSplineBasedDecay` | none | Android gesture physics |
| `BackHandler` | `gtk::EventControllerKey` Escape | global to the player window |
| `HorizontalPager` (3 cards) | `gtk::Stack` + `gtk::StackSwitcher` | desktop tabs instead of swipe |
| `TopAppBar` | `gtk::HeaderBar` inside the panel or a plain `gtk::Box` title | |
| `ConstraintLayout` `linkTo(bias)` | `gtk::Overlay` with `valign`/`halign` and `margin_top` | |
| `LazyColumn` / `LazyRow` | `gtk::ListBox` / `gtk::ListView` | |
| `FlowRow` | `gtk::FlowBox` | |
| `AnimatedVisibility` (fade + slide) | `gtk::Revealer` (`transition-type` `slide-down`, duration 250 ms) | |
| `animateContentSize` | `gtk::Expander` / `Revealer` | |
| `IconToggleButton` | `gtk::ToggleButton` with icon | |
| `FilterChip`, `InputChip` | chip-styled `gtk::ToggleButton` / box with close button | |
| `Checkbox`, `RadioButton` | `gtk::CheckButton` (group for radio) | |
| `DropdownMenu`, `ExposedTextDropDownMenu` | `gtk::DropDown` / `gtk::PopoverMenu` | |
| `Dialog(usePlatformDefaultWidth=false)` | `gtk::Window` modal + transient-for player | |
| `TimePicker` / `TimeInput` | two `gtk::SpinButton` (hours 0..99, minutes 0..59, wrap) | clock dial dropped (p2-ui-parity) |
| `OpenDocument` + `*/*` | `gtk::FileDialog::open` (no filter) | |
| `Intent.ACTION_SEND` share | `ashpd` `OpenURI` / `FileChooser` save | verify at implementation |
| `ActivityResultContracts` | `gtk::FileDialog` async (`open_future`) | |
| `HapticFeedback.TextHandleMove` | none | dropped (no haptics on desktop) |
| `DateUtils.formatElapsedTime` | `logic::number::format_elapsed` | pure |
| `Utils.prettyTime` (mpv-android) | `logic::number::pretty_time` | external source; see §11.1 |
| `TTFFile` (truetypeparser) | `ttf-parser` | §7 |
| `Color.parseColor` | `logic::color::parse_mpv_color` | returns Option |
| `Injekt.get<Preferences>()` | constructor injection of `&dyn Store` | |
| `collectAsState()` on preference | `Store` change subscription via `glib` signal (area:02 provides) | |
| `MPVLib.*` | `MpvProps` (§2) | |
| `Dispatchers.IO` | `std::thread` + `async_channel` | |
| `DISABLED_ALPHA` 0.38 | CSS `opacity: 0.38` on the dim label class | |
| `R.drawable.*` (outline_brand_family_24, sharp_shadow_24) | GTK icon names (`font-x-generic-symbolic`, `format-text-…`), bundled SVG if missing | |

### 11.1 Chapter time format

`is.xyz.mpv.Utils.prettyTime` is from an external artifact not in this repo. The port defines `pretty_time(s)` as `H:MM:SS` when `s >= 3600` else `M:SS` with zero-padded seconds (`01:05`), and must be checked against the artifact's output before release (flagged in p2-edge-cases-and-tests.md as an open verification item).

---

## 12. Performance notes

- `ChaptersSheet` builds rows with an ordinal computed once (§9); avoid `indexOf` per row.
- Quality sheet can be rebuilt on every hoster state change. Keep rebuild cost O(n) per change; n is small (hosters tens, videos hundreds at most).
- Font list: sort families case-insensitively for the dropdown? Kotlin keeps discovery order. Keep discovery order (parity).
- Speed presets: sorted once per change.

---

## 13. Pitfalls specific to this part

1. **Read-after-write ordering.** The Kotlin `LaunchedEffect(delay, secondaryDelay)` writes mpv after state changes. The port writes mpv in the same call that changes the model, never from a timer, to avoid a second write with a stale value.
2. **Read-back of `sub-speed` above 1.0.** Kotlin does not apply live speeds above 1.0 (behavior §11.2). The port applies every accepted value (0.1..10) live. This is a deliberate fix and must be recorded in the release notes.
3. **`af` clearing.** Never write `af=""` (behavior §7.3). Remove only the pan filter.
4. **Enum naming trap.** `AudioChannels::Auto` writes `auto-safe`; `AutoSafe` writes `auto`. The unit test in §8 pins this.
5. **Scrim must consume clicks.** `gtk::Overlay` lets clicks fall through unless the scrim claims them; use `gtk::GestureClick` with `set_state(Claimed)` on the card and the scrim close handler on the scrim.
6. **`gtk::SpinButton` clamps.** GTK clamps to `range` by default. For the unclamped `OutlinedNumericChooser` semantics, set the adjustment range to the full `f64` range and show the error text when outside `[min, max]`. Test this explicitly.
7. **`DropDown` string model.** `gtk::DropDown::set_model` needs a `gtk::StringList`; updating it in place re-selects item 0 unless `set_selected` is called after `splice`.
8. **libmpv2 errors vs NaN.** `get_f64` can return `NaN` for unset numeric properties. Treat non-finite values as `None`.
9. **`sub-scale = 0`.** Kotlin allows 0 (subtitles disappear). Keep the range, but show a hint label when 0.
10. **Duplicate chapters.** Use ordinal, not equality (§9).
11. **`Decoder` unknown value.** Never panic (`from_hwdec_current` returns None).
12. **Font families duplicated.** The Kotlin list can contain `Sans Serif` twice; the port dedupes against the default (documented deviation).
13. **Screenshot temp file race.** Use a unique file per capture (`<cache>/<epoch-ms>_shot.png`) and a final rename; the Kotlin fixed `mpv_screenshot.png` can race with concurrent Save and Share.

---

## 14. Integration with the existing Kurobayomi mockup

| Mockup element | Current code | After this part |
|---|---|---|
| Top-bar icons Subtitles / Audio / Qualities / More | `src/player.rs:184-191` calls `sheet("player", tap)` with spec ids `subtitle_tracks`, `audio_tracks`, `quality_tracks`, `more`; long-press `panel_subtitle_settings`, `panel_audio_delay`, `quality_tracks`, `panel_video_filters` | `SheetHost::open(SheetKind::SubtitleTracks, …)` etc.; long-press `open_panel(PanelKind::…)`. Note: the mockup's long-press on "Qualities" re-opens the same sheet (no panel exists; keep as-is or drop, see p2-ui-parity) |
| Speed chip | `src/player.rs:227` `sheet("player", "playback_speed")` long press | `SheetKind::PlaybackSpeed` |
| Chapters button | `src/player.rs:229` | `SheetKind::Chapters` |
| Screenshot on long press of surface | `src/player.rs:330` | `SheetKind::Screenshot` |
| Dialog `sleep_timer` | `dialog("player", "sleep_timer")` via spec | `sleep_timer.rs` window |
| `show_sheet(&Sheet)` modal window | `src/ui.rs:391-404` (`modal(&title, &stack, 520, 620)`) | reader keeps it; player uses `SheetHost` (in-player overlay, not a separate window) |
| `show_dialog` | `src/ui.rs:406-435` | reused for `set_art_confirm`, `episode_list`, `integer_picker` (part 1) |

The mockup's spec `screens[player].sheets` items are static (for example chapter "#1: Intro 00:00"); the port replaces them with `PlayerModel` data. Keep `spec/reader_player.json` as the layout reference until area:45 lands.

---

## 15. Test hooks

- `logic/*` modules are pure and compile without GTK (`cargo test --lib --no-default-features` if a feature gate is added; otherwise keep them in a module with no `gtk` imports).
- `MpvProps` fake records calls in a `Vec<(String, String)>` so tests can assert the exact property writes listed in p2-data-and-config.md §4.
- `Store` fake is a `HashMap<String, String>` with the stored-name enum rule.
- UI smoke: a headless `gtk::init` test is out of scope for v1; widgets are checked manually against p2-ui-parity.md.
