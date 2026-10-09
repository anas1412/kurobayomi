# P1 Rust design: player settings, custom buttons, mpv script editor, code editor

Target: Kurobayomi (`/home/blackbox/kurobayomi`, crate `kurobayomi`, edition 2024, `gtk = gtk4 0.11.5` with `v4_16`, `serde`, `serde_json`). No libadwaita. Everything below is a **proposal** unless marked OBSERVED (which refers to the Kotlin app).

Current mockup relevant parts (OBSERVED in `/home/blackbox/kurobayomi`):
- `src/settings.rs`: builds settings screens from `spec/settings.json` via `settings::body(id)`. `settings_player_custom_buttons` is `cards(id, &["Skip opening", "Toggle shader"], true)` (placeholder data). `settings_player_editor` is `editor()` with hard-coded `autoskip.lua`, `aniskip.lua`, `aniskip.conf`. `settings_player_code_editor` is `code_editor()` with a fixed sample text, no line numbers, no highlighting, no unsaved-change guard.
- `src/spec.rs`: deserialises the JSON item model (switch, list, multi, slider, text, link, action, info ...).
- `src/player.rs`: player mock (OSD, overlays).
- `src/ui.rs`, `src/screens.rs`, `src/main.rs`, `src/entry.rs`, `src/clamp.rs`: shell, navigation, entries.
- No database, no preference store, no mpv binding exists yet.

---

## 1. Crate and module layout (new and changed files)

```
src/
  prefs/
    mod.rs            PrefStore trait, Pref<T>, PrefValue, NamedEnum, SqlitePrefStore, PrefCache
    ui.rs             UiPrefs (themes, nav, start screen, date format, relative time)
    player.rs         PlayerPrefs, GesturePrefs, DecoderPrefs, AudioPrefs, SubtitlePrefs, AdvancedPrefs, TorrentPrefs
  lang.rs             language list validator (ISO 639 table), used by audio and subtitle pages
  db/
    mod.rs            connection open, migrations (user_version), DbWorker
    custom_buttons.rs CustomButtonRepo (rusqlite impl), schema and seed row
  player/
    custom_buttons/
      mod.rs
      model.rs        CustomButton, CustomButtonUpdate, render placeholders
      plan.rs         pure reorder / delete / favourite planners (no I/O)
      service.rs      CustomButtonService: command enum, worker dispatch, error mapping
      lua.rs          custombuttons.lua generator
    lua_bridge.rs     LuaCall enum, parse_lua_call (14 properties), dispatch to player state
    vf.rs             build_vf_chain (debanding + yuv420p), hwdec/vo mapping
    mpv_dir.rs        runtime directory sync (generated files, user-file symlinks or copies)
    gestures.rs       SingleActionGesture dispatch table (keyboard/mouse/touch -> action)
  editor/
    files.rs          list / create / rename / delete / save for mpv-config/scripts and script-opts
    highlight.rs      Highlight engine + Lua and conf rule sets (fancy-regex)
    code_view.rs      code editor widget (GtkSourceView 5 or TextView fallback)
  torrent/
    settings.rs       port / tracker / proxy validation, default tracker list
    server.rs         child-process lifecycle for the torrserver binary
  settings/
    player/
      mod.rs          sidebar root (9 rows) and page stack
      rows.rs         typed row builders (switch, list, slider, text, edit-text, info)
      internal.rs     internal player page
      gestures.rs     gestures page
      decoder.rs      decoder page
      subtitle.rs     subtitle page
      audio.rs        audio page
      advanced.rs     advanced page (mpv.conf, input.conf, user files)
      torrent.rs      torrent page
      custom_buttons.rs  list page, card, create/edit dialog, delete dialog
      script_editor.rs   file list page, type dropdown, create/rename/delete dialogs
      dialogs.rs      shared modal dialog helpers
```

Remove the mock-only functions `settings::editor`, `settings::code_editor`, and `settings::cards` once the real pages exist. Keep `spec/settings.json` only for screens that stay spec-driven (see section 9).

---

## 2. Preferences

### 2.1 Trait and typed keys

```rust
// src/prefs/mod.rs
use std::marker::PhantomData;

#[derive(Debug, thiserror::Error)]
pub enum PrefError {
    #[error("preference storage failed: {0}")]
    Storage(#[from] rusqlite::Error),
}

/// Stored form of a value. Encodings are fixed by this trait, not by the caller.
pub trait PrefValue: Sized + Clone + Send + 'static {
    fn encode(&self) -> String;
    fn decode(raw: &str) -> Option<Self>;
}

impl PrefValue for bool {
    fn encode(&self) -> String { if *self { "true".into() } else { "false".into() } }
    fn decode(raw: &str) -> Option<Self> { match raw { "true" => Some(true), "false" => Some(false), _ => None } }
}
impl PrefValue for i64 {
    fn encode(&self) -> String { self.to_string() }
    fn decode(raw: &str) -> Option<Self> { raw.parse().ok() }
}
impl PrefValue for f32 {
    fn encode(&self) -> String { self.to_string() }   // round-trips f32 exactly
    fn decode(raw: &str) -> Option<Self> { raw.parse().ok() }
}
impl PrefValue for String {
    fn encode(&self) -> String { self.clone() }
    fn decode(raw: &str) -> Option<Self> { Some(raw.to_owned()) }
}

/// Enums are stored by variant name, like Kotlin's `getEnum` (`name`, fall back to default).
pub trait NamedEnum: Copy + Sized + Send + 'static {
    const VARIANTS: &'static [Self];
    fn name(self) -> &'static str;
}
impl<E: NamedEnum> PrefValue for E {
    fn encode(&self) -> String { self.name().to_owned() }
    fn decode(raw: &str) -> Option<Self> { Self::VARIANTS.iter().copied().find(|v| v.name() == raw) }
}

pub struct Pref<T: PrefValue> {
    pub key: &'static str,
    pub default: fn() -> T,        // fn pointer, so `const` tables are possible
    _t: PhantomData<T>,
}

impl<T: PrefValue> Pref<T> {
    pub const fn new(key: &'static str, default: fn() -> T) -> Self {
        Self { key, default, _t: PhantomData }
    }
    /// Missing or undecodable value → default. Never errors (matches Kotlin behaviour).
    pub fn get(&self, store: &dyn PrefStore) -> T {
        store.read(self.key).and_then(|raw| T::decode(&raw)).unwrap_or_else(|| (self.default)())
    }
    pub fn set(&self, store: &dyn PrefStore, value: &T) -> Result<(), PrefError> {
        store.write(self.key, &value.encode())
    }
    pub fn reset(&self, store: &dyn PrefStore) -> Result<(), PrefError> {
        store.remove(self.key)
    }
}

pub trait PrefStore {
    fn read(&self, key: &str) -> Option<String>;
    fn write(&self, key: &str, value: &str) -> Result<(), PrefError>;
    fn remove(&self, key: &str) -> Result<(), PrefError>;
}
```

Notes:
- `impl<E: NamedEnum> PrefValue for E` coexists with the concrete impls because `NamedEnum` is a local trait that `bool`, `i64`, `f32` and `String` do not implement. If the compiler objects, wrap enums in a newtype `Named<E>`.
- `f32` defaults that Kotlin stores as Float (`pref_progress_preference` 0.85F, `pref_sub_scale` 1f) must be read back with the same value. `0.85f32.to_string()` yields `"0.85"`, which parses back to the same `f32`.
- Keys are identical to the Kotlin keys (section 3) so a future export can carry them unchanged.

### 2.2 Store implementation and cache

```rust
// src/prefs/mod.rs
pub struct SqlitePrefStore {
    cache: std::cell::RefCell<std::collections::HashMap<String, String>>, // GTK thread only
    tx: async_channel::Sender<DbCommand>,                                 // to DB worker
}
```
- Load the whole `prefs(key TEXT PRIMARY KEY, value TEXT NOT NULL)` table into `cache` once at startup (small table).
- `read` consults the cache (synchronous, no I/O).
- `write` updates the cache immediately and sends `DbCommand::PrefSet` to the worker; the worker writes with `INSERT ... ON CONFLICT(key) DO UPDATE SET value = excluded.value`. Errors are logged and surfaced as a toast ("Unable to save setting"), the cache keeps the new value for the session (documented trade-off; the Kotlin SharedPreferences `apply()` behaves the same).
- `remove` deletes from cache and sends `DbCommand::PrefRemove`.
- `PrefStore` is not `Send`: it lives on the GTK thread. Background code gets values through owned snapshots (`PlayerPrefsSnapshot`), not through the store.

Change notification: pages re-read their rows when shown (no live subscription needed; the Kotlin `collectAsState` live behaviour is replaced by refresh-on-show plus an `RefCell` event `on_changed(key)` for the few rows that must update in place, for example the slider subtitle).

### 2.3 Typed groups

```rust
// src/prefs/player.rs
pub struct PlayerPrefs;   // constant table of Pref<T> values, no state
pub const PROGRESS: Pref<f32> = Pref::new("pref_progress_preference", || 0.85);
pub const PRESERVE_POSITION: Pref<bool> = Pref::new("pref_preserve_watching_position", || false);
pub const ORIENTATION: Pref<PlayerOrientation> = Pref::new("pref_default_player_orientation_type_key", || PlayerOrientation::SensorLandscape);
// ... one const per key in p1-data-and-config.md section 1
```
Constant tables keep the defaults in one place and avoid string typos. `fn() -> T` defaults are needed because `T` may not be `const`-constructible (`String`).

---

## 3. Enum definitions (exact stored names and values)

```rust
// src/prefs/player.rs and src/prefs/ui.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerOrientation { Free, Video, Portrait, ReversePortrait, SensorPortrait, Landscape, ReverseLandscape, SensorLandscape }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingleAction { None, Seek, PlayPause, Switch, Custom }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Debanding { None, Cpu, Gpu }      // stored names: "None", "CPU", "GPU"

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioChannels { Auto, AutoSafe, Mono, Stereo, ReverseStereo }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyMode { None, Tracker, Peers, Full }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode { Light, Dark, System }   // stored names: "LIGHT", "DARK", "SYSTEM"
```

Stored names are the Kotlin constant names (`"CPU"`, `"GPU"`, `"AutoSafe"`, `"SensorLandscape"`, ...). Implement `NamedEnum` with `name()` returning those exact strings, so existing exports stay compatible. (Rust variant names differ only by case where Kotlin uses upper case; the `name()` method is the source of truth.)

Audio channels mapping (fixed; see behaviour section 7):

```rust
impl AudioChannels {
    /// (mpv property, value) pairs applied at player start.
    pub fn mpv_setting(self) -> (&'static str, &'static str) {
        match self {
            Self::Auto => ("audio-channels", "auto"),        // label "Auto" now sends mpv "auto"
            Self::AutoSafe => ("audio-channels", "auto-safe"),
            Self::Mono => ("audio-channels", "mono"),
            Self::Stereo => ("audio-channels", "stereo"),
            Self::ReverseStereo => ("af", "pan=[stereo|c0=c1|c1=c0]"),
        }
    }
}
```
**Deliberate deviation:** Kotlin maps "Auto" to `auto-safe` and "Auto-safe" to `auto`. The default stored value `AutoSafe` therefore changes meaning (it becomes `auto-safe`). Record in the release notes; users on the default keep working because the default is still `AutoSafe` (now mpv `auto-safe`, which is mpv's own default).

Proxy mode: no `value()` method. The server argument is built only when a proxy URL is set (see `p1-topic-torrent-settings.md` section 6).

---

## 4. Custom buttons

### 4.1 Model and rendering

```rust
// src/player/custom_buttons/model.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomButton {
    pub id: i64,
    pub name: String,
    pub is_favorite: bool,
    pub sort_index: i64,
    pub content: String,
    pub long_press_content: String,
    pub on_startup: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomButtonUpdate {
    pub id: i64,
    pub name: Option<String>,
    pub is_favorite: Option<bool>,
    pub sort_index: Option<i64>,
    pub content: Option<String>,
    pub long_press_content: Option<String>,
    pub on_startup: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewButton {
    pub name: String,
    pub content: String,
    pub long_press_content: String,
    pub on_startup: String,
}

impl CustomButton {
    /// Textual substitution, in this order, exactly as Kotlin does.
    pub fn render(&self, template: &str, primary_id: i64) -> String {
        let is_primary = if primary_id == self.id { "true" } else { "false" };
        template.replace("$id", &self.id.to_string()).replace("$isPrimary", is_primary)
    }
}
```
`primary_id` is `0` when no favourite exists (as in Kotlin), so `$isPrimary` is `false` for every button.

### 4.2 Pure planners (testable without a database)

```rust
// src/player/custom_buttons/plan.rs
/// `ids` is the current list in sortIndex order. Returns (id, new_sort_index) for every row.
pub fn plan_reorder(ids: &[i64], id: i64, new_index: usize) -> Result<Vec<(i64, i64)>, PlanError> {
    let from = ids.iter().position(|&x| x == id).ok_or(PlanError::Unchanged)?;
    let mut v = ids.to_vec();
    let moved = v.remove(from);
    if new_index > v.len() { return Err(PlanError::IndexOutOfRange); }   // Kotlin: IndexOutOfBounds -> InternalError
    v.insert(new_index, moved);
    Ok(v.iter().enumerate().map(|(i, &x)| (x, i as i64)).collect())
}

/// Deleting: remaining ids renumbered 0..n-1 in current order.
pub fn plan_after_delete(ids: &[i64], id: i64) -> Vec<(i64, i64)> {
    ids.iter().filter(|&&x| x != id).enumerate().map(|(i, &x)| (x, i as i64)).collect()
}

/// Favourite toggle. Turning off affects only `id`. Turning on sets exactly one favourite.
pub fn plan_favorite(all: &[(i64, bool)], id: i64, currently_favorite: bool) -> Vec<(i64, bool)> {
    if currently_favorite {
        vec![(id, false)]
    } else {
        all.iter().map(|&(x, _)| (x, x == id)).collect()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PlanError { Unchanged, IndexOutOfRange }
```
Important: `plan_favorite` takes `currently_favorite` from the **fresh** row read inside the worker, not from the UI snapshot (the Kotlin code uses the snapshot; the fix is part of T-48a-042 acceptance).

Create: `sort_index = max(sort_index) + 1` or `0` when empty (computed in the worker from a fresh read; the serialised worker removes the race from edge E-CB-04).

### 4.3 Repository and worker

```rust
// src/player/custom_buttons/service.rs
pub trait CustomButtonRepo {
    fn list(&self) -> rusqlite::Result<Vec<CustomButton>>;             // ORDER BY sort_index, _id
    fn insert(&mut self, new: &NewButton, sort_index: i64) -> rusqlite::Result<i64>; // favourite = false
    fn update_partial(&mut self, u: &CustomButtonUpdate) -> rusqlite::Result<()>;     // coalesce semantics
    fn update_partial_many(&mut self, us: &[CustomButtonUpdate]) -> rusqlite::Result<()>; // one transaction
    fn delete(&mut self, id: i64) -> rusqlite::Result<()>;
}

pub enum ButtonCommand {
    Create(NewButton, Reply<Result<(), ButtonError>>),
    Update(CustomButtonUpdate, Reply<Result<(), ButtonError>>),      // never carries sort_index from the UI
    Delete(i64, Reply<Result<(), ButtonError>>),
    Reorder { id: i64, new_index: usize, reply: Reply<Result<ReorderOutcome, ButtonError>> },
    ToggleFavorite { id: i64, reply: Reply<Result<(), ButtonError>> },   // reads current state itself
}

pub type Reply<T> = async_channel::Sender<T>;

#[derive(Debug, thiserror::Error)]
pub enum ButtonError {
    #[error("Error Saving Custom Button to Database")]   // SaveCustomButtonException message kept for logs
    Save(#[source] rusqlite::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

pub enum ReorderOutcome { Done, Unchanged }
```

Worker loop (dedicated `std::thread`, owns the `rusqlite::Connection`):
- Receives `ButtonCommand` from an `std::sync::mpsc::Receiver`. Processes **one command at a time** (this replaces the Kotlin `Mutex` in `ReorderCustomButton` and `ToggleFavoriteCustomButton`; no lock is needed).
- Each command runs in its own transaction where multiple rows change (`update_partial_many`, delete+renumber).
- Replies on the `async_channel` sender. The GTK side awaits with `glib::spawn_future_local`.
- Delete: one transaction for `delete` plus renumber (Kotlin runs them as two statements; the transaction removes the gap window).
- Logging: `tracing::error!` on every `Err`, including `Update` (Kotlin skips logging for update; fix).

Error mapping: `rusqlite` errors on insert become `ButtonError::Save`; other failures are `Internal`. The GTK side shows the toast `internal_error` ("InternalError: Check crash logs for further information") for both, to keep the Kotlin message. Proposal: use a plain, user-readable message: "Could not save the custom button. Details are in the log."

Reorder command from the UI: call once on drop (not per drag step; Kotlin writes per step).

### 4.4 Lua generation

```rust
// src/player/custom_buttons/lua.rs
pub const HEADER: &str = "local lua_modules = mp.find_config_file('scripts')\n\
if lua_modules then\n\
    package.path = package.path .. ';' .. lua_modules .. '/?.lua;' .. lua_modules .. '/?/init.lua;' .. '{SCRIPTS_DIR}' .. '/?.lua'\n\
end\n\
local aniyomi = require 'aniyomi'\n";

pub fn custombuttons_lua(buttons: &[CustomButton], primary_id: i64, scripts_dir: &std::path::Path) -> String {
    let mut out = HEADER.replace("{SCRIPTS_DIR}", &scripts_dir.to_string_lossy());
    for b in buttons {
        let id = b.id;
        // per-button isolation (proposal): each body runs under pcall, errors are reported with the button name
        out += &format!(
            "{startup}\nfunction button{id}()\n{tap}\nend\nmp.register_script_message('call_button_{id}', button{id})\n\
function button{id}long()\n{long}\nend\nmp.register_script_message('call_button_{id}_long', button{id}long)\n",
            startup = b.render(&b.on_startup, primary_id),
            tap = b.render(&b.content, primary_id),
            long = b.render(&b.long_press_content, primary_id),
        );
    }
    out
}
```
Kotlin behaviour is **without** `pcall`: one syntax error disables all buttons (edge E-CB-01). The `pcall` form above is the proposal. The file must be written to `<runtime>/scripts/custombuttons.lua` and loaded with `load-script` through libmpv.

Keep the header's `package.path` concatenation identical except for path separator handling (Linux paths need no change).

### 4.5 Runtime state and events

```rust
// src/player/mod.rs (GTK thread)
pub struct PrimaryButtonState {
    pub button: Option<CustomButton>,   // favourite, or None
    pub title: String,                  // set by Lua; "" → use button.name
}
```
Rules (from Kotlin, section 6 of the topic file): on load, `button` = favourite and `title` = favourite name if empty; `set_button_title` overwrites `title`; `reset_button_title` sets `title` to the favourite's name; `toggle_button show|hide|toggle` as in Kotlin.

---

## 5. Lua bridge

```rust
// src/player/lua_bridge.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LuaCall {
    ShowText(String),
    ToggleUi(Visibility),                         // show | hide | toggle (hide also clears sheets, panels, dialogs)
    ShowPanel(PanelKind),                         // SubtitleSettings | SubtitleDelay | AudioDelay | VideoFilters
    SoftwareKeyboard(Visibility),                 // Android-only on mobile; ignored on desktop with a debug log
    SetButtonTitle(String),
    ResetButtonTitle,
    ToggleButton(Visibility),
    SwitchEpisode(Step),                          // "n" → Next, "p" → Previous
    Pause(PauseAction),                           // pause | unpause | pauseunpause
    SeekBy(i64),
    SeekTo(i64),
    SeekByWithText { by: i64, text: String },
    SeekToWithText { to: i64, text: String },
    IntPicker { title: String, name_format: String, min: i64, max: i64, step: i64, property: String },
}

#[derive(Debug, PartialEq, Eq)]
pub enum LuaParseError { UnknownProperty(String), BadInteger(String), BadArity { expected: usize, got: usize } }

/// `property` is the full mpv name, e.g. "user-data/aniyomi/seek_by". `raw` is the observed string value.
pub fn parse_lua_call(property: &str, raw: &str) -> Result<Option<LuaCall>, LuaParseError> { /* … */ }
```

Parsing rules (must match Kotlin exactly, with errors instead of exceptions):
1. `data = raw.strip_prefix('"').and_then(strip_suffix('"'))` (remove exactly one leading and one trailing quote character, as `removePrefix`/`removeSuffix`). If `data` is empty → `Ok(None)` (ignored).
2. Name = last path segment of `property`.
3. `seek_by_with_text` / `seek_to_with_text`: `splitn(2, '|')`; the integer is parsed; text is the remainder (may contain `|`).
4. `launch_int_picker`: `split('|')` must give exactly 6 parts; otherwise `BadArity`. Kotlin silently throws; here it is logged and dropped.
5. Integers: `i64::from_str`; failure → `BadInteger` (Kotlin throws `NumberFormatException` on the observer thread; proposal logs and drops).
6. After handling, always clear the property (`mpv.set_property(property, "")`), even when the call was ignored. The clear is what makes repeated identical calls fire again.

Threading: libmpv property-change events arrive on the mpv thread. Forward `(property, raw)` through `async_channel` to the GTK main context; parse and apply there. Never touch GTK widgets on the mpv thread.

mpv observation: 14 names, all `MPV_FORMAT_STRING` in Kotlin. In `libmpv2` use the string format observer for each name (verify the crate's `observe_property` for strings; fallback raw FFI `mpv_observe_property` with `MPV_FORMAT_STRING`).

Ordering: Kotlin dispatches on the observer thread. Keep the same order in one GTK-thread queue so `seek_by` and `pause` requested in one Lua block keep their order.

---

## 6. Language codes

```rust
// src/lang.rs
pub struct InvalidCode(pub String);

/// Same split/filter/trim order as the Kotlin validator. Returns the first invalid element.
pub fn validate_list(raw: &str) -> Result<(), InvalidCode> {
    for code in raw.split(',').filter(|s| !s.is_empty()).map(str::trim) {
        if !is_known_code(code) { return Err(InvalidCode(code.to_owned())); }
    }
    Ok(())
}

fn is_known_code(code: &str) -> bool {
    let lower = code.to_ascii_lowercase();
    // Accept ISO 639-1 (2 letters) and ISO 639-2/T or 639-3 (3 letters) from a static table.
    lower.len() >= 2 && (ISO_639_1.binary_search(&lower.as_str()).is_ok() || ISO_639_3.binary_search(&lower.as_str()).is_ok())
}
```
Decision needed (see the behaviour section 6, "three-letter codes are unverified"): this design accepts all ISO 639-1 and ISO 639-3 codes (the table size is about 8,000 entries; the `isolang` crate has them). Candidate crate: `isolang` (verify its API for `from_639_1` / `from_639_3`). Empty trimmed codes are invalid (`""`), matching Kotlin.

Subtitle and audio pages call the same function; the error message is `Invalid code: "<code>"`.

---

## 7. Decoder, debanding and the vf chain

```rust
// src/player/vf.rs
pub fn build_vf_chain(debanding: Debanding, yuv420p: bool) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if debanding == Debanding::Cpu { parts.push("gradfun=radius=12"); }
    if yuv420p { parts.push("format=yuv420p"); }
    if parts.is_empty() { None } else { Some(parts.join(",")) }
}
// Debanding::Gpu is not a vf: it sets the option `deband=yes`.
```
Kotlin overwrites `vf` (observed bug). This version keeps both filters. `format=yuv420p` after `gradfun` is deliberate (gradfun runs on the source format).

Hardware decoding (`hwdec`): Kotlin uses `auto` or `no`. Linux proposal: `auto` (mpv picks VA-API, NVDEC, or software), `no`. Do not expose the Android `mediacodec*` values. The `Decoder` enum from `PlayerEnums.kt` is dropped on desktop.

Video output: `vo=gpu` or `vo=gpu-next` (preference `pref_gpu_next`). Requires the GL/EGL context GTK provides. Verify that mpv embedding in a GTK GLArea works with `gpu-next` on the user's driver; keep `gpu` as default.

---

## 8. Gestures

```rust
// src/player/gestures.rs
pub enum GestureSlot { DoubleTapLeft, DoubleTapCenter, DoubleTapRight, MediaPrevious, MediaPlayPause, MediaNext }

pub fn allowed_actions(slot: GestureSlot) -> &'static [SingleAction] {
    use SingleAction::*;
    match slot {
        GestureSlot::DoubleTapLeft | GestureSlot::DoubleTapRight | GestureSlot::MediaPrevious | GestureSlot::MediaNext
            => &[None, Seek, PlayPause, Switch, Custom],
        GestureSlot::DoubleTapCenter => &[None, PlayPause, Custom],
        GestureSlot::MediaPlayPause => &[None, PlayPause, Custom],
    }
}
pub fn default_action(slot: GestureSlot) -> SingleAction { /* left/right Seek, center PlayPause, media prev/next Switch, play/pause PlayPause */ }
```
Input mapping on desktop:
- Double click left / centre / right third of the video area → the matching slot.
- Mouse back/forward buttons and keyboard media keys → media slots (via `gtk::EventControllerKey` and MPRIS, section 11).
- `Custom` → send mpv key name `kurobayomi-<slot>` (`kurobayomi-double-tap-left`, `-center`, `-right`, `kurobayomi-media-previous`, `kurobayomi-media-play-pause`, `kurobayomi-media-next`). Bind in `input.conf`.

Brightness and volume gestures: vertical drag on the left half changes brightness, on the right half changes volume (Kotlin default, swapped by `pref_swap_volume_and_brightness`). Desktop replacement: brightness is applied as mpv `brightness` (video filter, -100..100) because Wayland has no per-app backlight control. Document the loss (screen backlight is not changed).

---

## 9. Settings UI

### 9.1 Row model

```rust
// src/settings/player/rows.rs
pub enum Enabled { Always, When(fn(&dyn PrefStore) -> bool) }

pub enum Row {
    Group { title: &'static str, rows: Vec<Row> },
    Switch { key: &'static str, title: String, subtitle: Option<String>, enabled: Enabled, default: bool },
    List   { key: &'static str, title: String, options: Vec<(String, String)>, default: String, enabled: Enabled },
    Slider { key: &'static str, title: String, range: std::ops::RangeInclusive<i64>, default: i64, subtitle: SliderLabel, enabled: Enabled },
    Text   { title: String, subtitle: Option<String>, on_click: fn(&gtk::Window), enabled: Enabled },
    EditText { key: &'static str, title: String, dialog_subtitle: Option<String>, validate: fn(&str) -> Result<(), String>, multiline: bool, can_be_blank: bool, enabled: Enabled },
    Info   { text: String, enabled: Enabled },
}
pub enum SliderLabel { Plain, Percent, Milliseconds(&'static str) }
```

Build page content from `Vec<Row>` with a single function `build_page(rows, prefs) -> gtk::Widget`. This replaces both `spec.rs` items and the per-screen Kotlin `Preference` lists.

Rendering rules (adapted from Kotlin semantics, `p1-behavior.md` section 2):
- `Enabled::When(f)` false → row shown **greyed and insensitive** (proposal; Kotlin hides it). Each row carries a tooltip with the reason only where the spec says so (not defined: no tooltips in v1).
- `List` dialog: `gtk::Window` modal with a `gtk::ListBox` of radio rows; choosing a row applies immediately and closes; Cancel closes without change.
- `EditText` dialog: `gtk::Window` with `gtk::Entry` (single line) or `gtk::TextView` (multi-line), validation on every change, OK sensitivity per section 2 of behaviour (`text != current && (non_blank || can_be_blank) && validate ok`).
- `Slider`: `gtk::Scale` with `set_draw_value(true)` and the label in the row subtitle.

### 9.2 Pages to build (module mapping)

| Page | File | Notes |
|---|---|---|
| Root (9 rows) | `settings/player/mod.rs` | sidebar `gtk::ListBox` + `gtk::Stack`; narrow width (< 720 px) shows a list page with back arrow |
| Internal | `internal.rs` | groups: top-level, Controls, Hosters, Display, Intro skip, PiP (only if `pip_supported()`), External player |
| Gestures | `gestures.rs` | Sliders, Seeking, Double tap, Media controls; intro-length dialog uses `gtk::SpinButton` 0..=255 (initial value set explicitly) |
| Decoder | `decoder.rs` | 4 rows |
| Subtitles | `subtitle.rs` | 3 edit rows with validation (lang only) |
| Audio | `audio.rs` | 4 rows |
| Advanced | `advanced.rs` | switch + two multi-line editors (mpv.conf, input.conf) |
| Torrent | `torrent.rs` | see topic file |
| Custom buttons | `custom_buttons.rs` | real list model (section 4) |
| Script editor | `script_editor.rs` | real directory listing (section 10) |

### 9.3 Mockup integration

- Keep `spec/settings.json` for the shell and non-player screens (not part of this area).
- For the player pages, remove the spec entries `settings_player_custom_buttons`, `settings_player_editor`, `settings_player_code_editor` (they are placeholders), or keep them as `"kind": "custom"` markers that route to the Rust builders. Decision recorded in `p1-tasks.md` T-48a-010.
- `settings::cards` with fixed names is removed; the list is backed by `CustomButtonRepo` results.

---

## 10. Script editor and code editor

### 10.1 File service

```rust
// src/editor/files.rs
pub enum ScriptKind { Lua, ConfOpts }                      // dir "scripts" / "script-opts"
pub struct ScriptEntry { pub name: String, pub size: u64, pub modified: Option<std::time::SystemTime> }

pub struct ScriptDir { root: std::path::PathBuf, kind: ScriptKind }
impl ScriptDir {
    pub fn list(&self) -> std::io::Result<Vec<ScriptEntry>>;                     // files only, sorted by name (case-insensitive)
    pub fn create(&self, name: &str) -> std::io::Result<()>;                     // create_new; error if exists
    pub fn rename(&self, from: &str, to: &str) -> std::io::Result<()>;           // rename(2); no overwrite check
    pub fn delete(&self, name: &str) -> std::io::Result<()>;
    pub fn read_text(&self, name: &str) -> std::io::Result<String>;             // from_utf8_lossy, flags lossy
    pub fn save_text(&self, name: &str, text: &str) -> std::io::Result<()>;      // temp file + rename
}

/// Same rules as DiskUtil.buildValidFilename + the two checks in PlayerSettingsEditorScreenModel.isValidName
pub fn validate_name(name: &str, others: &[String], initial: Option<&str>) -> Result<(), NameError>;
pub enum NameError { AlreadyExists, Invalid }
pub fn build_valid_filename(name: &str) -> String;     // trim '.' and ' '; empty → "(invalid)"; illegal → '_'; take 240 chars
```
- Root = `<data>/mpv-config/<dir>`, created on demand.
- `create` uses `OpenOptions::new().write(true).create_new(true)`. Kotlin's `createFile` returns the existing file; the port reports an error instead (edge E-ED-04).
- `rename` must not overwrite: check existence first (race window accepted, documented).
- `save_text` writes `name.tmp-<random>` in the same directory, `fsync`, then `rename` over the target, then `fsync` the directory (atomic on ext4/btrfs/xfs).
- Filename validation: forbid `/`, NUL, and control characters; the Kotlin set also forbids `"`, `*`, `:`, `<`, `>`, `?`, `\`, `|`, and DEL, keep them for cross-platform sharing with the Android app's folder (proposal: keep the full Kotlin set).

### 10.2 Code view

```rust
// src/editor/code_view.rs
pub struct CodeView { pub widget: gtk::ScrolledWindow, pub buffer: sourceview5::Buffer, /* … */ }
pub fn open_editor(path: PathBuf, parent: &gtk::Window) -> CodeView;
```
- GtkSourceView 5 (`sourceview5` crate, verify it targets the same `gtk4` as `gtk = 0.11`). `buffer.set_language(lua or ini)`. Line numbers on (`show_line_numbers(true)`), monospace font, no wrap (`WrapMode::None`), auto indent off (match Kotlin plain editing).
- Modified tracking: `buffer.connect_changed` → set `modified = true`; `save` resets it.
- Save action enabled iff modified; Ctrl+S accelerator; toast on success "File saved" and on error "Unable to save file: <error>".
- Close with modified buffer: modal dialog "You have unsaved changes. Do you still wish to go back?" with Cancel / OK (OK discards).
- Highlight: the Kotlin regex colouring (section 4 of the highlight topic) is applied with `gtk::TextTag`s on `buffer.connect_changed` when `len <= 15000`. When GtkSourceView's own Lua/ini highlighting is on, disable the custom tags (only one highlighter is active).
- Fallback if `sourceview5` cannot be used: `gtk::TextView` plus the custom tags only, and draw line numbers in a second `gtk::TextView` as in Kotlin.

---

## 11. Linux equivalents for Android facilities (mapping)

| Android / Kotlin | Linux / Rust replacement | Notes |
|---|---|---|
| `PreferenceStore` (SharedPreferences) | `SqlitePrefStore` over SQLite `prefs` table (rusqlite) + in-memory cache | same keys |
| Voyager `Navigator` / `Screen` | `gtk::Stack` + `Vec<PageId>` back stack in `ui.rs` | |
| Compose `AlertDialog` | `gtk::Window` modal (transient for main window) | `gtk::AlertDialog` (GTK ≥ 4.10) for plain confirms |
| `Context.toast` | overlay label in `gtk::Overlay`, 2 s (Toast.LENGTH_SHORT) | no libadwaita toast |
| `UniFile` / SAF | `std::fs`, `gio::File` for URIs | base dir is a path, not a content URI |
| `Environment.isExternalStorageManager` | not needed | |
| `PackageManager.getInstalledPackages` | `gio::AppInfo::all_for_type("video/*")` | external players list is MIME-driven |
| `Intent(ACTION_VIEW)` | `gio::AppInfo::launch_uris` on the chosen app | |
| `UriHandler.openUri` | `gtk::UriLauncher` (GTK ≥ 4.10) | |
| `DisplayMetrics` (stretch aspect) | `gdk::Monitor::geometry()` | |
| `AudioManager` / audio focus | none in app; rely on PipeWire/PulseAudio policy | document |
| `Build.VERSION.SDK_INT` gates | runtime capability checks (`pip_supported()` = compositor support probe; fullscreen always) | |
| `WheelTextPicker` | `gtk::SpinButton` or `gtk::DropDown` for 0..=255 | |
| Haptics on wheel | none | |
| `kotlinx.serialization` protobuf | area:21 chooses `prost` or equivalent; field numbers from `p1-data-and-config.md` section 8 | |
| `java.net.URI`/`android.net.Uri` | `url::Url::parse` | scheme/host checks |
| `Locale`, `MissingResourceException` | `isolang` table, `Result` | |
| `SimpleDateFormat("MMMM d, yyyy HH:mm")` | `chrono` with `%B %-d, %Y %H:%M` | |
| `DecimalFormat` / `NumberFormat.getPercentInstance` | `format!("{}%", v)`; locale digits not required | |
| `kohesive Injekt` | constructor injection (`Rc<Services>`) | |
| coroutines / `StateFlow` | `async_channel` + `glib::spawn_future_local`; `RefCell` state with change callbacks | |
| `Logcat` | `tracing` + `tracing-subscriber` | |
| libmpv JNI `MPVLib` | `libmpv2` crate (verify observe/set/command APIs) | |
| TorrServer JNI | child process | section 3 of topic |
| Foreground service notification | `gio::Notification` (only for torrent server, optional) | |

---

## 12. Threading model

| Work | Thread | Mechanism |
|---|---|---|
| GTK widgets, preference cache, page state | GTK main thread | `glib` main loop |
| SQLite (`custom_buttons`, `prefs` writes) | DB worker `std::thread` | `std::sync::mpsc` in, `async_channel` out |
| libmpv core and events | mpv thread (libmpv internal) | observers push `(property, raw)` to an `async_channel` |
| Lua bridge parsing and dispatch | GTK main thread | consumes the channel with `glib::spawn_future_local` |
| File I/O for editors | GTK thread for small files (< 1 MiB); `gio::spawn_blocking`-equivalent (a worker thread) for larger | |
| Torrent child process supervision | one `std::thread` reading stdout and waiting | results to GTK by channel |
| Network (other areas) | tokio runtime on its own thread | owned by area owners |

Rules:
- Never call a GTK API from a non-main thread.
- Never block the GTK thread on the DB worker; always `await` the reply.
- Commands from the UI are processed in order by the DB worker; no separate mutexes.
- Bridge handlers run in the order received.

---

## 13. Error handling

- Library errors: `thiserror` enums per module (`PrefError`, `ButtonError`, `LuaParseError`, `NameError`, `InvalidCode`).
- Boundary errors (UI): log with `tracing::error!`, show one toast. Never panic on user-supplied Lua, names or numbers.
- `rusqlite` failures on read at startup: show a blocking error dialog with the path to the database file; the app does not start with an unknown schema (`PRAGMA user_version` checked).
- The `custom_buttons` seed insert uses `INSERT OR IGNORE`, so repeat migrations are idempotent.
- Save failures keep the editor open with the modified flag set (no data loss).

---

## 14. Security notes

- Custom button Lua and `onStartup` code run inside the mpv process with the user's privileges. This is the feature. Restoring a backup can add buttons that run on the next player start. Proposal: after a restore that adds buttons, show a one-time notice listing their names and stating they run Lua.
- Script and config files are plain text in the user's data directory. Do not execute files that are not user-enabled (`mpv_scripts` switch off means user scripts are not loaded).
- Torrent proxy URLs may contain credentials; do not log them. Mask in error text (`scheme://***@host`).

---

## 15. Tests

See `p1-edge-cases-and-tests.md`. The pure planners (`plan.rs`), `parse_lua_call`, `validate_list`, `validate_name`, `build_valid_filename`, `build_vf_chain`, `custombuttons_lua` and the highlight engine are unit-testable without GTK or libmpv.

---

## 16. Pitfalls

1. `regex` (Rust) has no look-around and no back-references. Use `fancy-regex` for the highlight rule sets (`Lua` rule 10 needs `\1`).
2. Java `\w` and `\d` are ASCII by default; Rust Unicode classes differ. Replace them explicitly.
3. SQLite `ORDER BY sortIndex` with ties is undefined. Always `ORDER BY sort_index, _id`.
4. `rusqlite::Connection` is not `Sync`. Keep it on the worker thread only.
5. mpv observers run on the mpv thread. The Lua handler must not run there.
6. The Kotlin `vf` overwrite bug must not be copied (section 7).
7. `$id` replacement is textual; a user writing `$idx` gets `1x`. Keep the rule and document it, or move to explicit placeholders in a future version (no change now).
8. GTK 4.16 `gtk::FileChooser` is deprecated in favour of `gtk::FileDialog` (GTK ≥ 4.10); use `FileDialog` for the base-directory picker.
9. Without libadwaita there is no `AdwPreferencesPage`. Use `gtk::ListBox` with the `boxed-list` style class; the class exists in GTK's default theme but the user's GTK theme may style it differently (the user runs Tokyo Night in other apps; verify it renders).
10. Wayland has no client-side `always on top` for normal toplevels. The PiP replacement must degrade to "keep window on top where the compositor supports it" (`xdg-toplevel` has no such request; on KDE use the window rules). Document.
11. `gtk::TextView` line numbers: count logical lines (`buffer.line_count()`), not wrapped display lines.
12. Mouse double-click detection must not fire the seek action on the first click of a double-click that becomes a drag (use `GestureClick` with `n_press`).
