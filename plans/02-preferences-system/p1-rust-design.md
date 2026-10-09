# Area 02 preferences system, part 1: Rust design

Everything here is Proposal unless marked OBSERVED. The design implements the contract in `p1-behavior.md` and the formats in `p1-data-and-config.md`. It follows the cross-platform rules of `plans/addenda/requirements-cross-platform.md` (all OS access through `kuro-platform`, no `cfg(target_os)` here).

## 1. Crates and modules

New workspace crate `kuro-prefs` (no GTK dependency, unit-testable headless). The current repo is a single binary crate (`/home/blackbox/kurobayomi/Cargo.toml`, edition 2024, gtk4 0.11.5, serde, serde_json); area:01-foundation-app-bootstrap turns it into a workspace, this area adds `crates/kuro-prefs` and the GTK glue module `kuro-ui::bind` (or `src/bind.rs` until the workspace exists).

```
crates/kuro-prefs/
  Cargo.toml
  src/
    lib.rs            // pub use; `Prefs` aggregate
    value.rs          // PrefValue, Namespace, key prefixes
    codec.rs          // PrefType, Decoded<T>, enum/object codecs, pref_enum! macro
    pref.rs           // Pref<T> handle, set helpers (insert/remove/toggle/update)
    store.rs          // PrefStore (in-memory map + change bus + flush scheduling)
    backend/
      mod.rs          // trait Backend
      toml.rs         // TomlBackend: config.toml + state.toml, atomic writes, lock
      memory.rs       // MemoryBackend for tests (replaces InMemoryPreferenceStore)
    layout.rs         // key -> (toml table path, field) mapping for registry + dynamic keys
    secrets.rs        // SecretPref over kuro_platform::SecretStore
    tristate.rs       // TriState, CheckboxState
    sort.rs           // LibraryDisplayMode, MangaLibrarySort, AnimeLibrarySort, flag algebra
    flags.rs          // chapter/episode/season flag constants and helpers
    datefmt.rs        // java DateTimeFormatter pattern -> chrono
    schema/
      mod.rs          // define_prefs! macro and the `Prefs` struct
      base.rs ui.rs sources.rs tracking.rs library.rs downloads.rs backup.rs
      storage.rs security.rs network.rs torrent.rs reader.rs
      player.rs       // player, gestures, decoder, audio, subtitles, advanced sub-structs
    source.rs         // SourcePrefs (per-source typed store)
    backup.rs         // export/import of preference entries (neutral structs; protobuf in area:21-backup-format-and-create)
    legacy.rs         // legacy key rewrites (area:11-app-migrations hook) and Aniyomi key import table
```

Dependencies (stable crates, MSRV = current stable):

| Crate | Why |
|---|---|
| `serde` (derive) | TOML (de)serialisation of enums and structs |
| `toml_edit` | read-modify-write that preserves unknown keys, comments and order; fits the "unknown keys survive" rule |
| `tokio` (features `sync`, `rt`, `time`, `macros`) | `broadcast` for change events (works on any executor, including the GLib main loop), `time` for flush debounce |
| `parking_lot` | cheap `RwLock` for the value map |
| `thiserror` | `PrefError` |
| `tracing` | logs ("Invalid value for key; deleting") |
| `atomic-write-file` | atomic replace that also works on Windows |
| `fd-lock` | advisory lock file so two processes never write the same config |
| `sys-locale` | system language for the `source_languages` default |
| `chrono` | date formatting |
| `kuro-platform` (workspace) | `Paths`, `SecretStore` traits (never `directories`/`keyring` directly) |

`proptest` and `tempfile` are dev-dependencies.

## 2. Core types

```rust
// value.rs
use std::collections::BTreeSet;

pub const APP_STATE_PREFIX: &str = "__APP_STATE_";
pub const PRIVATE_PREFIX: &str = "__PRIVATE_";

#[derive(Clone, Debug, PartialEq)]
pub enum PrefValue {
    Int(i32),
    Long(i64),
    Float(f32),
    Str(String),
    Bool(bool),
    Set(BTreeSet<String>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Namespace { Normal, AppState, Private }

impl Namespace {
    pub fn of(key: &str) -> Namespace {
        if key.starts_with(APP_STATE_PREFIX) { Namespace::AppState }
        else if key.starts_with(PRIVATE_PREFIX) { Namespace::Private }
        else { Namespace::Normal }
    }
}
pub fn is_private(key: &str) -> bool { key.starts_with(PRIVATE_PREFIX) }
pub fn is_app_state(key: &str) -> bool { key.starts_with(APP_STATE_PREFIX) }
pub fn private_key(k: &str) -> String { format!("{PRIVATE_PREFIX}{k}") }
pub fn app_state_key(k: &str) -> String { format!("{APP_STATE_PREFIX}{k}") }
```

```rust
// codec.rs
pub enum Decoded<T> {
    Ok(T),
    /// Stored value has a different primitive kind: Kotlin ClassCastException path,
    /// the key is deleted and the default returned.
    WrongType,
    /// Right kind but not decodable (unknown enum name, bad object string):
    /// default returned, key kept (Kotlin getEnum / Object.read behaviour).
    Invalid,
}

pub trait PrefType: Clone + PartialEq + Send + Sync + 'static {
    fn encode(&self) -> PrefValue;
    fn decode(v: &PrefValue) -> Decoded<Self>;
}
// impls: bool (Bool), i32 (Int), i64 (Long), f32 (Float), String (Str), BTreeSet<String> (Set)
```

Note on kinds: Kotlin keeps `Int` and `Long` distinct. A value written as `Long` and read as `Int` throws `ClassCastException` in `SharedPreferences`; in Rust `decode` for `i32` accepts only `PrefValue::Int`. Because TOML integers carry no width, `TomlBackend` loads each field using the registry kind, so a registry field of kind Int loads as `PrefValue::Int` (and out-of-range integers load as `WrongType`).

Enums:

```rust
#[macro_export]
macro_rules! pref_enum {
    ($(#[$m:meta])* pub enum $name:ident { $($var:ident),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($var),+ }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            pub fn name(self) -> &'static str { match self { $($name::$var => stringify!($var)),+ } }
            pub fn from_name(s: &str) -> Option<$name> { match s { $(stringify!($var) => Some($name::$var),)+ _ => None } }
        }
        impl $crate::codec::PrefType for $name {
            fn encode(&self) -> $crate::value::PrefValue { $crate::value::PrefValue::Str(self.name().to_owned()) }
            fn decode(v: &$crate::value::PrefValue) -> $crate::codec::Decoded<Self> {
                match v {
                    $crate::value::PrefValue::Str(s) => match Self::from_name(s) {
                        Some(x) => $crate::codec::Decoded::Ok(x),
                        None => $crate::codec::Decoded::Invalid,
                    },
                    _ => $crate::codec::Decoded::WrongType,
                }
            }
        }
    };
}
```

Variant identifiers are the exact Kotlin names (`ToggleSeen`, `SensorLandscape`, `ENABLED_IS`, `MOVE_HISTORY_TO_MORE`), so `#[allow(non_camel_case_types)]` is placed on the SCREAMING enums (`TriState`, `AppTheme`, `NavStyle`, `StartScreen`, `ThemeMode`, `DataSaver`, `AutoTrackState`, `SecureScreenMode`, `FlashColor`, `TappingInvertMode`, `ReaderHideThreshold`, `TabletUiMode`, `MigrateSortMode`, `MigrateSortDirection`, `ExtensionInstaller`). Enum lists are in `p1-data-and-config.md` section 4.

Objects (string codec):

```rust
impl PrefType for LibraryDisplayMode {   // "COMPACT_GRID" | "COMFORTABLE_GRID" | "LIST" | "COVER_ONLY_GRID"
    fn encode(&self) -> PrefValue { PrefValue::Str(self.serialize().to_owned()) }
    fn decode(v: &PrefValue) -> Decoded<Self> {
        match v { PrefValue::Str(s) => Decoded::Ok(Self::deserialize(s)) /* unknown -> default, like Kotlin */, _ => Decoded::WrongType }
    }
}
// MangaLibrarySort / AnimeLibrarySort: Str("TYPE,DIR"); decode never fails (empty or bad -> default).
```

Handles:

```rust
// pref.rs
use std::borrow::Cow;
pub struct Pref<T: PrefType> {
    store: PrefStore,
    key: Cow<'static, str>,
    default: T,
}
impl<T: PrefType> Pref<T> {
    pub fn key(&self) -> &str;
    pub fn default_value(&self) -> T;
    pub fn is_set(&self) -> bool;                 // store contains key
    pub fn get(&self) -> T;                       // Kotlin get(): WrongType => delete + default; Invalid => default
    pub fn set(&self, v: T);                      // writes, marks dirty, publishes Changed{key}
    pub fn delete(&self);                         // remove key, publishes Changed{key}
    pub fn delete_and_get(&self) -> T { self.delete(); self.get() }
    pub fn update(&self, f: impl FnOnce(T) -> T) { self.set(f(self.get())) }   // getAndSet
    pub fn changes(&self) -> Changes<T>;          // see section 4
}
impl Pref<bool> { pub fn toggle(&self) -> bool; }
impl Pref<BTreeSet<String>> {
    pub fn insert(&self, item: impl Into<String>);            // +=
    pub fn extend(&self, items: impl IntoIterator<Item = String>);   // += items
    pub fn remove(&self, item: &str);                         // -=
}
```

Every `Pref` is a cheap clone (an `Arc` bump and a key). Equal-value writes: `set` still marks the key dirty and still publishes, matching SharedPreferences (which notifies only when the value actually changed on some Android versions but not guaranteed); consumers must be idempotent. Proposal detail: `set` skips publish and flush when the new value equals the stored one, which is a safe strengthening because `changes()` always re-reads.

## 3. Store, backends and flushing

```rust
// store.rs
#[derive(Clone)]
pub struct PrefStore(Arc<Inner>);
struct Inner {
    values: parking_lot::RwLock<HashMap<String, PrefValue>>,   // full keys incl. prefixes (mirrors Aniyomi's flat namespace)
    tx: tokio::sync::broadcast::Sender<Changed>,               // capacity 256
    dirty: AtomicBool,
    flush_wake: tokio::sync::Notify,
    backend: Box<dyn Backend>,
}
#[derive(Clone, Debug)] pub struct Changed { pub key: Option<String> }   // None = everything changed (bulk restore, clear)

impl PrefStore {
    pub fn open(backend: Box<dyn Backend>) -> Result<PrefStore, PrefError>;   // loads, spawns flusher if a runtime is present
    pub fn get_raw(&self, key: &str) -> Option<PrefValue>;
    pub fn set_raw(&self, key: &str, v: PrefValue);
    pub fn remove(&self, key: &str);
    pub fn all(&self) -> Vec<(String, PrefValue)>;           // == PreferenceStore.getAll()
    pub fn replace_many(&self, entries: Vec<(String, PrefValue)>);   // one Changed{None}
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Changed>;
    pub async fn flush(&self) -> Result<(), PrefError>;      // forced (quit, before backup)
    pub fn pref<T: PrefType>(&self, key: impl Into<Cow<'static, str>>, default: T) -> Pref<T>;
}

pub trait Backend: Send + Sync {
    fn load(&self) -> Result<Vec<(String, PrefValue)>, PrefError>;
    fn save(&self, all: &[(String, PrefValue)]) -> Result<(), PrefError>;   // atomic
}
```

`TomlBackend` details:

1. `load()`: take `fd_lock` on `<config dir>/.lock` (second instance gets `PrefError::Locked`, the app shows "Another Kurobayomi window owns the settings" and exits via the single-instance path of `kuro-platform::SingleInstance`, normally preempted by it). Parse `config.toml` and `state.toml` into `toml_edit::DocumentMut`s kept in the backend. Walk `layout::FIELDS` (the registry) and decode each present field to `PrefValue` using the registry kind; dynamic families (`filter_tracked_*`, `auth_expired`) via `layout::FAMILIES`; every remaining top-level table (`[extra]`) loads as typed `{ t, v }` entries so unknown keys are retained. Missing file = empty store. Parse error = rename to `*.corrupt-<unix>` and start empty (message in `p1-data-and-config.md` 1.2).
2. `save(all)`: for each entry write into the matching table of the in-memory documents (create tables when missing, never touch comments of unrelated keys), keep defaults out of the file? Decision: write only explicitly set keys (Aniyomi semantics: `isSet()` is meaningful and defaults can change between versions). Removed keys are deleted from the document. Write both documents with `atomic-write-file`, mode 0600.
3. Flusher task (spawned on the tokio runtime owned by area:01-foundation-app-bootstrap): `loop { notified().await; sleep(250ms); if dirty.swap(false) { backend.save(snapshot) } }`. Snapshot is taken under the read lock then released before IO. Errors are logged with `tracing::error!` and re-mark dirty with exponential retry (1 s, 2 s, 4 s, capped at 30 s); after 3 failures a one-time toast `settings_save_failed` is queued for the UI.
4. `flush()` awaits an in-progress write and performs a final one; called from `gio::Application::connect_shutdown` (blocking on a short-lived runtime handle) and by the backup creator before reading `all()`.

`MemoryBackend` stores a `Mutex<Vec<...>>`; `PrefStore::open` on it never spawns the flusher. It replaces `InMemoryPreferenceStore` and, unlike the Kotlin class, supports sets and emits change events.

## 4. Change events and the GTK main loop

```rust
pub struct Changes<T> { rx: broadcast::Receiver<Changed>, pref: Pref<T>, started: bool }
impl<T: PrefType> Changes<T> {
    /// First call yields the current value immediately (Kotlin `onStart { emit("ignition") }`),
    /// then one value per matching Changed (own key or key == None), re-read with get().
    /// Lagged receivers (RecvError::Lagged) are treated as "something changed": re-read and continue (== conflate).
    pub async fn next(&mut self) -> Option<T>;
}
```

GTK integration (module `kuro_ui::bind`, GTK main thread only):

```rust
pub fn watch<T: PrefType>(p: &Pref<T>, f: impl Fn(T) + 'static) -> glib::SourceId-like guard {
    let mut ch = p.changes();
    glib::spawn_future_local(async move { while let Some(v) = ch.next().await { f(v) } })
}
```

`tokio::sync::broadcast` receivers are executor-agnostic, so `glib::spawn_future_local` (GLib main context) can await them; handlers therefore always run on the GTK thread and may touch widgets. Writers can be any thread (tokio tasks, `spawn_blocking`, GTK handlers). Drop the returned guard (stored on the widget with `connect_destroy` / `weak` ref) to stop watching so closed pages do not leak tasks.

Echo suppression: a bound widget that writes the preference receives its own change back. `bind_*` helpers compare the incoming value with the widget state and skip `set_active`/`set_value` when equal, and wrap programmatic updates in a `Cell<bool>` re-entrancy guard so `connect_active_notify` does not write back.

Widget binders (all in `kuro_ui::bind`):

| Function | Widget | Behaviour |
|---|---|---|
| `switch(row, &Pref<bool>)` | `gtk::Switch` | initial from `get()`, `notify::active` -> `set` |
| `dropdown<T: Clone + PartialEq>(dd, &Pref<T>, values: &[T])` | `gtk::DropDown` | index <-> value by position in `values`; value not in list selects index 0 without writing |
| `chips<T>(buttons, &Pref<T>, values)` | `ToggleButton` group | same |
| `scale_i32(scale, &Pref<i32>)`, `scale_f32` | `gtk::Scale` | writes on `value-changed` (the 250 ms flush debounce absorbs drags) |
| `entry(entry, &Pref<String>)` | `gtk::Entry` | writes on `changed`; debounced by flush only |
| `multi(checks: &[(CheckButton, String)], &Pref<BTreeSet<String>>)` | popover with checkboxes | toggling inserts/removes the value; the menu button label re-renders "N selected" |
| `tristate(check, &Pref<TriState>)` | `CheckButton` | state cycle `next()`: DISABLED -> ENABLED_IS (checked) -> ENABLED_NOT (inconsistent) -> DISABLED, matching the mockup's existing tristate widget |
| `sort(dd, dir_button, &Pref<MangaLibrarySort>)` | `DropDown` + direction button | |

## 5. Typed sections

All 18 Kotlin classes become fields of one aggregate, built once in the app context:

```rust
pub struct Prefs {
    pub base: BasePrefs, pub ui: UiPrefs, pub sources: SourcePrefs_, pub tracking: TrackingPrefs,
    pub library: LibraryPrefs, pub downloads: DownloadPrefs, pub backup: BackupPrefs, pub storage: StoragePrefs,
    pub security: SecurityPrefs, pub network: NetworkPrefs, pub torrent: TorrentPrefs,
    pub reader: ReaderPrefs, pub player: PlayerPrefs,
    store: PrefStore,
}
impl Prefs {
    pub fn open(paths: &dyn kuro_platform::Paths, secrets: Arc<dyn kuro_platform::SecretStore>) -> Result<Prefs, PrefError>;
    pub fn in_memory() -> Prefs;
    pub fn store(&self) -> &PrefStore;
}
```

Each section is declared once with a macro that generates the struct, its constructor, the registry entry (`PrefMeta`) and the TOML layout row, so key string, default, type, namespace and TOML field cannot drift:

```rust
define_prefs! {
    pub struct LibraryPrefs in "library" {
        display_mode: LibraryDisplayMode = "pref_display_mode_library" => LibraryDisplayMode::CompactGrid;
        manga_sorting_mode: MangaLibrarySort = "library_sorting_mode" => MangaLibrarySort::DEFAULT;
        auto_update_interval: i32 = "pref_library_update_interval_key" => 0;
        auto_update_device_restrictions: BTreeSet<String> = "library_update_restriction" => set!["wifi"];
        auto_update_item_restrictions: BTreeSet<String> = "library_update_manga_restriction"
            => set!["manga_fully_read", "manga_ongoing", "manga_started", "manga_outside_release_period"];
        last_updated_timestamp: i64 = app_state "library_update_last_timestamp" => 0;
        // dynamic family: key pattern with {id}
        family filter_tracked_manga(id: i32): TriState = "pref_filter_library_tracked_{id}_v2" => TriState::DISABLED;
        // the real invocation lists all 87 LibraryPreferences rows in registry order
    }
}
```

`PrefMeta { key, ns, kind, table, field, decision, priority }` is emitted into `REGISTRY: &[PrefMeta]`. The macro supports `app_state`, `private` and `family` forms. A unit test compares `REGISTRY` against `p1-topic-key-registry.md` rows (key, store, kind, default) parsed from the checked-in plan (task T-02a-001 acceptance), which also guarantees that all 328 keys exist.

Field naming rule: snake_case of the Kotlin function name, acronym runs kept together (`always_decode_long_strip_with_ssiv`, `try_hw_decoding`, `use_yuv420p`). The registry column "Rust field" is authoritative.

The deliberately dropped Android keys remain in the registry (kind, default) so a restored Aniyomi backup is preserved and re-exported unchanged; nothing in the app reads them.

`Prefs::in_memory()` plus helper `Prefs::with(|p| ...)` give tests a fully defaulted tree.

## 6. Secrets (tracker credentials)

```rust
pub struct SecretPref { store: Arc<dyn SecretStore>, kind: &'static str, tracker: i64, cache: Arc<Mutex<Option<String>>> }
impl SecretPref {
    pub async fn get(&self) -> String;            // "" when absent, like Kotlin default
    pub async fn set(&self, v: &str) -> Result<(), PrefError>;
    pub async fn delete(&self);
}
```

`TrackingPrefs::track_username(tracker_id) -> SecretPref` (and `track_password`, `track_token`) replace `TrackPreferences.trackUsername(tracker)`, `trackPassword(tracker)` and `trackToken(tracker)` `set_credentials(tracker, username, password)` writes username, then password, then clears the auth-expired flag in `state.toml` (same order as Kotlin; failures abort before the flag is cleared). `SecretStore` calls are blocking D-Bus; implementations run them inside `tokio::task::spawn_blocking`. When no Secret Service is running, `kuro-platform` falls back to an encrypted file (see T-req-platform-003) and the app shows its once-only notice.

Backup interplay: `PrefStore::all()` returns no private keys; `backup::export(include_private)` asks `Prefs` to read tracker secrets for every tracker id found in `state.toml`/registry only when `include_private` is true, producing entries named `__PRIVATE_pref_mangasync_username_<id>` etc.

## 7. Per-source preferences

```rust
pub struct SourcePrefs { id: i64, path: PathBuf, values: Arc<RwLock<IndexMap<String, PrefValue>>>, tx: broadcast::Sender<Changed> }
impl SourcePrefs {
    pub fn open(paths: &dyn Paths, source_id: i64) -> Result<SourcePrefs, PrefError>;   // sources/source_<id>.toml
    pub fn pref_key(id: i64) -> String { format!("source_{id}") }      // preferencesKey(id)
    pub fn get_bool(&self, key: &str, default: bool) -> bool;          // plus get_int, get_long, get_float, get_string, get_set with the same shape
    pub fn set(&self, key: &str, v: PrefValue);
    pub fn all(&self) -> Vec<(String, PrefValue)>;
    pub fn is_empty(&self) -> bool;
}
```

Same flush/atomic rules as `TomlBackend` (own small flusher, one file per source). Wrong-kind reads follow the `ClassCastException` rule (delete, default). Extensions are not Kotlin in Kurobayomi; how an extension declares settings is decided in area:20-extension-strategy-redesign, and the GTK widget factory renders its declaration via the same `bind::*` functions with a `SourcePrefs` accessor in place of `Pref<T>`.

## 8. Backup interop surface

```rust
pub struct PrefEntry { pub key: String, pub value: PrefValue }
pub struct SourcePrefEntries { pub source_key: String, pub prefs: Vec<PrefEntry> }   // source_key == "source_<id>"

impl Prefs {
    pub fn export_app(&self, include_private: bool) -> Vec<PrefEntry>;
    pub fn export_sources(&self, include_private: bool) -> Vec<SourcePrefEntries>;
    pub async fn import_app(&self, entries: Vec<PrefEntry>, cats: &CategoryRemap) -> ImportReport;
    pub fn import_sources(&self, entries: Vec<SourcePrefEntries>) -> ImportReport;
}
```

Algorithms are in `p1-topic-backup-interop.md`; protobuf (`prost`) message structs are generated by area:21-backup-format-and-create and converted to these neutral structs so `kuro-prefs` does not depend on `prost`.

## 9. How this plugs into the existing mockup

Existing code (OBSERVED): `src/spec.rs` loads `spec/settings.json`, `app.json`, `entries.json`, `reader_player.json` into `Spec` with `Item` variants (`Switch`, `List`, `Multi`, `Slider`, `Text`, `Tristate`, `Sort`, `Chips`, `Link`, `Action`, `Info`) that carry only title, subtitle, options and a default; `src/ui.rs::item_row` (line 221) builds a widget from each item using the default and discards state; `src/settings.rs::search` indexes titles. Nothing persists.

Changes:

1. `spec/SCHEMA.md` and `Item` gain two optional fields on `Switch`, `List`, `Multi`, `Slider`, `Text`, `Tristate`, `Sort`, `Chips`: `"pref": "<table.field>"` (for example `"library.display_mode"` or `"player.gestures.gesture_horizontal_seek"`; the path is the TOML table plus the Rust field name from the registry, never the Kotlin key string) and, for choice items, `"values": [..]` listing the stored value for each option label in order (enum names, integers, or strings). Parallel arrays are used because the labels are the real English strings from `strings.xml` and must not be re-keyed.
2. `spec.rs` `#[serde(default)] pref: Option<String>, values: Vec<Value>` on those variants, plus a test `every_pref_exists_in_registry` that resolves each `pref` through `REGISTRY` and checks `values.len() == options.len()` and that every value decodes with the registry type and that the item default equals the registry default (this converts the mockup's baked-in defaults into a checked invariant).
3. `ui::item_row`: after creating a widget for an item that has `pref`, call the matching `bind::*` function with `ctx().prefs.lookup(pref)` (a dynamic lookup by `table.field` returning an object-safe `AnyPref` wrapper over `Pref<T>`). Items without `pref` keep today's inert behaviour, so the mockup keeps working while pages are bound incrementally (task T-02a-030 lists the P0 pages).
4. `ui::ctx()` returns an `Rc<AppCtx>` stored in a `thread_local!` set by `main::build_ui`; `AppCtx` is the foundation area's DI container holding `Prefs`. This is the only global, GTK-thread-only, and matches the existing `with_spec` thread-local style.
5. Settings search (`settings.rs::search`) keeps indexing titles; add the pref's table name as a hidden second search term so "tapping" finds the tap-zone settings.
6. Theme/dark mode: `ui::apply_theme(prefs)` reads `ui.theme_mode` and calls `gtk::Settings::default().set_gtk_application_prefer_dark_theme(..)`; it is subscribed to `Pref::changes`. The addendum rule 6 (ship own Tokyonight CSS) and the task statement (vanilla GTK, user theme applies) are reconciled by `app_theme`: `DEFAULT` = no extra CSS (user GTK theme), every other value loads the CSS owned by area:49-theme-and-design-system.
7. `main.rs` startup order: build `Paths`, build `Prefs::open` (blocking, <20 ms for a normal file), apply theme and `ui.start_screen`, then construct widgets.

## 10. Android API to Linux mapping (APIs touched by this scope)

| Android / Kotlin | Used for | Linux / Rust |
|---|---|---|
| `SharedPreferences` + `PreferenceManager.getDefaultSharedPreferences` | main store | `PrefStore` over `config.toml`/`state.toml` |
| `SharedPreferences.OnSharedPreferenceChangeListener` + `callbackFlow` | change events | `tokio::sync::broadcast` + `glib::spawn_future_local` |
| `Flow`, `StateFlow`, `conflate`, `stateIn` | reactive reads | `Changes<T>` stream; widgets keep their own state |
| `Context.getSharedPreferences(name)` | per-source stores | `SourcePrefs` files |
| `androidx.preference.PreferenceDataStore`, `PreferenceFragmentCompat` | extension settings UI | GTK widget factory with `bind::*` (area:20-extension-strategy-redesign/47) |
| Injekt `addSingletonFactory` | singletons | `AppCtx` fields (area:01-foundation-app-bootstrap) |
| `kotlinx.coroutines` dispatchers | async | tokio runtime + GLib main context |
| `AppCompatDelegate.setDefaultNightMode` | theme mode | `gtk::Settings` + xdg-desktop-portal `org.freedesktop.appearance` |
| `DynamicColors` / `DeviceUtil.isDynamicColorAvailable` | default theme | constant `false` (default `DEFAULT`) |
| `LocaleHelper` / `Locale.getDefault().language` | default languages | `sys-locale` |
| `java.time.format.DateTimeFormatter` | date format | `chrono` + `datefmt` translator |
| `PackageManager.hasSystemFeature(PIP)` | PiP | constant false |
| `Environment.getExternalStorageDirectory` + `AndroidStorageFolderProvider` | default storage dir | `Paths::default_storage_dir()` |
| `BiometricPrompt` (`use_biometric_lock`) | app lock | passphrase dialog (area:36-ui-shell-navigation-security-webview) |
| `WindowManager.LayoutParams.FLAG_SECURE` (`secure_screen_v2`) | screenshot block | dropped |
| `androidx.work.Constraints` (network/charging/battery) | job constraints from prefs | GIO `NetworkMonitor` + UPower (area:26-library-update-jobs) |
| `kotlinx.serialization.protobuf` `@ProtoNumber` | backup | `prost` in area:21-backup-format-and-create |
| `androidx.compose.ui.graphics.Color.toArgb()` | subtitle colours | plain `i32` ARGB |
| `androidx.compose.ui.graphics.BlendMode` | colour filter | `gsk::BlendMode` |

## 11. Pitfalls

* Integer width: keep `i32` for `Int` keys and `i64` for `Long` keys. The three most error-prone: `pref_progress_preference` is `Float` (0.85), `default_*` flag templates are `Long`, subtitle colours are `Int` and negative (`-1` is white).
* `f32` in TOML: `toml_edit` stores `f64`; write `f32 as f64` shortest-round-trip (`f32::to_string` parsed to f64) so `0.85` stays `0.85` rather than `0.8500000238418579`.
* Set ordering is irrelevant to Kotlin (`Set<String>`), but TOML arrays are ordered: always write `BTreeSet` order so files are diff-stable.
* The three namespaces share one key string space; a key can never collide because the prefixes are unique, but `Prefs` must reject `app_state_key` constants being passed to `Normal` handles in debug builds (`debug_assert!`).
* `ExtensionInstallerPreference.get()` writes during a read; the Rust port has no equivalent (dropped) and must not copy the pattern (reads must be side-effect free except the documented wrong-type delete).
* Writer threads: `Pref::set` takes a write lock for microseconds; never call it while holding a GTK borrow that a change handler also borrows (handlers run later on the main loop, so re-entrancy is impossible by construction).
* Quit while a flush is pending loses at most 250 ms of changes unless `flush()` runs; the shutdown hook must run before the tokio runtime is dropped.
* `SingleInstance` forwarding means the second process must never open `PrefStore`; it only forwards arguments (area:01-foundation-app-bootstrap).
* Windows (later): `atomic-write-file` and `fd-lock` are the portable choices; do not use `rename` over an open file directly. Path building only through `Paths`.
