# Area 45 (player core, part 1): Rust design

Target: crate `kurobayomi` (edition 2024, `gtk4` 0.11 with `v4_16`, see `/home/blackbox/kurobayomi/Cargo.toml`). No libadwaita. Behaviour source: `p1-behavior.md`. Config and endpoints: `p1-data-and-config.md`. Task IDs: `p1-tasks.md`.

Versions of new crates below are not pinned in this document. Before adding each one, check the current release on crates.io and the MSRV against the stable toolchain.

---

## 1. Boundary with part 2 (contracts this part depends on)

Part 1 calls these part-2 items (`PlayerViewModel` and loaders, documented in the part-2 files). Part 1 defines them as a Rust trait so the two halves can be built and tested separately.

```rust
// src/player/controller.rs (part 1 owns the trait, part 2 implements it)
pub trait PlayerModel {
    fn init(&mut self, req: &PlayerRequest) -> Result<InitResult, PlayerError>;
    fn load_episode(&mut self, episode_id: Option<i64>) -> Result<Option<SwitchMethod>, PlayerError>;
    fn load_hosters(&mut self, source: SourceRef, hosters: Vec<Hoster>, hoster_index: i32, video_index: i32);
    fn change_episode(&mut self, previous: bool, auto_play: bool);
    fn pause(&mut self);
    fn unpause(&mut self);
    fn pause_unpause(&mut self);
    fn seek_by(&mut self, seconds: i64);
    fn left_seek(&mut self);
    fn right_seek(&mut self);
    fn change_volume_by(&mut self, delta: i32);
    fn change_mpv_volume_to(&mut self, value: i32);
    fn change_video_aspect(&mut self, aspect: VideoAspect);
    fn handle_lua_invocation(&mut self, property: &str, value: &str);
    fn update_play_back_pos(&mut self, pos_seconds: f32);
    fn set_chapter(&mut self, pos_seconds: f32);
    fn update_read_ahead(&mut self, seconds: i64);
    fn set_mpv_volume(&mut self, volume: i32);
    fn set_anime_skip_intro_length(&mut self, seconds: i64);
    fn load_chapters(&mut self);
    fn load_tracks(&mut self);
    fn update_chapters(&mut self, chapters: Vec<Chapter>);
    fn save_current_episode_watching_progress(&mut self);
    fn delete_pending_episodes(&mut self);
    async fn ani_skip_response(&self, duration: f64) -> Option<Vec<TimeStamp>>;   // async fn in trait (Rust 1.75+)
    fn on_save_instance_state_non_configuration_change(&mut self);
    fn volume_boost_cap(&self) -> i32;
    fn set_volume_boost_cap(&mut self, cap: i32);
    fn max_volume(&self) -> i32;
    fn is_loading_episode(&self) -> bool;
    fn is_episode_online(&self) -> Option<bool>;
    fn current_video(&self) -> Option<Video>;
    fn update_video(&mut self, video: Video);
}
```

Notes:

- `SwitchMethod` carries `source`, `hoster_list: Option<Vec<Hoster>>`, `episode_title: String` (see behaviour 7.1).
- `Hoster` and `SerializableHoster` are source-api types (area 16/part 2). Part 1 only carries them.
- Duration and position are `f64` seconds in the trait for mpv parity; the internal episode positions are milliseconds (`i64`) as the data doc states (section 2.6).

---

## 2. Crate and module layout

```
src/
  player/
    mod.rs          pub use; PlayerRequest; PlayerError
    enums.rs        all enums of PlayerEnums.kt (section 2.9 of data doc)
    state.rs        PlayerState, Sheets/Panels/Dialogs/PlayerUpdates usage
    options.rs      PURE: PlayerPrefs -> Vec<MpvSetting> (startup options, section 2.2 behaviour)
    mpv.rs          MpvHandle: safe wrapper over libmpv2 (create, options, props, commands, render)
    observer.rs     MpvEvent enum, property table (34 entries), dispatch to controller
    keys.rs         PURE: gdk keyval + modifiers -> mpv key string (section 8)
    bridge.rs       PURE: user-data/aniyomi protocol parsing and dispatch
    source_url.rs   PURE: resolve file / data / protocol strings; basename helper
    controller.rs   PlayerController: state machine (behaviour 1.12), episode switch, file_loaded
    progress.rs     save progress/history/delete-after/tracker update (async, IO)
    external.rs     ExternalPlayer: presets, argv builder (PURE), spawn, IPC progress poll
    window.rs       GTK ApplicationWindow + GLArea + overlay controls host (part 2 controls)
    mini.rs         mini-player window (P2)
    mpris.rs        MPRIS server (P1)
    screenshot.rs   save/share/set-art result handling (messages only in part 1)
  services/
    mod.rs
    local_server.rs LocalServer (127.0.0.1, ephemeral port)
    torrent.rs      TorrentServer lifecycle + TorrentApi client + play-link builder (PURE)
```

Pure functions (no GTK, no libmpv, no I/O) are `options.rs`, `keys.rs`, `bridge.rs` (parsing), `source_url.rs`, `external.rs` (argv), `torrent.rs` (link and magnet index). These carry the unit tests in `p1-edge-cases-and-tests.md`.

### 2.1 Module ownership of Kotlin files

| Kotlin file | Rust module(s) |
|---|---|
| `AniyomiMPVView.kt` | `mpv.rs` (handle), `options.rs` (initOptions, setupAudio/SubtitlesOptions), `keys.rs` (onKey), `observer.rs` (observedProps), `enums.rs` (Debanding, Decoder) |
| `PlayerActivity.kt` | `window.rs` (lifecycle, GTK), `controller.rs` (fileLoaded, changeEpisode, setVideo, setMpvOptions/MediaTitle/Chapters/Tracks), `mpris.rs` (media session), `mini.rs` (PiP) |
| `PlayerObserver.kt` | `observer.rs` |
| `ExternalIntents.kt` | `external.rs` (intents), `progress.rs` (save logic) |
| `PlayerUtils.kt` | `source_url.rs` |
| `PipActions.kt` | `mini.rs` (actions and codes) |
| `PlayerEnums.kt` | `enums.rs` |
| `aniyomi.lua` | `bridge.rs` (protocol); Lua file retained only for custom buttons (P2) |
| `HttpServerService.kt` | `services/local_server.rs` |
| `TorrentServerService.kt`, `TorrentServerApi.kt`, `TorrentServerUtils.kt`, `TorrentPreferences.kt` | `services/torrent.rs` |

---

## 3. Key types

### 3.1 Player request (replaces intent extras)

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerRequest {
    pub anime_id: i64,          // -1 means "missing" and closes the window (behaviour 1.2)
    pub episode_id: i64,
    pub host_list: Option<Vec<Hoster>>, // typed; Kotlin serialises to a string extra
    pub host_index: Option<i32>,
    pub video_index: Option<i32>,
}
```

### 3.2 Enums

`enums.rs` mirrors `PlayerEnums.kt` one-to-one. Stored values use the enum name (`PreferenceStore::get_enum` in area 02 must store `name()` strings, matching `PreferenceStore.kt:27-45`).

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Debanding { #[default] None, Cpu, Gpu }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decoder { AutoCopy, Auto, Sw, Hw, HwPlus }

impl Decoder {
    pub const fn title(self) -> &'static str {
        match self { Self::AutoCopy | Self::Auto => "Auto", Self::Sw => "SW", Self::Hw => "HW", Self::HwPlus => "HW+" }
    }
    /// Android value as stored by the Kotlin app (data doc 2.9).
    pub const fn mpv_value(self) -> &'static str {
        match self {
            Self::AutoCopy => "auto-copy", Self::Auto => "auto", Self::Sw => "no",
            Self::Hw => "mediacodec-copy", Self::HwPlus => "mediacodec",
        }
    }
    /// Linux value (Proposal, section 6). Resolved against the running mpv at startup.
    pub const fn linux_mpv_value(self) -> &'static str {
        match self {
            Self::AutoCopy => "auto-copy", Self::Auto => "auto", Self::Sw => "no",
            Self::Hw => "vaapi-copy", Self::HwPlus => "vaapi",
        }
    }
}

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VideoAspect { Crop, #[default] Fit, Stretch }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SingleActionGesture { None, Seek, #[default] PlayPause, Switch, Custom }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum CustomKeyCode { DoubleTapLeft = 0x10001, DoubleTapCenter = 0x10002, DoubleTapRight = 0x10003,
                         MediaPrevious = 0x10004, MediaPlay = 0x10005, MediaNext = 0x10006 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetAsArt { Success, AddToLibraryFirst, Error }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtType { Cover, Background, Thumbnail }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet { None, PlaybackSpeed, SubtitleTracks, AudioTracks, QualityTracks, Chapters, More, Screenshot }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel { None, SubtitleSettings, SubtitleDelay, AudioDelay, VideoFilters }

#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    None,
    EpisodeList,
    IntegerPicker(IntegerPicker),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IntegerPicker {
    pub default_value: i32, pub min_value: i32, pub max_value: i32, pub step: i32,
    pub name_format: String, pub title: String,
}
// onChange / onDismissRequest become closures held by the dialog widget, not data.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoFilter { Brightness, Saturation, Contrast, Gamma, Hue }

impl VideoFilter {
    pub const fn mpv_property(self) -> &'static str {
        match self {
            Self::Brightness => "brightness", Self::Saturation => "saturation", Self::Contrast => "contrast",
            Self::Gamma => "gamma", Self::Hue => "hue",
        }
    }
    pub const fn pref_key(self) -> &'static str {
        match self {
            Self::Brightness => "pref_player_filter_brightness", Self::Saturation => "pref_player_filter_saturation",
            Self::Contrast => "pref_player_filter_contrast", Self::Gamma => "pref_player_filter_gamma",
            Self::Hue => "pref_player_filter_hue",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioChannels { Auto, AutoSafe, Mono, Stereo, ReverseStereo }
// property/value pairs: see data doc 2.9 (Auto -> ("audio-channels","auto-safe"), AutoSafe -> ("audio-channels","auto"), Mono -> ("audio-channels","mono"), Stereo -> ("audio-channels","stereo"), ReverseStereo -> ("af","pan=[stereo|c0=c1|c1=c0]"))

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubtitleBorderStyle { OutlineAndShadow, OpaqueBox, BackgroundBox }   // "outline-and-shadow" | "opaque-box" | "background-box"

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubtitleJustification { Left, Center, Right, Auto }                  // "left" | "center" | "right" | "auto"

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerOrientation { Free, Video, Portrait, ReversePortrait, SensorPortrait, Landscape, ReverseLandscape, SensorLandscape }
// Dropped on Linux (T-45a-035); kept only so a stored value can be read and ignored.
```

### 3.3 MPV events

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum MpvEvent {
    FileLoaded,
    Seek,
    PlaybackRestart,
    EndFile { eof_reached: bool, error: Option<String> },
    LogLine { prefix: String, level: LogLevel, text: String },
    Prop(PropValue),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropValue {
    None(&'static str),                 // chapter-list, track-list (no payload)
    Int(&'static str, i64),
    Flag(&'static str, bool),
    Str(&'static str, String),
    Double(&'static str, f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel { Error, Warn, Info, Debug }   // FATAL/ERROR->Error, WARN->Warn, INFO->Info, other->Debug
```

`EndFile.error` replaces the Kotlin `efEvent(err)`. When `error` is `None` the text is `Error: File ended`.

The property table is a `const` array of `(name, Format)` with 34 entries in the order of `AniyomiMPVView.kt:210-252`:

```rust
pub const OBSERVED: &[(&str, PropFormat)] = &[
    ("chapter", PropFormat::Int), ("chapter-list", PropFormat::None), ("track-list", PropFormat::None),
    ("time-pos", PropFormat::Int), ("demuxer-cache-time", PropFormat::Int), ("duration", PropFormat::Int),
    ("volume", PropFormat::Int), ("volume-max", PropFormat::Int),
    ("sid", PropFormat::Str), ("secondary-sid", PropFormat::Str), ("aid", PropFormat::Str),
    ("speed", PropFormat::Double), ("video-params/aspect", PropFormat::Double),
    ("hwdec-current", PropFormat::Str), ("hwdec", PropFormat::Str),
    ("pause", PropFormat::Flag), ("paused-for-cache", PropFormat::Flag), ("seeking", PropFormat::Flag), ("eof-reached", PropFormat::Flag),
    // 14 user-data/aniyomi/* string properties, then user-data/current-anime/intro-length (Int)
];
```

Count check: 19 entries before the user-data group (3 NONE, 16 others) plus 14 `user-data/aniyomi/*` plus `user-data/current-anime/intro-length` = 34. The `chapter` entry (INT64, handler commented out in Kotlin) is kept so the set matches.

### 3.4 Controller state

```rust
pub struct PlayerState {
    pub is_exiting: bool,            // behaviour 1.6 / 6.4
    pub paused: Option<bool>,        // mirrors "pause" property
    pub is_loading: bool,            // seek / paused-for-cache / seeking
    pub is_loading_episode: bool,
    pub is_loading_hosters: bool,
    pub is_loading_tracks: bool,
    pub duration: f32,               // seconds, 0 when unknown
    pub pos: f32,                    // seconds
    pub playback_speed: f32,
    pub sheet: Sheet,
    pub panel: Panel,
    pub dialog: Dialog,
    pub pip_active: bool,            // mini-player visible
    pub keep_awake: Option<IdleCookie>,
    pub restore_volume_on_resume: bool,
}
```

`IdleCookie` is the `u32` returned by `gtk::Application::inhibit` and is released with `uninhibit`.

### 3.5 Mpv handle

```rust
pub struct MpvHandle {
    inner: libmpv2::Mpv,                       // created once per window
    render: Option<libmpv2::render::RenderContext>,
}

impl MpvHandle {
    pub fn create(opts: &[MpvSetting], config_dir: &Path, cache_dir: &Path, log_level: &str) -> Result<Self, MpvError>;
    pub fn set_option(&self, name: &str, value: &str) -> Result<(), MpvError>;
    pub fn set_property_str(&self, name: &str, value: &str) -> Result<(), MpvError>;
    pub fn set_property_i64(&self, name: &str, value: i64) -> Result<(), MpvError>;
    pub fn set_property_f64(&self, name: &str, value: f64) -> Result<(), MpvError>;
    pub fn set_property_flag(&self, name: &str, value: bool) -> Result<(), MpvError>;
    pub fn get_i64(&self, name: &str) -> Option<i64>;
    pub fn get_f64(&self, name: &str) -> Option<f64>;
    pub fn get_flag(&self, name: &str) -> Option<bool>;
    pub fn get_string(&self, name: &str) -> Option<String>;
    pub fn get_node(&self, name: &str) -> Option<serde_json::Value>;   // used for "metadata"
    pub fn command(&self, args: &[&str]) -> Result<(), MpvError>;
    pub fn observe(&self, name: &str, fmt: PropFormat) -> Result<(), MpvError>;
    pub fn drain_events(&mut self) -> Vec<MpvEvent>;                   // main thread only
    pub fn destroy(self);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MpvSetting {
    Option(&'static str, String),        // mpv_set_option_string, applied before init
    Property(&'static str, String),      // mpv_set_property_string, applied after init (e.g. pause)
    Flag(&'static str, bool),            // mpv_set_property with yes/no flag
}
```

Ordering: `options::startup_settings(&PlayerPrefs)` returns the ordered `Vec<MpvSetting>`; the `MpvHandle::create` applies all `Option` entries before `mpv_initialize`, then `Property`/`Flag`. This keeps the Kotlin ordering where `pause` is set before options (behaviour 2.2 step 2 is a property set; the order inside the set is preserved).

Important: libmpv render-API output. The Kotlin embeds with `setVo("gpu")` on a surface it owns. With a `gtk::GLArea` and the libmpv render API, the video output must be the render output (`vo=libmpv`). The `gpu` / `gpu-next` preference then selects the renderer used inside that output. Verify the exact option name against the libmpv version installed before shipping (see pitfalls, section 9).

### 3.6 Preference snapshot (pure input for options)

```rust
#[derive(Clone, Debug)]
pub struct PlayerPrefs {
    pub try_hwdec: bool,                  // pref_try_hwdec, default true
    pub gpu_next: bool,                   // pref_gpu_next, default false
    pub debanding: Debanding,             // pref_video_debanding, default None
    pub use_yuv420p: bool,                // use_yuv420p, default true
    pub verbose_logging: bool,            // verbose_logging, default false
    pub player_speed: f32,                // pref_player_speed, default 1.0
    pub filters: [i32; 5],                // brightness, saturation, contrast, gamma, hue; default 0
    pub sub: SubtitlePrefs,
    pub audio: AudioPrefs,
    pub stats_page: i32,                  // pref_player_statistics_page, default 0
    pub mpv_user_files: bool,             // mpv_scripts, default false
    pub screenshot_dir: PathBuf,
    pub data_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct SubtitlePrefs {
    pub delay_ms: i32, pub speed: f32, pub secondary_delay_ms: i32, pub font: String,
    pub font_size: i32, pub bold: bool, pub italic: bool, pub justify: SubtitleJustification,
    pub text_color_argb: i32, pub back_color_argb: i32, pub border_color_argb: i32,
    pub border_size: i32, pub border_style: SubtitleBorderStyle, pub shadow_offset: i32,
    pub pos: i32, pub scale: f32, pub override_ass: bool,
}

#[derive(Clone, Debug)]
pub struct AudioPrefs {
    pub lang: String, pub delay_ms: i32, pub pitch_correction: bool,
    pub channels: AudioChannels, pub volume_boost_cap: i32,
}
```

Defaults for every field are in `p1-data-and-config.md` section 3. `impl Default for PlayerPrefs` in `options.rs` must reproduce the golden list of section 4 of that document. Tests use `PlayerPrefs::default()`.

### 3.7 Local server

```rust
pub struct LocalServer { port: u16, shutdown: tokio::sync::oneshot::Sender<()>, handle: tokio::task::JoinHandle<()> }

impl LocalServer {
    /// Binds 127.0.0.1:0. Returns Err when bind fails (the Kotlin swallows this; see behaviour 17.1).
    pub async fn start(handler: Arc<dyn StreamHandler>) -> Result<Self, std::io::Error>;
    pub fn port(&self) -> u16;
    pub fn url(&self) -> String { format!("http://localhost:{}", self.port) }
    pub fn stop(self);
}

#[async_trait::async_trait]
pub trait StreamHandler: Send + Sync {
    async fn serve(&self, req: http::Request<()>) -> http::Response<Body>;
}
```

`async_trait` may be replaced by a native `async fn` in trait when the MSRV allows; the signature is what matters.

### 3.8 Torrent

```rust
pub struct TorrentConfig { pub port: u16, pub data_dir: PathBuf, pub proxy_mode: ProxyMode, pub proxy_url: String, pub trackers: Vec<String> }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyMode { None, Tracker, Peers, Full }   // wire values: "tracker", "tracker", "peers", "full"

pub struct TorrentFile { pub id: Option<i32>, pub path: String, pub length: i64 }
pub struct Torrent { pub title: String, pub hash: Option<String>, pub torrent_size: Option<i64>, pub trackers: Option<Vec<String>>, pub file_stats: Option<Vec<TorrentFile>> }

pub trait TorrentEngine: Send + Sync {
    fn start(&self, cfg: &TorrentConfig) -> Result<u16, TorrentError>;   // returns bound port
    fn stop(&self);
}

pub struct TorrentApi { base: String /* http://127.0.0.1:<port> */, http: reqwest::Client }

impl TorrentApi {
    pub async fn echo(&self) -> String;                              // "" on any error
    pub async fn add(&self, link: &str, title: &str, save: bool) -> Result<Torrent, TorrentError>;
    pub async fn upload(&self, bytes: Vec<u8>, title: &str, save: bool) -> Result<Torrent, TorrentError>;
}

/// PURE: Kotlin TorrentServerUtils.getTorrentPlayLink
pub fn play_link(port: u16, torrent: &Torrent, index: i32) -> String;
/// PURE: magnet "index=" parsing, PlayerActivity.kt:1175-1183
pub fn magnet_index(url: &str) -> i32;
```

`TorrentEngine` exists because the engine choice is open (topic doc `p1-topic-local-servers.md`). Two implementations are proposed: `TorrServerProcess` (spawn the TorrServer binary and talk HTTP, matching the Kotlin API) and `LibrqbitEngine` (pure Rust, requires a different streaming route).

---

## 4. Async and threading model

Two executors, one owner for each kind of work:

| Work | Executor | Notes |
|---|---|---|
| GTK widgets, mpv handle, mpv property reads and commands | GTK main loop (`glib::MainContext::default()`) | libmpv handle is `!Send` in this design. All calls on the main thread. |
| mpv event pump | `glib::idle_add_local` or `wakeup_callback` → main thread | `mpv_set_wakeup_callback` posts a glib source that calls `drain_events()`. |
| DB writes (history, progress, tracking) | `tokio` runtime (multi-thread) via `spawn_blocking` for `rusqlite` | Results come back with `glib::MainContext::channel`. |
| Network (torrent API, local server, tracker calls) | `tokio` runtime | |
| Metadata JSON, file copies | `tokio::task::spawn_blocking` | |
| MPRIS | `zbus` on the tokio runtime | Commands marshalled to the main loop. |

Rules:

- No `block_on` on the main thread. The Kotlin `runBlocking` aniSkip call in `fileLoaded` (`PlayerActivity.kt:1290`) becomes an async task whose result is applied on the main thread (behaviour 4.2 step 7).
- Every message from another executor goes to the main loop through one channel `AppEvent::Player(PlayerEvent)`.
- Shutdown order (behaviour 1.8) is implemented in `Drop for PlayerWindow`, not in a global handler.

```rust
pub enum PlayerEvent {
    Mpv(MpvEvent),
    Toast(String),
    Save(Result<PathBuf, String>),
    ShareReady(PathBuf, String),
    SetArt(SetAsArt, ArtType),
    TrackerUpdated(i64 /*anime*/, Result<(), String>),
    ProgressSaved,
    Exit,
}
```

---

## 5. Controller state machine and file_loaded

`PlayerController` owns `PlayerState`, the `MpvHandle`, the `PlayerModel` (part 2), and the event receiver. The state machine (behaviour 1.12) is enforced in one function:

```rust
impl PlayerController {
    pub fn on_mpv_event(&mut self, ev: MpvEvent) {
        if self.state.is_exiting && !matches!(ev, MpvEvent::PlaybackRestart) { return; } // see 6.4 proposal
        match ev {
            MpvEvent::FileLoaded => self.file_loaded(),
            MpvEvent::Seek => self.state.is_loading = true,
            MpvEvent::PlaybackRestart => self.state.is_exiting = false,
            MpvEvent::EndFile { eof_reached, error } => self.end_file(eof_reached, error),
            MpvEvent::LogLine { .. } => {}
            MpvEvent::Prop(p) => self.on_prop(p),
        }
    }

    fn file_loaded(&mut self) {
        // order from behaviour 4.2
        self.apply_metadata_options();     // 4.3 (skip malformed entries; see behaviour 23.7)
        self.set_media_title();            // 4.4
        // orientation: dropped (T-45a-035)
        self.setup_chapters();             // 4.5
        self.setup_tracks();               // 4.6
        self.model.waiting_skip_intro(/* pref */);
        self.spawn_ani_skip();             // async; applies chapters on main loop when ready
    }
}
```

Design choice (proposal): `on_mpv_event` checks `PlaybackRestart` before the exiting gate. This is the fix recorded in behaviour 6.4. The Kotlin behaviour (flag stuck until `onResume`) is not replicated.

Episode switching (`change_episode`, behaviour 7.1) is one `async fn` that returns a `SwitchOutcome` enum: `NoNextEpisode`, `EmptyHosters`, `Loaded { title }`. The toast for `NoNextEpisode` is suppressed when `auto_play` is true.

---

## 6. Hardware decoding on Linux (Proposal)

The Kotlin `Decoder` values are Android MediaCodec names. Linux equivalents (for `hwdec`):

| Kotlin `Decoder` | Android value | Linux `hwdec` value (proposal) | Notes |
|---|---|---|---|
| AutoCopy | `auto-copy` | `auto-copy` | same name on mpv |
| Auto | `auto` | `auto` | default when `pref_try_hwdec` = true |
| SW | `no` | `no` | software decoding |
| HW | `mediacodec-copy` | `vaapi-copy` on Intel/AMD, `nvdec-copy` on NVIDIA | copy-back mode, safe with the GL render path |
| HWPlus | `mediacodec` | `vaapi` / `nvdec` (zero-copy) | needs GL interop; falls back to copy mode on failure |

Selection rule (proposal): read the mpv `hwdec` candidate list from the running mpv (`hwdec-interop` property) and pick the first matching `*-copy` for HW, zero-copy for HWPlus. The preference stores the Kotlin enum name; the Linux mapping is internal.

---

## 7. Android API to Linux mapping

| Android / Kotlin | Linux / Rust replacement | Task |
|---|---|---|
| `MPVLib` / `BaseMPVView` (aniyomi-mpv-lib 1.18.n) | `libmpv2` crate, render API | T-45a-002 |
| `SurfaceView` / `binding.player` | `gtk4::GLArea` + mpv render context | T-45a-002 |
| `Handler`, `runOnUiThread`, `lifecycleScope` | glib main loop + `glib::MainContext::channel` | T-45a-007 |
| `Dispatchers.IO`, `launchIO`, `viewModelScope` | `tokio` runtime, `spawn_blocking` | section 4 |
| `Injekt.get<T>()` | explicit constructor injection in `AppState` | section 2 |
| `AudioManager`, `AudioFocusRequestCompat` | Drop (T-45a-042) | T-45a-042 |
| `MediaSession`, `PlaybackState` | MPRIS via `zbus` (or the `mpris-server` crate on zbus; verify version) | T-45a-043 |
| `ACTION_AUDIO_BECOMING_NOISY` broadcast | PipeWire/PulseAudio sink-removal event (libpulse subscription or pw registry) | T-45a-082 |
| `PictureInPictureParams`, `enterPictureInPictureMode` | Drop (T-45a-047); mini-player `gtk::Window` | T-45a-048 |
| `RemoteAction`, `PendingIntent` broadcast codes | `mini.rs` action enum + glib action | T-45a-048 |
| `FLAG_KEEP_SCREEN_ON`, `PowerManager` | `gtk::Application::inhibit(Some(&win), InhibitFlags::IDLE, reason)`; `uninhibit(cookie)` | T-45a-041 |
| `WindowInsetsController`, immersive flags | `gtk::Window::fullscreen()` / `unfullscreen()` | T-45a-044 |
| `Settings.System` brightness | Drop (T-45a-045) | T-45a-045 |
| `ActivityInfo.SCREEN_ORIENTATION_*` | Drop (T-45a-035); `video-rotate` property | T-45a-035 |
| `Intent(ACTION_VIEW)` external launch | `gio::AppInfo::launch_default_for_uri` or `std::process::Command` | T-45a-055 |
| `FileProvider` URI | plain path | T-45a-068 |
| `ContentResolver`, `ParcelFileDescriptor`, `content://` | Drop; `file://` only | T-45a-081 |
| `ForegroundService`, `startForeground`, `START_STICKY` | owned tasks in `AppState` | T-45a-080 |
| `NotificationCompat` with actions | `gio::Notification` with `add_button` (org.freedesktop.Notifications) | T-45a-071, T-45a-079 |
| `Toast` | `gtk::Overlay` pill (the mockup's `pill()` helper in `src/player.rs`) | T-45a-009 |
| `Environment.DIRECTORY_PICTURES` | `glib::user_special_dir(glib::UserDirectory::Pictures)` | T-45a-081 |
| `UniFile` | `std::fs` | T-45a-026 |
| `SharedPreferences` / `PreferenceStore` | SQLite-backed store (area 02) | section 3.6 |
| `Thread.setDefaultUncaughtExceptionHandler` | Drop (T-45a-054); `std::panic::set_hook` for logging only | T-45a-054 |
| NanoHTTPD (`HttpServer`) | `hyper` 1.x on tokio, bound to `127.0.0.1` | T-45a-069 |
| `TorrServer` (Go, `xyz.secozzi.torrserver`) | `TorrentEngine` trait (process or librqbit) | T-45a-072 |
| `OkHttp` client | `reqwest` | T-45a-073 |
| `kotlinx.serialization` JSON | `serde` / `serde_json` | data doc section 2 |
| `logcat` | `tracing` with target `mpv/<prefix>` | T-45a-008 |
| `Uri.toUri()`, `Uri.path` | `url::Url` or `std::path` | T-45a-010 |
| `URLEncoder.encode(name, "utf8")` | `percent-encoding` with the form-urlencoded set (space as `+`) | T-45a-075 |
| `MessageDigest` (SHA-1 info hash) | out of scope (area 13) | n/a |
| `LocaleHelper.getSimpleLocaleDisplayName()` | `glib::language_names()` first entry, simple name | T-45a-058 |

---

## 8. Keyboard mapping (keys.rs)

`keys.rs` is pure. Input: GTK key name (from `gdk::Key::name()`), unicode char if printable, and modifier flags. Output: the mpv key name, or `None`.

Table (proposal, mpv key names):

| GTK key | mpv key |
|---|---|
| Left, Right, Up, Down | LEFT, RIGHT, UP, DOWN |
| space | SPACE |
| Return, KP_Enter | ENTER, KP_ENTER |
| Escape | ESC |
| Tab, ISO_Left_Tab | TAB |
| BackSpace | BS |
| Delete | DEL |
| Insert | INS |
| Home, End | HOME, END |
| Page_Up, Page_Down | PGUP, PGDWN |
| F1..F24 | F1..F24 |
| KP_0..KP_9 | KP0..KP9 |
| KP_Decimal | KP_DEC |
| KP_Add, KP_Subtract, KP_Multiply, KP_Divide | (no entry; falls back to the printable char) |
| XF86AudioPlay | PLAY |
| XF86AudioPause | PAUSE |
| XF86AudioPlayPause | PLAYPAUSE |
| XF86AudioStop | STOP |
| XF86AudioNext | NEXT |
| XF86AudioPrev | PREV |
| XF86AudioForward | FORWARD |
| XF86AudioRewind | REWIND |
| XF86AudioRaiseVolume | VOLUME_UP |
| XF86AudioLowerVolume | VOLUME_DOWN |
| XF86AudioMute | MUTE |

Modifier prefixes: `shift+`, `ctrl+`, `alt+`, `meta+` (Super maps to `meta`), in that order. Repeat events are swallowed (behaviour 8.4). Printable characters: the character itself; `#` becomes `SHARP` (proposal: avoids the input.conf comment character); dead keys are ignored.

Note on Android key names in the Kotlin: `KeyMapping.map` (external) is not in the repo. The table above is the Linux proposal; the behaviour is covered by tests in `p1-edge-cases-and-tests.md`.

---

## 9. Pitfalls

1. Render API and `vo`. `vo=gpu` with a `wid` is an X11/Wayland-surface pattern. With `gtk::GLArea` the render API is used and `vo` must be `libmpv`. The Kotlin `setVo("gpu")` value is not copied as-is; `gpu`/`gpu-next` in preferences selects the internal renderer. Confirm against the installed libmpv (`mpv --version`, `vo=help`).
2. GL context. `mpv_render_context_render` must run with the GLArea's context current. Draw only in the GLArea `render` signal; call `mpv_render_context_report_swap` after each swap.
3. Update callback. `mpv_render_context_set_update_callback` must not call mpv functions directly. It posts a glib idle that calls `render` on the main thread.
4. `metadata` is a node, not a string. The Kotlin reads it as a JSON string through `getPropertyString("metadata")`. The Rust port should read the `MPV_FORMAT_NODE` map (`get_node`), which avoids a JSON round trip.
5. `NONE` format observers. `chapter-list` and `track-list` are observed with `MPV_FORMAT_NONE` (change notification only). Keep this in `observe`.
6. Command quoting. The Kotlin builds the per-file options string as `key="value",...` with no escaping (`PlayerActivity.kt:1089-1091`), so a value containing `"` or `,` breaks the load. The Rust port escapes each value for mpv option syntax before building the `loadfile` options argument (or sets the properties immediately before `loadfile`). Tests cover values with quotes and commas.
7. `hwdec-current` before file load is empty; treat empty as `no` (behaviour 6.5, `hwdecActive`).
8. Tracks `aid`/`sid` are strings in observers (`"no"`, `"auto"`, numbers). Parse with `parse::<i64>()` and treat other strings as "no change" (behaviour 6.6).
9. `video-params/aspect` may be absent for a few frames. Guard the PiP rational (behaviour 23.5); the Rust `Rational` denominator must not be zero.
10. Dropping the handle. `Drop for MpvHandle` must free the render context before `mpv_terminate_destroy`.
11. Wayland and NVIDIA. `gpu-api=opengl` and `gpu-context=wayland` may be needed on some drivers. Make both overridable through the user's `mpv.conf` (written in T-45a-026, read by mpv after the Rust defaults, so user values win).
12. `glib` main context and blocking calls. Never call `mpv_wait_event` with a non-zero timeout on the main thread. Use the wakeup callback and `timeout=0`.

---

## 10. Integration with the existing mockup

Mockup facts (from `src/`):

- `src/player.rs:154` `pub fn player(title: &str, episode: &str) -> GBox` is the placeholder. It builds an `Overlay` with a `surface` `GBox` (`osd` CSS class) and a bottom control bar. Its callers are `src/screens.rs:38` (`"player" => return crate::player::player("Title 1", "Episode 1")`) and `src/screens.rs:67` (`open_media`).
- `src/player.rs:7-30` helpers: `sheet(screen_id, sheet_id)`, `dialog(screen_id, dialog_id)`, `osd(o, spacing)`, `on_long_press(widget, f)`; `src/player.rs:40` `pill(overlay, text)` is the toast host.
- `src/main.rs:54-58` puts `screen:player` into immersive mode (no sidebar or tab bar).
- `spec/reader_player.json` supplies the sheet and dialog ids used by `sheet("player", ...)` (`playback_speed`, `chapters`, `screenshot`, `episode_list`).

Planned changes (no code here, tasks reference them):

1. `src/player/window.rs` replaces the placeholder `player()`. Signature: `pub fn player_window(app: &gtk::Application, req: PlayerRequest, state: AppStateRef) -> PlayerWindow`. The existing overlay control layout is kept and wired to the `PlayerController` (part 2 owns the controls).
2. `src/screens.rs:38` and `src/screens.rs:67` call `player_window` instead of `player` (the mockup's fixed title/episode are replaced by the request's anime and episode).
3. `src/main.rs:58` immersive logic stays; it now also covers the mini-player when visible (not in scope for P0).
4. `src/ui.rs` helpers (`push`, `pop`, `open`) keep handling navigation; the player is pushed as a page, and "Back" pops it. The Android `onBackPressed` PiP path is dropped (T-45a-047).
5. Toasts use the existing `pill()` helper (`src/player.rs:40`).
6. Sheets and dialogs keep the `sheet(...)` / `dialog(...)` names from `spec/reader_player.json`.

Missing in the mockup (for `p1-ui-parity.md` and tasks): every real behaviour listed in sections 2-9 above. The mockup has no mpv, no keyboard handling, no state, no toasts with real messages, no external launch, no servers.

---

## 11. Error handling

- `thiserror` enums per module: `MpvError`, `PlayerError`, `TorrentError`, `ExternalError`, `ServerError`.
- `anyhow` at the window boundary only, for logging and toasts.
- Every user-visible failure has a message key (behaviour sections 9 and 13). Missing keys are a compile-time error by using a `enum UserMessage` with a `fn text(&self) -> &'static str` that returns the English text from `p1-data-and-config.md` section 9.
- No `unwrap` / `expect` on mpv results in production code paths. Replace Kotlin `!!` sites (behaviour 23.5, 23.6) with explicit `Option` handling.

---

## 12. Logging

- `tracing` targets: `player`, `player::mpv`, `player::external`, `services::torrent`, `services::local_server`.
- mpv log lines use target `mpv::<prefix>` and level per `LogLevel` (T-45a-008).
- Default filter: `info` for app, `warn` for mpv (matching `msg-level all=warn`), `v` (debug) for mpv when `verbose_logging` is on.
