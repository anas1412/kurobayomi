# 46 Player controls UI, part 1: Rust design

Target: `/home/blackbox/kurobayomi` (crate `kurobayomi`), GTK 4.16+ through `gtk4` 0.11 (`gtk = { package = "gtk4", version = "0.11.5", features = ["v4_16"] }` in `Cargo.toml`). No libadwaita. Behaviour source: `p1-behavior.md`. Data: `p1-data-and-config.md`.

Design rule: the control logic is pure Rust with no GTK types, so it is unit-testable without a display. GTK widgets only render state and forward input.

---

## 1. Module layout

Current mockup: `src/player.rs` (337 lines, one `reader()` and one `player()` function, placeholder surface). New layout (proposal):

```
src/player/mod.rs                 pub fn player(title, episode) -> GBox   (replaces src/player.rs player(), keeps reader in src/reader/)
src/player/model.rs               PURE: ControlsModel, Sheet/Panel/Dialog/Update enums, host rules
src/player/math.rs                PURE: gesture math, zone(), speed/aspect cycles, volume icon/texts, brightness icon
src/player/time.rs                PURE: pretty_time(), pretty_time_signed(), format_episode_number()
src/player/segments.rs            PURE: build_seek_segments(), current_chapter(), chapter_label()
src/player/repeat.rs              PURE: RepeatSchedule iterator (750 -> 5 ms, decay 0.25)
src/player/settings.rs            PlayerSettings struct with serde defaults = p1-data-and-config.md section 3
src/player/backend.rs             MpvCmd enum, MpvHandle (command queue), property/event types
src/player/controls/mod.rs        GTK assembly: overlay, visibility wiring, host routing
src/player/controls/overlay.rs    root gtk::Overlay build, veil CSS, placement, animations
src/player/controls/gestures.rs   GestureClick / GestureDrag / GestureLongPress wiring to ControlsModel
src/player/controls/seekbar.rs    SeekBar GObject subclass (GtkWidget, snapshot-drawn)
src/player/controls/transport.rs  centre play/prev/next + gesture seek text
src/player/controls/top.rs        top-left (back + titles) and top-right (auto-play, subtitles, audio, quality, more)
src/player/controls/bottom.rs     bottom-left (lock, rotate, speed, chapter pill) and bottom-right (primary button, PiP, aspect)
src/player/controls/buttons.rs    ControlsButton / FilledControlsButton equivalents, CSS classes
src/player/controls/gauge.rs      VerticalGauge widget (overflow segment), used by volume and brightness pills
src/player/controls/volume.rs     VolumePill (device + boost) and SystemVolume trait
src/player/controls/brightness.rs BrightnessPill and SystemBrightness trait
src/player/controls/veil.rs       BrightnessVeil (software dimming for negative brightness)
src/player/controls/oval.rs       RightOval/LeftOval cairo paths, DoubleTapTriangles animation
src/player/controls/toast.rs      PlayerUpdate pill (replaces mockup pill())
src/player/controls/thumbnail.rs  ThumbnailPreview popup, tile LRU
src/player/controls/sheet.rs      PlayerSheet container (modal window or revealer), drag-to-dismiss
src/player/controls/panels.rs     panel host (gtk::Stack, SlideLeft), AudioDelayPanel, shared DelayCard
src/player/controls/dialogs/mod.rs
src/player/controls/dialogs/shell.rs        PlayerDialog shell
src/player/controls/dialogs/episode_list.rs EpisodeListDialog
src/player/controls/dialogs/integer_picker.rs IntegerPickerDialog
src/player/controls/widgets.rs    SwitchRow, SliderRow, TintedSlider, ExpandableCard, DropDownRow
src/player/controls/numeric.rs    NumericChooser (Int and f64), RepeatingButton
```

Pure modules (`model`, `math`, `time`, `segments`, `repeat`, `settings`) compile without GTK and have `#[cfg(test)]` unit tests (T-46a-056).

Mockup items that move: `fmt_time` (src/player.rs:150-152) becomes `time::pretty_time`; `pill()` (src/player.rs:40-47) becomes `toast::flash`; `on_long_press` (src/player.rs:26-30) becomes `gestures::long_press_timer` (see pitfalls, section 9); `sheet()` and `dialog()` helpers (src/player.rs:7-17) become `host::open_sheet` and `host::open_dialog` that enforce the mutual-exclusion rules.

---

## 2. Pure model

### 2.1 Enums (src/player/model.rs)

```rust
// Mirrors PlayerEnums.kt. Names are the Aniyomi names. Ordinal order is not persisted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sheet { #[default] None, PlaybackSpeed, SubtitleTracks, AudioTracks, QualityTracks, Chapters, More, Screenshot }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Panel { #[default] None, SubtitleSettings, SubtitleDelay, AudioDelay, VideoFilters }

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Dialog { #[default] None, EpisodeList, IntegerPicker(IntegerPickerSpec) }

#[derive(Clone, Debug, PartialEq, Default)]
pub enum PlayerUpdate { #[default] None, DoubleSpeed, AspectRatio, Text(String) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoAspect { Crop, Fit, Stretch }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingleAction { None, Seek, PlayPause, Switch, Custom }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoubleTapZone { Left, Center, Right }

#[derive(Clone, Debug, PartialEq)]
pub struct IntegerPickerSpec {
    pub default: i64,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub name_format: String,   // Java format string, e.g. "%d ms"
    pub title: String,
    pub property: String,      // mpv property to write on commit
}
```

`VideoAspect` `Crop`, `Fit`, `Stretch` map to the stored preference strings `"Crop"`, `"Fit"`, `"Stretch"` (`serde` with the variant name, no rename).

### 2.2 Controls state machine (src/player/model.rs)

```rust
pub const AUTO_HIDE_MS_DEFAULT: u32 = 4000;     // pref_player_time_to_disappear
pub const VOLUME_HIDE_MS: u32 = 2000;
pub const BRIGHTNESS_HIDE_MS: u32 = 2000;
pub const PLAYER_UPDATE_MS: u32 = 2000;
pub const DOUBLE_TAP_CLEAR_MS: u32 = 800;       // then +100 ms to hide seekbar
pub const DOUBLE_TAP_HIDE_SEEKBAR_MS: u32 = 100;

/// Pure visibility and host state. No timers here; the GTK layer owns the glib sources
/// and calls `on_timer(kind)` when they fire.
#[derive(Clone, Debug)]
pub struct ControlsModel {
    pub controls_shown: bool,       // starts true (pref_hide_controls = false)
    pub seekbar_shown: bool,        // starts true
    pub locked: bool,               // starts false, never persisted
    pub paused: bool,
    pub is_seeking: bool,           // seekbar drag in progress
    pub gesture_seek: Option<(i64, i64)>, // (start_sec, delta_sec)
    pub sheet: Sheet,
    pub panel: Panel,
    pub dialog: Dialog,
    pub panel_gestures_allowed: bool, // pref_allow_gestures_in_panels
    pub auto_hide_ms: u32,            // pref_player_time_to_disappear
    pub reset_epoch: u64,             // bumped on every control click (LocalPlayerButtonsClickEvent)
    pub hide_timer_armed: bool,       // derived; GTK arms/disarms glib source to match
}

pub enum Effect { ArmAutoHide(u32), DisarmAutoHide, ShowStatusBar, HideStatusBar }

impl ControlsModel {
    /// VM.showControls(): refuse while a sheet, panel or dialog is open.
    pub fn show_controls(&mut self) -> bool;
    /// VM.hideControls()
    pub fn hide_controls(&mut self);
    /// Single tap: toggles (GH:124-126). Returns whether controls are now shown.
    pub fn tap(&mut self) -> bool;
    /// Any control button click (ControlsButton, AutoPlaySwitch, SeekBar timer): bumps reset_epoch.
    pub fn control_clicked(&mut self);
    /// Recomputes whether the auto-hide timer should be armed:
    /// armed iff controls_shown && !paused && !is_seeking. The GTK layer arms with auto_hide_ms
    /// whenever the armed-state or reset_epoch changes (LaunchedEffect keys, PC:134-144).
    pub fn auto_hide_wanted(&self) -> bool;
    pub fn on_auto_hide_fired(&mut self);     // -> hide_controls()
    pub fn set_sheet(&mut self, s: Sheet);    // VM.showSheet rules (VM:709-719)
    pub fn set_panel(&mut self, p: Panel);    // VM.showPanel rules (VM:721-730)
    pub fn set_dialog(&mut self, d: Dialog);  // VM.showDialog rules (VM:732-741)
    pub fn lock(&mut self);
    pub fn unlock(&mut self);
}
```

Host rules (verbatim from VM):
- `set_sheet(s)`: sheet = s; if `s == None` → show_controls; else hide_controls, panel = None, dialog = None.
- `set_panel(p)`: panel = p; if `p == None` → show_controls; else hide_controls, sheet = None, dialog = None.
- `set_dialog(d)`: dialog = d; if `d == None` → show_controls; else hide_controls, sheet = None, panel = None.
- `show_controls` returns false and changes nothing if `sheet != None || panel != None || dialog != None`.
- `show_seekbar()` returns if `sheet != None` (VM:688-691).

### 2.3 Math (src/player/math.rs)

```rust
/// Kotlin `Int.toInt()` on Float truncates toward zero; Rust `as i32` does the same for in-range values.
pub fn vertical_step_i32(orig: i32, start_y: f32, new_y: f32, sens: f32) -> i32 {
    orig + ((start_y - new_y) * sens) as i32
}
pub fn vertical_step_f32(orig: f32, start_y: f32, new_y: f32, sens: f32) -> f32 {
    orig + (start_y - new_y) * sens
}
pub fn horizontal_step_i32(orig: i32, start_x: f32, new_x: f32, sens: f32) -> i32 {
    orig + ((new_x - start_x) * sens) as i32
}

pub const SEEK_SENSITIVITY: f32 = 0.15;       // seconds per px
pub const BRIGHTNESS_SENSITIVITY: f32 = 0.001;
pub const VOLUME_SENSITIVITY_PER_STEP: f32 = 0.001; // multiplied by max_volume
pub const BOOST_SENSITIVITY_PER_PERCENT: f32 = 0.001; // multiplied by boost cap

pub fn zone(x: f64, width: f64) -> DoubleTapZone {
    if x > width * 3.0 / 5.0 { DoubleTapZone::Right }
    else if x < width * 2.0 / 5.0 { DoubleTapZone::Left }
    else { DoubleTapZone::Center }
}

/// Vertical drag target: brightness on the left half unless swapped (GH:295-299).
pub fn drag_target(x: f64, width: f64, swapped: bool) -> DragTarget {
    let right_half = x > width / 2.0;
    if right_half == swapped { DragTarget::Brightness } else { DragTarget::Volume }
}
// check: swapped=false: right_half=false -> (false==false) true -> Brightness, right_half=true -> Volume. Matches GH:298.
// swapped=true: right_half=true -> (true==true) -> Brightness, false -> Volume. Matches GH:296.
// Note: Kotlin uses `x > width/2` for swapped and `x < width/2` for not swapped. At exactly x == width/2
// Kotlin non-swapped chooses volume (x<w/2 false), this function chooses Volume too. Kotlin swapped at x == w/2
// chooses volume (x > w/2 false). Matches.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragTarget { Volume, Brightness }

/// Speed cycle, BottomLeftPlayerControls.kt:69.
pub fn next_speed(current: f32) -> f32 { if current >= 2.0 { 0.25 } else { current + 0.25 } }

/// Aspect cycle, PC:512-520.
pub fn next_aspect(a: VideoAspect) -> VideoAspect {
    match a { VideoAspect::Fit => VideoAspect::Stretch, VideoAspect::Stretch => VideoAspect::Crop, VideoAspect::Crop => VideoAspect::Fit }
}

/// Volume icon by rounded percentage, first match wins (VerticalSliders.kt:224-233).
pub fn volume_icon(percent: i32) -> VolumeIcon {
    match percent {
        0 => VolumeIcon::Off,
        0..=30 => VolumeIcon::Mute,
        31..=60 => VolumeIcon::Down,
        61..=100 => VolumeIcon::Up,
        _ => VolumeIcon::Off,
    }
}
pub enum VolumeIcon { Off, Mute, Down, Up }

/// Brightness icon by clamped fraction, first match wins (VerticalSliders.kt:163-169).
pub fn brightness_icon(frac: f32) -> BrightnessIcon {
    if frac <= 0.0 { BrightnessIcon::Night }
    else if frac <= 0.3 { BrightnessIcon::Low }
    else if frac <= 0.6 { BrightnessIcon::Medium }
    else { BrightnessIcon::High }
}
pub enum BrightnessIcon { Night, Low, Medium, High }
```

Note on `brightness_icon`: Kotlin `in 0f..0.3f` is inclusive on both ends, so `0.3` is `Low`, and `0.6` is `Medium`. The code above matches that. `frac` is clamped to 0..1 before the call, so `in -1f..0f` means exactly `0`.

Volume text and boost text (`getVolumeSliderText`, VerticalSliders.kt:237-257):

```rust
pub fn volume_texts(volume: i32, mpv_volume: i32, percent: i32, as_percent: bool) -> (String, String) {
    let device = if as_percent { format!("{percent}%") } else { volume.to_string() };
    let boost_v = mpv_volume - 100;
    let boost = match boost_v {
        0 => String::new(),
        1..=1000 => format!("+{boost_v}"),
        -100..=-1 => format!("-{}", -boost_v),
        _ => boost_v.to_string(),
    };
    (device, boost)
}
```

Note: Kotlin uses `NumberFormat` with the device locale, so thousands separators and the percent symbol are locale-dependent. Proposal: use `format!` for the integers and `format!("{percent}%")`; the grouping differences only matter above 999 and are not reachable for `volume` (max 15 typically). Document that the percent format differs for locales with a space before `%` (for example French). Acceptable deviation, listed in `p1-ui-parity.md`.

Time (src/player/time.rs):

```rust
pub fn pretty_time(d: i64) -> String {
    let hours = d / 3600;
    let minutes = d % 3600 / 60;
    let seconds = d % 60;
    if hours == 0 { format!("{minutes:02}:{seconds:02}") } else { format!("{hours}:{minutes:02}:{seconds:02}") }
}
pub fn pretty_time_signed(d: i64) -> String {
    format!("{}{}", if d >= 0 { '+' } else { '-' }, pretty_time(d.abs()))
}
```

Caveat: for negative `d` without sign, Kotlin's `%` keeps the sign of the dividend (`-70 % 60 = -10`), so the non-signed path with negative input yields odd output. Callers only pass non-negative to `pretty_time` (Kotlin `abs` in gesture text, `toInt()` on non-negative positions). `pretty_time` for negative input: Rust `%` has the same sign behaviour, so results match Kotlin; document, do not change.

`format_episode_number(v: f64) -> String`: emulate `DecimalFormat("#.###")`: round half-even to 3 decimals, strip trailing zeros, no leading zero for |v| < 1. Implement with `format!("{:.3}", v)`, round-half-even is not what Rust does (Rust uses round-half-to-even for `{:.3}`? Rust's float formatting uses round-half-to-even on the exact decimal value; DecimalFormat HALF_EVEN on the binary value). Decision: implement as `let s = format!("{:.3}", v); trim trailing zeros and '.'; strip leading "0" when followed by '.'`. Test vectors: 1.0 → "1", 12.5 → "12.5", 1.2345 → "1.234" or "1.235" (check; E-46-022 expects HALF_EVEN result "1.234"), 0.5 → "0.5" by Rust vs ".5" by Kotlin: decide Kurobayomi shows "0.5" and record deviation in parity doc.

Segments (src/player/segments.rs):

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Chapter { pub name: String, pub start: f32, pub chapter_type: ChapterType }

/// Seekbar ticks. Fixes the duplicated list in SeekBar.kt:119-129 (see E-46-031).
pub fn seek_segments(chapters: &[Chapter], duration: f32) -> Vec<Segment> {
    let mut out: Vec<Segment> = chapters.iter()
        .filter(|c| (0.0..=duration).contains(&c.start))
        .map(|c| Segment { name: c.name.clone(), start: c.start })
        .collect();
    if !out.is_empty() && out[0].start != 0.0 {
        out.insert(0, Segment { name: String::new(), start: 0.0 });
    }
    out
}

/// PlayerViewModel.getCurrentChapter(position) (VM:2150-2155): last chapter with start <= position.
pub fn current_chapter(chapters: &[Chapter], pos: f32) -> Option<usize> {
    chapters.iter().enumerate().filter(|(_, c)| c.start <= pos)
        .max_by(|a, b| a.1.start.total_cmp(&b.1.start)).map(|(i, _)| i)
}
```

Subtle: Kotlin `maxByOrNull` returns the first maximal element on ties (`maxBy` keeps the first when equal? Kotlin `maxBy` returns the first element with the max value). Rust `max_by` returns the LAST maximal element on ties. To match Kotlin, use `fold` keeping the first on ties. Include this in code:

```rust
pub fn current_chapter(chapters: &[Chapter], pos: f32) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, c) in chapters.iter().enumerate() {
        if c.start <= pos {
            match best {
                None => best = Some(i),
                Some(b) if c.start > chapters[b].start => best = Some(i),   // strict > keeps the first on ties
                _ => {}
            }
        }
    }
    best
}
```

`ThumbnailPreview` chapter label uses `lastOrNull { start <= pos }` (ThumbnailPreview.kt:72), which is the LAST in list order, not max start. For sorted chapters they are identical; define `chapter_label_at(chapters, pos)` as `chapters.iter().rev().find(|c| c.start <= pos)`, kept distinct to match the Kotlin, and note the difference when unsorted (E-46-034).

Repeat schedule (src/player/repeat.rs):

```rust
pub struct RepeatSchedule { next_ms: u64, min_ms: u64, decay: f64 }
impl RepeatSchedule {
    pub fn new(max_ms: u64, min_ms: u64, decay: f64) -> Self { Self { next_ms: max_ms, min_ms, decay } }
    /// Returns the delay to wait before the next fire, then decays it.
    pub fn next_delay(&mut self) -> u64 {
        let d = self.next_ms;
        let decayed = (d as f64 - d as f64 * self.decay) as u64;   // Kotlin: (cur - cur*f).toLong()
        self.next_ms = decayed.max(self.min_ms);
        d
    }
}
```

Note the Kotlin sequence starts with the immediate call, then `delay(current)` then decays. So "fire now; then wait `next_delay()` before the second fire". Implement as: fire on press; then loop: `wait(sched.next_delay()); fire();`. Check: Kotlin loop: call (t=0); delay(750); decay → 562; call (t=750); delay(562)... In Kotlin: at iteration 1, `currentDelay = 750`, call, `delay(750)`, then `currentDelay = decayed(750) = 562`. Iteration 2: call (t=750), delay(562), decay → 421. So the waits are 750, 562, 421, ... before the 2nd, 3rd, ... calls. My `next_delay()` returns the pre-decay value `d` and sets next to decayed: first call returns 750 and sets 562. Matches.

Expected sequence: waits 750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5, 5, ... Test with assertion.

Settings (src/player/settings.rs): a `PlayerSettings` with `#[serde(default = ...)]` fields exactly matching p1-data-and-config.md section 3, including `skip_length_secs: u32 = 10`, `time_to_disappear_ms: u32 = 4000`, `panel_opacity_pct: u32 = 60`, `volume_boost_cap: u32 = 30`, `display_volume_as_percent: bool = true`, `invert_duration: bool = false`, `aspect: VideoAspect = Fit`, `left_double_tap: SingleAction = Seek`, `center_double_tap: SingleAction = PlayPause`, `right_double_tap: SingleAction = Seek`, `gesture_volume_brightness: bool = true`, `swap_volume_brightness: bool = false`, `gesture_horizontal_seek: bool = true`, `show_seekbar: bool = false`, `smooth_seek: bool = false`, `show_loading: bool = true`, `show_current_chapter: bool = true`, `reduce_motion: bool = false`, `allow_gestures_in_panels: bool = false`, `enable_pip: bool = true`, `speed: f32 = 1.0`, `auto_play: bool = false`, `audio_delay_ms: i32 = 0`, `enable_skip_intro: bool = true`, `auto_skip_intro: bool = false`, `netflix_style_skip: bool = false`, `waiting_time_skip_secs: u32 = 5`, `screenshot_subtitles: bool = false`, `show_status_bar: bool = false`.

Serde with `rename_all` not used; keys are the Kotlin keys in a TOML table so a future import from Aniyomi backups can map them (area 21-22).

---

## 3. Backend boundary (src/player/backend.rs)

```rust
/// Commands the UI sends to the mpv thread. Every command is asynchronous (fire-and-forget)
/// except `Query`, which replies on a oneshot channel.
pub enum MpvCmd {
    Pause(bool),
    TogglePause,
    SeekRelative { secs: f64, exact: bool },
    SeekAbsolute { secs: f64, exact: bool },
    SetSpeed(f64),
    SetVolumeMpv(i64),          // mpv `volume`, 100..=100+cap
    SetAudioDelay(f64),         // seconds
    SetAspect { panscan: f64, aspect_override: f64 },
    KeyPress(String),           // custom double-tap key codes "0x10001", "0x10002", "0x10003"
    ScriptMessage(String),      // custom button: "call_button_<id>" / "_long"
    Screenshot { path: std::path::PathBuf, subtitles: bool },
    Query(Query, tokio::sync::oneshot::Sender<QueryResult>),
}

/// Property changes flow the other way on a glib channel.
pub enum MpvEvent {
    Position(f64),
    Duration(f64),
    Pause(bool),
    Speed(f64),
    Volume(i64),          // mpv volume
    CacheReadAhead(f64),  // seconds ahead
    ChaptersChanged,
    EndFile,
}

pub struct MpvHandle {
    tx: std::sync::mpsc::Sender<MpvCmd>,
}

impl MpvHandle {
    pub fn send(&self, cmd: MpvCmd) { let _ = self.tx.send(cmd); }   // never blocks the GTK thread
}
```

Ordering and blocking rules:
- The mpv thread owns the `mpv_handle`. It drains `rx` in a loop, interleaved with `mpv_wait_event(timeout)`.
- `send` never blocks; the Kotlin synchronous `MPVLib` calls become queued. Consequence: the seekbar commit `SeekAbsolute` is delivered in order with the `Pause` command.
- `Query` is the only blocking path, and it is used off the GTK thread only.

Event fan-in: mpv thread → `glib::MainContext::channel::<MpvEvent>(glib::Priority::DEFAULT)` → GTK main thread updates `ControlsModel` and widgets. Sender is `Send`, receiver stays on main.

Ownership: `MpvHandle` is stored in the player `Rc<PlayerState>` on the GTK thread. The mpv thread is joined on player close.

Position polling: none. Use mpv property observation (`observe_property`) for `time-pos` at 10 Hz via `mpv_observe_property` with `time-pos` (mpv pushes changes). Proposal: throttle UI redraw of the seekbar to 30 fps while playing (`glib::timeout` not needed if the widget only queues draw on change, use `queue_draw` with a frame clock).

---

## 4. GTK widget tree

```
PlayerRoot (gtk::Box, vertical, vexpand)
└── gtk::Overlay                                     [overlay.rs]
    ├── child: gtk::Stack or gtk::Picture           video surface (area 45 provides the GL area)
    ├── overlay: GestureSurface (gtk::Box, hexpand, vexpand, transparent)   [gestures.rs]
    ├── overlay: DoubleTapOvals (gtk::DrawingArea + triangles)              [oval.rs]
    ├── overlay: Chrome (gtk::Box, halign fill, valign fill, veil CSS)       [overlay.rs]
    │   ├── TopBar (gtk::Overlay-like layout via gtk::CenterBox or gtk::Box)  [top.rs]
    │   ├── CentreTransport                                                   [transport.rs]
    │   ├── Gauges: VolumePill (left or right), BrightnessPill                  [volume.rs, brightness.rs]
    │   ├── UpdatePill                                                         [toast.rs]
    │   ├── UnlockButton                                                       [overlay.rs]
    │   ├── ThumbnailPreview (above seekbar)                                  [thumbnail.rs]
    │   └── BottomArea: BottomRow (left, right) + SeekBarRow                   [bottom.rs, seekbar.rs]
    ├── overlay: BrightnessVeil (no input)                                    [veil.rs]
    └── overlay: PanelHost (gtk::Stack, end-aligned, no input when None)     [panels.rs]
Sheets: separate modal gtk::Window per open sheet (sheet.rs), transient for the player window.
Dialogs: separate modal gtk::Window (dialogs/shell.rs).
```

Layout mapping (behaviour section 1.5): the ConstraintLayout geometry becomes a `gtk::Overlay` whose children carry `halign`/`valign`/`margin_*`. Use these exact margins (16 px for `medium`):
- Top bar: `valign = Start`, `margin_top = 16`, `margin_start = 16`, `margin_end = 16`.
- Bottom area: `valign = End`, `margin_bottom = 16`, `margin_start/end = 16`.
- Seekbar: inside the bottom area, below the bottom row, `margin_top = 0`.
- Volume pill: `halign = Start`, `valign = Center`, `margin_start = 16`. Brightness pill: `halign = End`, `valign = Center`, `margin_end = 16`. Swap = swap halign.
- Update pill: `halign = Center`, `valign = Start` with `margin_top = 0.2 * overlay_height` implemented as a custom `measure`/`allocate` (`gtk::LayoutManager`) or as a box with `valign = Fill` and a spacer `vexpand` of ratio 0.2:1.8 (proposal: custom `gtk::LayoutManager` that places the child at `y = 0.2 * (H - h)`, which matches the ConstraintLayout bias formula).

Tokens:

```css
/* share/kurobayomi/player.css, loaded with gtk::CssProvider for the player window */
.kuro-veil { background-image: linear-gradient(to bottom, rgba(0,0,0,1) 0%, rgba(0,0,0,0) 20%, rgba(0,0,0,0) 70%, rgba(0,0,0,1) 100%); transition: opacity 300ms ease-in-out; }
.kuro-veil.shown { opacity: 0.8; }
.kuro-veil.hidden { opacity: 0; }
.kuro-update-pill { background-color: rgba(0,0,0,0.4); border-radius: 16px; padding: 8px 16px; color: white; }
.kuro-chapter-pill { background-color: alpha(@window_bg_color, 0.6); border-radius: 999px; padding: 8px 12px; }
.kuro-osd-text { color: white; }
.kuro-disabled-icon { opacity: 0.38; }
.kuro-gauge-track { background-color: @window_bg_color; border-radius: 16px; }
.kuro-gauge-fill { background-color: @accent_color; }        /* Android tertiary */
.kuro-gauge-overflow { background-color: @error_bg_color; }  /* errorContainer */
.kuro-panel-card { background-color: alpha(@window_bg_color, var(--panel-opacity, 0.6)); }
```

Note: GTK CSS does not support `transition` on `opacity` for every widget class reliably; the animation constants come from `model` (300 ms exit / 100 ms enter), and the code sets `opacity` through `gtk::Widget::set_opacity` with a `glib::timeout` ticker or `adw::TimedAnimation` (not allowed, no libadwaita). Proposal: a 60 Hz `add_tick_callback` interpolator in `overlay.rs` (`fn animate(widget, from, to, ms, easing)`), which avoids CSS transition issues. Easing: FastOutSlowIn approximated by cubic-bezier(0.4, 0, 0.2, 1); LinearOutSlowIn by cubic-bezier(0, 0, 0.2, 1).

Theme: all colours come from GTK named colours so the user's GTK theme (Tokyo Night) applies (memory note). Do not hard-code colours except the veil black, the white text (video overlay on black), and the oval white at alpha 0.2.

---

## 5. Gesture implementation (src/player/controls/gestures.rs)

GTK controllers attached to `GestureSurface` (a `gtk::Box` that expands):

| Behaviour (behaviour section) | GTK controller | Notes |
|---|---|---|
| Tap toggle (2.2) | `gtk::GestureClick` `released` with `n_press == 1` | Ignore if the release was a double-click (n_press == 2 arrives separately) |
| Double-tap zones (2.4) | `gtk::GestureClick` `pressed` with `n_press == 2` | Use `x` from the event; `width` from `widget.width()` |
| Long press (2.5) | `gtk::GestureLongPress` (`pressed` after delay) and `gtk::GestureClick` `released` to cancel | See pitfall P-4 for mouse |
| Horizontal drag (2.6) | `gtk::GestureDrag` `drag-begin` / `drag-update` / `drag-end` | Compute `new_x = start_x + offset_x` |
| Vertical drag (2.7) | second `gtk::GestureDrag` | Different instance so both axes do not conflict; use `drag-update` `offset_y` deltas |

Drag math for GTK: GTK `GestureDrag` reports `offset` from the press point, and `drag-begin` gives the start (x, y). The Kotlin `detectHorizontalDragGestures` starts after touch slop and reports `change.position.x` absolute. Use `start_x = begin.x` (not `begin.x + offset` after the threshold), so the first pixels are not lost. This changes the first step versus Android (which loses the slop distance); document as a deliberate improvement in parity.

Pure functions in `math.rs` take `(start, now)` pairs, so `gestures.rs` only translates GTK events into those.

Pseudo-code for the horizontal drag (matches 2.6):

```rust
// drag-begin
st.start_pos_secs = backend_pos.floor() as i64;   // position.toInt()
st.start_x = begin_x as f32;
st.was_paused = model.paused;
model.is_seeking = true;

// drag-update (offset_x)
let x = st.start_x + offset_x as f32;
if backend_pos <= 0.0 && offset_x < 0.0 { return; }
if backend_pos >= duration && offset_x > 0.0 { return; }
let new_pos = horizontal_step_i32(st.start_pos_secs as i32, st.start_x, x, SEEK_SENSITIVITY) as i64;
let delta = (new_pos - st.start_pos_secs).clamp(-st.start_pos_secs, duration as i64 - st.start_pos_secs);
model.gesture_seek = Some((st.start_pos_secs, delta));
seekbar.set_seek_preview(new_pos.clamp(0, duration) as f64);
if prefs.show_seekbar { model.show_seekbar(); }

// drag-end
backend.send(SeekAbsolute { secs: seek_pos.clamp(0.0, duration).trunc(), exact: prefs.smooth_seek });
model.is_seeking = false;
model.gesture_seek = None;
model.hide_seekbar();
// no unpause: Kotlin's unpause here is a no-op (see behaviour 2.6)
```

Note the `(new_pos - start).clamp(-start, duration - start)` matches Kotlin `coerceIn(0 - start, (duration - start).toInt())`.

Tap on locked: `tap()` still toggles `controls_shown` (behaviour 1.7). Double tap, long press and both drags early-return when `model.locked` (same as Kotlin).

Long-press: on press start schedule `LONG_PRESS_MS` (GTK setting `gtk-long-press-time`, default 500). On fire: `if locked return; if !is_long_pressing { is_long_pressing = true; backend.send(Pause(true)); model.set_sheet(Sheet::Screenshot); }`. On release: if `is_long_pressing` then `is_long_pressing = false; model.playerUpdate = None` (do NOT restore speed, see behaviour 2.3 quirk).

Double-tap: on the second press of a double-click, the zone decides the action:
- Right / Left with `seek` action: `if !is_seeking_forwards { seek_amount = 0 }; rightSeek/leftSeek`; set `is_double_tap_seeking = true`.
- Centre: `handle_center()` only.
- Repeat presses while `is_double_tap_seeking` (GH:148-157) call the same zone handler without the reset for the same direction. Implemented in `double_tap::handle(zone, state)`.

---

## 6. Seekbar (src/player/controls/seekbar.rs)

GObject subclass `SeekBar` (`gtk::Widget`), snapshot-based:

```rust
mod imp {
    use super::*;
    #[derive(Default)]
    pub struct SeekBar {
        pub duration: std::cell::Cell<f64>,
        pub position: std::cell::Cell<f64>,       // player position while not seeking
        pub seek_preview: std::cell::Cell<Option<f64>>, // gesture or drag position
        pub read_ahead: std::cell::Cell<f64>,
        pub segments: std::cell::RefCell<Vec<Segment>>,
        pub dragging: std::cell::Cell<bool>,
    }
    #[glib::object_subclass]
    impl ObjectSubclass for SeekBar { const NAME: &'static str = "KuroSeekBar"; type Type = super::SeekBar; type ParentType = gtk::Widget; }
    impl ObjectImpl for SeekBar {
        fn signals() -> &'static [glib::subclass::Signal] {
            static S: std::sync::OnceLock<Vec<glib::subclass::Signal>> = std::sync::OnceLock::new();
            S.get_or_init(|| vec![
                glib::subclass::Signal::builder("value-changed").param_types([f64::static_type()]).build(),
                glib::subclass::Signal::builder("seek-committed").param_types([f64::static_type()]).build(),
            ])
        }
    }
    impl WidgetImpl for SeekBar { fn snapshot(&self, s: &gtk::Snapshot) { /* draw track, read-ahead, progress, ticks, thumb */ } }
}
```

Behaviour contract (from behaviour section 6):
- `value = seek_preview.unwrap_or(position).clamp(0, duration)`.
- Drag start: `dragging = true`. Drag update: `value-changed(v)` (Kotlin `onValueChange`). Drag end: `seek-committed(v)` (Kotlin `onValueChangeFinished` with the last internal value).
- Click on track without drag: GTK `GestureClick` `pressed` jumps the thumb to the click x (Android `Slider` jumps too). Emit `value-changed` and `seek-committed` immediately.
- Ticks: draw one tick per segment at `start / duration` fraction, from `seek_segments(...)` (fixed list, no duplicates).
- Read-ahead: fill from progress to `read_ahead` in `inversePrimary` (`@accent_color` alpha 0.5, proposal).
- Height: `48` px row (Kotlin Row height), track thickness 4 px, thumb 12 px.
- Timers: two `gtk::Label` (92 px fixed width, `width_chars` 8 with `xalign` 0.5) to the left and right of the widget, each wrapped in a `gtk::Button` flat (duration toggles `invert_duration`).

Accessibility: `gtk::Accessible` role `slider` with `accessible-label` "Seek" (proposal, T-46a-054).

---

## 7. Sheets, panels and dialogs (host.rs under controls)

### 7.1 Sheet container

`PlayerSheet` in GTK: prefer a modal `gtk::Window` (`set_transient_for(player)`, `set_modal(true)`, `set_decorated(false)`, `set_resizable(false)`, `set_default_size(420, -1)` or 720 in landscape; `set_size_request` capped at 95% monitor height).

Reasons: a separate window gets native Wayland/X11 stacking, takes keyboard focus for Escape, and avoids clipping the 420 px sheet inside the video overlay. Drawback: the scrim must be a window opacity (`set_opacity`) or a CSS overlay in the player window. Proposal: keep the scrim in the player window as a `gtk::Box` with `.kuro-scrim` background alpha 0.5 animated by the tick interpolator (350 ms), and place the sheet as a bottom-aligned `gtk::Revealer` (`slide-up`, 350 ms) inside the same overlay. This keeps one window and gives exact control of the 350 ms and drag behaviour. Decision recorded in p1-ui-parity.md.

Drag to dismiss: `gtk::GestureDrag` on the sheet header and body. Positional threshold 56 px; velocity threshold 125 px/s (Kotlin 125 dp per second, converted to px at 1 px per dp, proposal). Snap animation 350 ms.

### 7.2 Host rules

```rust
pub struct Host { model: Rc<RefCell<ControlsModel>>, revealers: Sheets, ... }
impl Host {
    pub fn open_sheet(&self, s: Sheet) { let mut m = self.model.borrow_mut(); m.set_sheet(s); self.render(&m); }
    pub fn dismiss_sheet(&self) { ... } // dismissSheet flag then None (VM:701-705)
}
```

`render()` is the only place that changes widget visibility, so the rules live in one place.

### 7.3 Panels

`gtk::Stack` with `StackTransitionType::SlideLeft` (enter from the right, approximates `slideInHorizontally { it/3 }` plus fade), transition duration 300 ms. Child names are the `Panel` variants. Card width `max_width_request(420)`. Opacity `panel_opacity_pct / 100`.

### 7.4 Dialogs

`dialogs/shell.rs` builds a modal `gtk::Window` with a title label (`title-2` CSS class), a content box and an optional action row. Escape closes (key controller on the window). Click outside: the window is decorated=false, so a click on the parent scrim closes it: the player scrim `GestureClick` calls `dismiss`. Only the `IntegerPicker` and `EpisodeList` use it.

`IntegerPicker` (behaviour 10.3) uses `gtk::ListView` with a `SingleSelection` model (wheel replacement), initial selection = index of `default`, else 0. Commit on close: write `values[selection]` (not a separate `newValue` variable; fixes E-46-027).

---

## 8. Numeric chooser and repeat button (numeric.rs)

`RepeatButton` (GObject `gtk::Button` subclass or a `gtk::Button` plus controllers):
- `GestureClick` `pressed` → `fire()` immediately, then start a `glib::timeout_add_local_once` chain driven by `RepeatSchedule::next_delay()`.
- `released`/`cancel` → remove the source. `enabled` changes → cancel.
- Minimum 5 ms with GLib: GLib timeouts have 1 ms resolution but the frame clock is 16 ms. Proposal: keep the schedule values, expect the last ticks to bunch together (E-46-021).

`NumericChooser` (Int and f64 generic via a trait):

```rust
pub trait Num: Copy + PartialOrd + std::fmt::Display + std::str::FromStr { fn step_add(self, s: Self) -> Self; fn step_sub(self, s: Self) -> Self; }
pub struct NumericChooser<T: Num> { pub value: T, pub min: T, pub max: T, pub step: T, pub suffix: Option<String> }
```

Rules (behaviour 13.2): `value - step` and `value + step` on click; entry text parsed on change; invalid text ignored; `-` maps to zero; errors shown as CSS class `error` and labels "Value too big" / "Value too small"; value not clamped by the widget. Overflow: use `checked_sub`/`checked_add` for Int (`i64` fields) and saturate with a note (E-46-024).

Caution on Int and `"-"`: the Kotlin rewrites the text to `"0"` after the next value update. Kurobayomi: keep the `-` text while the entry has focus (no rewrite while focused). Deviation recorded in parity.

---

## 9. Threading model

```
Main thread (GTK, glib main context)
├── UI state (ControlsModel, widgets, timers)
├── glib::MainContext::channel<MpvEvent>  <- mpv thread
├── spawn_future_local for: sheets' data loads, file dialog results
└── gio/gtk callbacks

mpv thread (std::thread, owns mpv_handle)
├── drains std::sync::mpsc::Receiver<MpvCmd>
├── mpv_wait_event(timeout 50 ms) -> MpvEvent to glib channel
└── answers Query over oneshot

tokio runtime (multi-thread, created once in main; area 12-network-stack)
└── thumbnail tile fetch (AnimeHttpSource::get_image_tile), decode via image crate
    result delivered to main via glib channel (never touch widgets off-thread)

blocking pool (tokio::task::spawn_blocking)
└── screenshot save/share file IO, PipeWire volume command
```

Rules:
- Widgets and `ControlsModel` are `!Send`; they live on the main thread in `Rc<RefCell<_>>`.
- All cross-thread data is `Send` messages: `MpvCmd`, `MpvEvent`, `TileResult`.
- `glib::timeout_add_local` for UI timers (auto-hide 4000 ms, 2000 ms pills, 800/100 ms double tap, 150 ms triangles, repeat schedule). Their handles are stored in `TimerSlot` structs so that restarting a timer cancels the previous one (equivalent of `LaunchedEffect` cancel-on-key-change).
- Auto-hide restart: `ControlsModel.reset_epoch` change or armed-state change → cancel + re-arm (behaviour 1.3).

Timer ownership helper:

```rust
pub struct TimerSlot(std::cell::Cell<Option<glib::SourceId>>);

impl TimerSlot {
    /// Cancels any pending callback and arms a new one `ms` milliseconds from now.
    /// Equivalent of a LaunchedEffect restart on key change.
    pub fn restart(&self, ms: u32, f: impl FnOnce() + 'static) {
        if let Some(id) = self.0.take() {
            id.remove();
        }
        let mut f = Some(f);
        let id = glib::timeout_add_local(std::time::Duration::from_millis(ms as u64), move || {
            if let Some(f) = f.take() {
                f();
            }
            glib::ControlFlow::Break
        });
        self.0.set(Some(id));
    }
    pub fn cancel(&self) {
        if let Some(id) = self.0.take() {
            id.remove();
        }
    }
}
```

Note: `glib::timeout_add_local` returns a `SourceId` that is removed by `remove()`; a `Break` after the callback runs makes the source self-destroy, so calling `remove()` on an already-fired source must be guarded (the slot clears on restart; `remove()` on a finished source panics in some glib versions, so the implementation must clear the slot inside the callback). Implementation rule: the callback clears the slot before running `f`, so `cancel()` never removes a finished source.

Rust and dependencies (no new crate unless listed):
- `gtk4` 0.11 (present), `glib` (transitive, use `glib::` re-exports), `gdk4` (transitive).
- `serde` + `toml` for `PlayerSettings` (area 02 decides the file).
- `tokio` (area 12) for the thumbnail fetch; `image` crate for JPEG/PNG tile decode (area 14/34 may already choose it).
- `libmpv2` or `libmpv-sys` for the mpv bridge (area 45 decides). The UI depends only on `MpvCmd`/`MpvEvent`.
- Testing: built-in `#[test]`; no new dev-dependency.

Not used (on purpose): libadwaita (user rule), `relm4` (not needed for this surface), `iced` (not GTK).

---

## 10. Error handling

- Pure functions are total (no panics): `format_episode_number` handles NaN as `"NaN"`; `current_chapter` returns `None` for empty input; `next_speed` is total.
- Backend send errors (`mpsc::SendError`) mean the mpv thread died. Log with `tracing::error!` and show the pill "Player stopped" (new key, proposal) once per session, then disable controls. The Kotlin equivalent does not exist (no error channel); documented as a deliberate addition.
- Thumbnail fetch errors: log at `warn`, no UI change (Kotlin logs at ERROR, no UI, VM:1600-1605).
- File IO (screenshot): return `Result<PathBuf, std::io::Error>`; on error show the Kotlin-equivalent message from the screenshot sheet (part 2).
- Preference parse errors: fall back to the default per key (serde `default`), matching PreferenceStore.kt:35-40.
- No `unwrap()` on user input. `unwrap()` is allowed only on GTK construction of static widgets (as in the mockup).

---

## 11. Android-API to Linux mapping (cross-reference)

Full table in `p1-data-and-config.md` section 5. The design-relevant decisions:
- Device volume → PipeWire via `wpctl` (cheapest, no new crate), behind `trait SystemVolume { fn get(&self) -> i32; fn set(&self, v: i32); fn max(&self) -> i32 { 15 } }`.
- Screen brightness → `trait SystemBrightness { fn get(&self) -> Option<f32>; fn set(&self, v: f32) -> Result<(), BrightnessError>; }`, implementations: `BacklightSysfs` (writes `/sys/class/backlight/<dev>/brightness`, needs permission; if not writable fall back to `Logind` DBus; if neither, `None`).
- PiP → no implementation; `MiniPlayer` window is a T-46a-046 item.

```rust
pub trait SystemVolume {
    fn get_device_step(&self) -> Option<i32>;   // 0..=max_steps
    fn set_device_step(&self, step: i32) -> Result<(), String>;
    fn max_steps(&self) -> i32 { 15 }
}
pub trait SystemBrightness {
    fn get(&self) -> Option<f32>;                 // 0.0..=1.0
    fn set(&self, value: f32) -> Result<(), String>;
}
```

Initial brightness: `SystemBrightness::get().unwrap_or(0.0)` (Kotlin `getOrElse { 0f }`, VM:286-291).

---

## 12. Integration with the existing mockup

Current behaviour of `src/player.rs` `player()` that must change:

| Mockup | Problem | Change |
|---|---|---|
| Placeholder `surface` (`src/player.rs:~160`) | No video | Video surface from area 45 (GL area) goes at the bottom of the overlay |
| `Skip Opening` label, `"00:00 • Opening"` chapter label (src/player.rs ~ line 230) | Hard-coded | Bound to `ControlsModel` skip-intro text and current chapter |
| Auto-play switch always on screen, pill text literal `"Auto-play is on"` | Matches Aniyomi strings | Keep text; move to `keys::AUTO_PLAY_ON` |
| Quality button always visible (`src/player.rs`) | Aniyomi shows it only when `isEpisodeOnline == true` | Gate on `episode.is_online` |
| Speed label `format!("{:.2}x")` | Matches `%.2fx` | keep |
| Speed cycle `if v >= 2.0 { 0.25 } else { v + 0.25 }` (src/player.rs) | Same as Kotlin | Replace with `math::next_speed` (test) |
| Tap-to-toggle via `GestureClick` `connect_released` with `n == 2` branch | Double tap uses fixed 10 s and only left/centre/right zones without `Custom`/`Switch` | Use `double_tap::handle` with settings |
| Seek bar `gtk::Scale` with marks at 85 and 1330 (src/player.rs:~200) | Hard-coded marks | Replace with `SeekBar` widget + `segments` |
| Long press on surface opens `screenshot` sheet, does not pause | Aniyomi pauses | Pause + sheet |
| Lock hides controls except unlock (mockup) | Aniyomi hides all chrome except unlock; overlay visible when `controls shown && locked` | Use model rule |
| `pill()` 1200 ms | Aniyomi 2000 ms | `toast::flash` with 2000 ms |
| Placeholder `reader()` unaffected | | |

The mockup's `spec` system (`spec/reader_player.json`, `src/spec.rs`) defines sheet and dialog IDs `subtitle_tracks`, `audio_tracks`, `quality_tracks`, `chapters`, `playback_speed`, `more`, `screenshot`, `panel_subtitle_settings`, `panel_subtitle_delay`, `panel_audio_delay`, `panel_video_filters`, `episode_list`, `sleep_timer`, `set_art_confirm`, `integer_picker`. Keep these IDs as the `host` routing keys: map `Sheet::PlaybackSpeed` → `"playback_speed"`, `Panel::AudioDelay` → `"panel_audio_delay"`, `Dialog::EpisodeList` → `"episode_list"`, `Dialog::IntegerPicker` → `"integer_picker"`. The spec entries for `sleep_timer` and `set_art_confirm` are part 2 dialogs.

`src/ui.rs` helpers used: `show_sheet(&Sheet)` (ui.rs:391-405), `show_dialog(&Dialog)` (ui.rs:406-431), `modal` (ui.rs:377-390), `icon_button` (ui.rs:140-146), `cover` (ui.rs:177-183), `margins` (ui.rs:110-117), `tag` (ui.rs:501-504). The mockup's `show_sheet` renders from spec JSON; part 2 sheets keep that renderer. Part 1 uses it only for the host dispatch of sheet IDs, and replaces the Compose-specific behaviours (slide-up, scrim, drag) in `sheet.rs`.

`src/clamp.rs` (62 lines) is unrelated to the player.

---

## 13. Pitfalls

- P-1 GTK drag offsets: `GestureDrag` offsets are relative to the press point and only begin after GTK drag threshold (`gtk-dnd-drag-threshold`, default 8 px; `GestureDrag` uses its own threshold). Use the begin point as start, as in section 5, and note the deliberate difference.
- P-2 Double-click timing: GTK `gtk-double-click-time` default 400 ms vs Android `ViewConfiguration.doubleTapTimeout` 300 ms. Document; do not hard-code.
- P-3 Pointer vs touch: Compose tap/long-press gestures are touch-first; on a mouse, `GestureLongPress` may not fire for mouse buttons on every GTK backend. Proposal: implement long press as a 500 ms timer started in `pressed` and cancelled in `released` or on movement > 8 px (test on X11 and Wayland).
- P-4 Event consumption: GTK 4 propagation. Controls must `set_propagation_phase(Capture or Bubble)` deliberately: the gesture surface is below the controls in z-order, so the chrome widgets claim events and the surface only sees unclaimed clicks, as in Compose. Use `gtk::EventSequenceState::Claimed` in the double-tap handler (mockup does, `src/player.rs:~250`).
- P-5 Layout cost: `gtk::Overlay` with many children re-measures on every seek tick. Keep the seekbar as the only widget that redraws each frame; others update on state change.
- P-6 `set_visible` vs `set_opacity`: Compose `AnimatedVisibility` keeps layout space during exit. Use `set_opacity` + `set_can_target(false)` during exit to avoid jumps.
- P-7 Lua/custom key codes: `keypress 0x10001` strings must be sent verbatim.
- P-8 mpv thread blocking: never call mpv from the GTK thread directly; the Kotlin code does (PC:552, GH:165), Kurobayomi must not.
- P-9 Speed float drift: `0.25` steps are exact in binary; still format with `{:.2}` for display.
- P-10 `IntegerPicker` state: no `remember` equivalent is needed; keep the selection in the widget.
- P-11 `EpisodeListDialog` index `-1`: guard with `position().map(...)`.
- P-12 Fullscreen cursor: GTK does not hide the pointer over the overlay; optional idle-cursor hide is out of parity scope.

---

## 14. Test seams

Every pure function listed in sections 2 to 3 is covered by a `#[test]` (T-46a-056). Fixtures: a chapter list with unsorted entries, a 0-duration file, a duration not divisible by step (IntegerPicker), a negative brightness, and a volume with boost.
