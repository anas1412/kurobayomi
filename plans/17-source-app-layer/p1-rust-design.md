# Area 17 — App source layer: Rust design

Scope: the 27 Kotlin files listed in `p1-inventory.md`. This document is the Rust design for the parts of Aniyomi that sit between the extension loader and the UI: the live-source registry, the stub registry, the enabled/pinned/language lists, the toggles, the incognito and data-saver flags, the migration list ordering, and the remote entry refresh.

Crate names follow the other areas: `kuro-domain` (pure types), `kuro-core` (traits shared across areas, including `SourceGate` owned here), `kuro-sources` (this area), `kurobayomi` (GTK 4 UI, `src/`).

## 1. Crate and module layout

```
kuro-domain/src/source/
  mod.rs            pub use of the items below
  pin.rs            PinFlags (2-bit set)
  model.rs          SourceEntry, StubSource, SourceKind
kuro-core/src/sources.rs
                    SourceGate (watch receiver of readiness), SourceId = i64
kuro-sources/src/
  lib.rs            pub use; SourceSystem (builds both registries)
  identity.rs       SourceIdentity trait: adapter over area 15/16 source traits
  registry.rs       SourceRegistry<K> (live map, stub map, readiness, rename hook)
  stubs.rs          StubStore trait + rusqlite implementation for `sources` and `animesources`
  naming.rs         visual_name, info-name rule, stub display string, key()
  prefs.rs          SourcePrefs: typed view over the settings store + key constants
  locale.rs         default enabled languages, localized display names, collator
  enabled.rs        enabled_sources() pure function, toggles (sources, pins, languages)
  languages.rs      group_by_language() + language comparator
  incognito.rs      global + per-extension incognito rule
  datasaver.rs      data-saver exclusion toggle and DataSaver enum
  migration.rs      SetMigrateSorting values, favourite-count sort
  icons.rs          extension icon lookup -> gdk::Texture (GTK only)
  torrent.rs        is_source_for_torrents
  remote_update.rs  UpdateAnimeFromRemote equivalent (episodes and seasons)
```

Pure modules (`pin`, `model`, `naming`, `enabled`, `languages` grouping, `migration` comparator, `incognito`, `datasaver`, `torrent`) have no I/O and are unit-testable with the vectors in `p1-edge-cases-and-tests.md`. I/O lives in `registry`, `stubs`, `prefs`, `remote_update`, `icons`.

## 2. Core types

### 2.1 `PinFlags` (`kuro-domain/src/source/pin.rs`)

```rust
/// Bit layout identical to Aniyomi's `Pin` objects (see data file 1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PinFlags(u8);

impl PinFlags {
    pub const PINNED_BIT: u8 = 0b01; // Pin.Pinned
    pub const ACTUAL_BIT: u8 = 0b10; // Pin.Actual
    pub const UNPINNED: Self = Self(0b00);
    pub const PINNED: Self = Self(0b11); // Pins.pinned

    pub const fn bits(self) -> u8 { self.0 }
    /// Kotlin `Pin in pins`: (bit and code) == bit. A zero bit is always contained.
    pub const fn contains(self, bit: u8) -> bool { self.0 & bit == bit }
    pub const fn with(self, bit: u8) -> Self { Self(self.0 | bit) }
    /// PROPOSAL: AND-NOT. Kotlin uses XOR (`Pins.minus`), which sets the bit on
    /// unpinned rows. Results differ only for unpinned last-used rows (0b10 vs 0b00),
    /// and no consumer reads that bit (behavior 4.1).
    pub const fn without(self, bit: u8) -> Self { Self(self.0 & !bit) }
    pub const fn is_pinned(self) -> bool { self.contains(Self::PINNED_BIT) }
    pub const fn is_actual(self) -> bool { self.contains(Self::ACTUAL_BIT) }
}
```

Test (must pass): `PINNED.without(ACTUAL_BIT).bits() == 0b01`; `UNPINNED.without(ACTUAL_BIT).bits() == 0b00`; `UNPINNED.contains(0)` is true.

### 2.2 `SourceEntry` (one type for both kinds, `kuro-domain/src/source/model.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    pub id: i64,
    pub lang: String,            // "" | "other" | "all" | BCP-47-ish code, as published by the extension
    pub name: String,
    pub supports_latest: bool,
    pub is_stub: bool,
    pub pin: PinFlags,
    pub is_used_last: bool,
    /// Manga only; always false for anime (the Kotlin anime type has no such field).
    pub data_saver_excluded: bool,
}

impl SourceEntry {
    /// Kotlin `Source.key()`: "<id>-lastused" for the duplicate row, else "<id>".
    pub fn key(&self) -> String {
        if self.is_used_last { format!("{}-lastused", self.id) } else { self.id.to_string() }
    }
    /// Kotlin `visualName`: name only when lang is empty, else "name (LANG)" with uppercase lang.
    pub fn visual_name(&self) -> String {
        if self.lang.is_empty() { self.name.clone() }
        else { format!("{} ({})", self.name, self.lang.to_uppercase()) }
    }
}
```

Note: `to_uppercase` on `str` in Rust is locale-independent (Unicode default mapping), matching Kotlin `uppercase()` (ROOT locale).

### 2.3 `StubSource` (`kuro-domain/src/source/model.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]   // PartialEq is the fix for the Kotlin reference-equality defect (behavior 1.5)
pub struct StubSource {
    pub id: i64,
    pub lang: String,
    pub name: String,
}

impl StubSource {
    /// Kotlin `isInvalid`: name or lang is blank (whitespace-only counts as blank).
    pub fn is_invalid(&self) -> bool {
        self.name.trim().is_empty() || self.lang.trim().is_empty()
    }
    /// Kotlin `toString()`: "Name (LANG)" or the decimal id when invalid.
    pub fn display(&self) -> String {
        if self.is_invalid() { self.id.to_string() }
        else { format!("{} ({})", self.name, self.lang.to_uppercase()) }
    }
    pub fn fallback(id: i64) -> Self { Self { id, lang: String::new(), name: String::new() } }
}
```

### 2.4 `SourceKind` (`kuro-domain/src/source/model.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind { Manga, Anime }

impl SourceKind {
    pub const fn stub_table(self) -> &'static str {
        match self { Self::Manga => "sources", Self::Anime => "animesources" }
    }
}
```

### 2.5 Errors (`kuro-sources/src/error.rs`, thiserror)

```rust
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source {0} is not installed")]
    NotInstalled(i64),            // Kotlin SourceNotInstalledException / AnimeSourceNotInstalledException
    #[error("source {0} is not a catalogue source")]
    NotCatalogue(i64),            // replaces the Kotlin `as CatalogueSource` cast (behavior 12)
    #[error("stub store: {0}")]
    Store(#[from] rusqlite::Error),
    #[error("cancelled")]
    Cancelled,                    // distinct from failure (behavior 12.1)
}
```

### 2.6 `SourceIdentity` (`kuro-sources/src/identity.rs`)

The registry needs only these facts about a live source. The concrete source trait belongs to area 15 (manga) and area 16 (anime); this layer adapts them.

```rust
pub trait SourceIdentity: Send + Sync + 'static {
    fn id(&self) -> i64;
    fn lang(&self) -> &str;
    fn name(&self) -> &str;
    fn supports_latest(&self) -> bool;
    /// Kotlin HttpSource / AnimeHttpSource
    fn is_http(&self) -> bool;
    /// Kotlin CatalogueSource (manga). Anime implementations return true for every live source.
    fn is_catalogue(&self) -> bool;
    /// Local source has id 0 (LocalMangaSource.ID, LocalAnimeSource.ID).
    fn is_local(&self) -> bool { self.id() == LOCAL_SOURCE_ID }
}

pub const LOCAL_SOURCE_ID: i64 = 0;
```

### 2.7 `ExtensionCatalog` (consumed, owned by area 18-20)

```rust
/// Snapshot published by the extension manager on every change (Kotlin installedExtensionsFlow).
pub struct InstalledSnapshot {
    pub extensions: Vec<InstalledExtension>,   // in load order
}
pub struct InstalledExtension {
    pub pkg_name: String,
    pub is_torrent: bool,                      // anime only (AnimeExtension.isTorrent)
    pub sources: Vec<Arc<dyn SourceIdentity>>, // live sources, in extension order
}

pub trait ExtensionCatalog: Send + Sync {
    fn installed(&self) -> tokio::sync::watch::Receiver<Arc<InstalledSnapshot>>;
    /// Kotlin getSourceData(id): stub data from the AVAILABLE index (may be empty).
    fn index_source(&self, id: i64) -> Option<StubSource>;
    fn package_for_source(&self, id: i64) -> Option<String>;   // getExtensionPackage
    fn icon_path_for_package(&self, pkg: &str) -> Option<std::path::PathBuf>;
}
```

Area 18-20 must implement this trait. If that area names its types differently, keep this shape; it is the contract for this area.

## 3. Registry

### 3.1 `SourceRegistry<K>` (`kuro-sources/src/registry.rs`)

```rust
pub struct SourceRegistry {
    kind: SourceKind,
    live: std::sync::RwLock<Arc<LiveMap>>,         // copy-on-write; readers clone the Arc
    stubs: std::sync::RwLock<HashMap<i64, StubSource>>, // in-memory copy of the table + fallbacks
    store: Arc<dyn StubStore>,
    catalog: Arc<dyn ExtensionCatalog>,
    renamer: std::sync::OnceLock<Arc<dyn SourceRenamer>>, // filled after the download manager exists
    register_lock: tokio::sync::Mutex<()>,         // serialises read-compare-write-rename (behavior 1.5)
    ready: tokio::sync::watch::Sender<bool>,
    generation: tokio::sync::watch::Sender<u64>,   // bumps on any visible change; UI re-reads
}

pub struct LiveMap {
    pub by_id: HashMap<i64, Arc<dyn SourceIdentity>>, // includes the local source (id 0)
}

pub trait SourceRenamer: Send + Sync {
    fn rename_source(&self, old: &StubSource, new: &StubSource);   // download folder rename (area 23-25)
}
```

Public API (all synchronous, all memory-only, safe on the GTK thread):

```rust
impl SourceRegistry {
    pub fn is_ready(&self) -> bool;
    pub fn ready_watch(&self) -> tokio::sync::watch::Receiver<bool>;     // SourceGate (section 6)
    pub fn generation_watch(&self) -> tokio::sync::watch::Receiver<u64>;

    pub fn get(&self, id: i64) -> Option<Arc<dyn SourceIdentity>>;                // Kotlin get
    pub fn get_or_stub(&self, id: i64) -> ResolvedSource;                         // Kotlin getOrStub
    pub fn online_sources(&self) -> Vec<Arc<dyn SourceIdentity>>;                 // is_http
    pub fn catalogue_sources(&self) -> Vec<Arc<dyn SourceIdentity>>;              // is_catalogue (manga)
    pub fn all_live(&self) -> Vec<Arc<dyn SourceIdentity>>;                       // anime getAll
    pub fn stub_sources(&self) -> Vec<StubSource>;                                // getStubSources (fixed, 1.3)

    /// Requires a catalogue source; returns Err(NotCatalogue) instead of a cast crash.
    pub fn catalogue(&self, id: i64) -> Result<Arc<dyn SourceIdentity>, SourceError>;
}

pub enum ResolvedSource {
    Live(Arc<dyn SourceIdentity>),
    Stub(StubSource),          // Kotlin StubXSource (persisted or fallback)
}
```

Notes:
- `get_or_stub` returns `Stub(StubSource::fallback(id))` when nothing is known. The fallback is cached in memory only, as Kotlin does (`stubSourcesMap.getOrPut`), so a later index load can upgrade it: `on_index_loaded()` (3.4) replaces cached entries whose `lang` and `name` are both empty.
- Trait objects (`Arc<dyn SourceIdentity>`) are used because two kinds of source coexist in one map (extensions plus local). Keep the trait object: the number of implementations is small and fixed by area 15/16.

### 3.2 Extension-change loop (replaces `collectLatest` in init block A)

```rust
// Spawned once by SourceSystem::start(). Lives for the process.
async fn live_loop(reg: Arc<SourceRegistry>, mut rx: watch::Receiver<Arc<InstalledSnapshot>>) {
    loop {
        let snap = rx.borrow_and_update().clone();
        // Build the new map: local source first, then every extension source; a later
        // duplicate id replaces an earlier one (same as Kotlin `mutableMap[it.id] = it`).
        let mut by_id = HashMap::new();
        by_id.insert(LOCAL_SOURCE_ID, reg.local_source());
        for ext in &snap.extensions {
            for s in &ext.sources {
                by_id.insert(s.id(), s.clone());
            }
        }
        *reg.live.write().unwrap() = Arc::new(LiveMap { by_id });
        // Detached registration per source. NOT aborted by the next snapshot (Kotlin launches on `scope`).
        for ext in &snap.extensions {
            for s in &ext.sources {
                let r = reg.clone();
                let stub = StubSource { id: s.id(), lang: s.lang().into(), name: s.name().into() };
                tokio::spawn(async move { r.register_stub(stub).await });
            }
        }
        reg.ready.send_replace(true);
        reg.bump();
        if rx.changed().await.is_err() { break; }
    }
}
```

Proposal difference: the loop does not abort a registration on the next snapshot, and the map build is cheap, so Kotlin `collectLatest` cancellation has no visible effect and is not needed. Errors from `register_stub` are logged and do NOT panic the loop (behavior 1.5 cancels the Kotlin scope; here each task is isolated).

Local source construction: `reg.local_source()` is provided by area 14 (`LocalSource` is constructed from the storage paths and cover managers). Its identity: id 0, lang `"other"`, name from the localized string key.

### 3.3 `register_stub` (replaces `registerStubSource`)

```rust
impl SourceRegistry {
    async fn register_stub(self: Arc<Self>, incoming: StubSource) {
        let _guard = self.register_lock.lock().await;           // serialises concurrent registrations
        let stored = self.store.find(incoming.id).await;        // Result<Option<StubSource>, SourceError>
        let stored = match stored { Ok(v) => v, Err(e) => { tracing::error!(?e); return; } };
        if stored.as_ref() == Some(&incoming) { self.cache_stub(incoming); return; } // equality now works
        if let Err(e) = self.store.upsert(&incoming).await { tracing::error!(?e); return; }
        if let (Some(old), Some(r)) = (stored.as_ref(), self.renamer.get()) {
            r.rename_source(old, &incoming);                    // download folder follows the rename
        }
        self.cache_stub(incoming);
        self.bump();
    }
}
```

The `store.upsert` / `store.find` calls run on the blocking pool (section 4). The rename call is also blocking file I/O; `rename_source` is called from `spawn_blocking` as well.

### 3.4 Stub resolution (replaces `createStubSource`)

```rust
impl SourceRegistry {
    fn resolve_fallback(&self, id: i64) -> StubSource {
        // (a) stored row: already in memory (loaded at start, see 3.5)
        if let Some(s) = self.stubs.read().unwrap().get(&id) { return s.clone(); }
        // (b) index data for an uninstalled source; persisted through register_stub
        if let Some(s) = self.catalog.index_source(id) {
            tokio::spawn({ let r = self.clone(); let s = s.clone(); async move { r.register_stub(s).await } });
            return s;
        }
        // (c) fabricated, not persisted, cached in memory only
        StubSource::fallback(id)
    }

    /// Called when the extension index finishes loading: upgrade fallbacks that now have data.
    pub fn on_index_loaded(&self) { /* for each cached entry with empty lang and name, try (b) */ }
}
```

Difference from Kotlin (documented): `createStubSource` ran `runBlocking` DB reads; here (a) is a memory read because the table is loaded at start (3.5), so `get_or_stub` never blocks.

### 3.5 Start-up load of stub rows

```rust
impl SourceRegistry {
    pub async fn load_stubs(&self) -> Result<(), SourceError> {
        let rows = self.store.all().await?;                    // SELECT * FROM sources
        let mut map = self.stubs.write().unwrap();
        map.clear();
        for r in rows { map.insert(r.id, r); }
        self.bump();
        Ok(())
    }
}
```

Called at start, and again after every DB change to the stub table (the store exposes a change signal: `watch::Receiver<u64>` incremented after each `upsert`, which is the only writer in this app).

### 3.6 `SourceSystem` (`kuro-sources/src/lib.rs`)

```rust
pub struct SourceSystem {
    pub manga: Arc<SourceRegistry>,   // SourceKind::Manga
    pub anime: Arc<SourceRegistry>,   // SourceKind::Anime
}
impl SourceSystem {
    pub fn new(
        manga_store: Arc<dyn StubStore>, anime_store: Arc<dyn StubStore>,
        catalog: Arc<dyn ExtensionCatalog>,
    ) -> Self { /* two SourceRegistry::new(...) */ }
    /// Spawns live_loop and load_stubs for both kinds on the tokio runtime.
    pub fn start(self: &Arc<Self>, rt: &tokio::runtime::Handle) { /* ... */ }
    /// Attach the download renamer after the download manager is built (breaks the cycle, section 7).
    pub fn set_renamer(&self, r: Arc<dyn SourceRenamer>) { /* OnceLock::set, both kinds */ }
}
```

Start-up order (replaces the Kotlin warm-up at `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:242-251`): open both stores → `start()` → the registries become ready when the first extension snapshot is published. The UI does not wait for the stub load; the stub table is read before the first snapshot in `start()`.

## 4. Stub store (`kuro-sources/src/stubs.rs`)

```rust
#[async_trait::async_trait]   // or return boxed futures; the crate already uses async-trait if area 15 does
pub trait StubStore: Send + Sync {
    async fn all(&self) -> Result<Vec<StubSource>, SourceError>;
    async fn find(&self, id: i64) -> Result<Option<StubSource>, SourceError>;
    async fn upsert(&self, s: &StubSource) -> Result<(), SourceError>;
    fn changes(&self) -> tokio::sync::watch::Receiver<u64>;
}

pub struct SqliteStubStore {
    pool: Arc<DbPool>,               // the area 03/04 pool for the manga or anime database
    table: &'static str,             // "sources" | "animesources" (SourceKind::stub_table)
    changes: tokio::sync::watch::Sender<u64>,
}
```

SQL (exact, mirrors the `.sq` files):
```sql
SELECT _id, lang, name FROM sources;                       -- all
SELECT _id, lang, name FROM sources WHERE _id = ?1;        -- find
INSERT INTO sources(_id, lang, name) VALUES (?1, ?2, ?3)
ON CONFLICT(_id) DO UPDATE SET lang = excluded.lang, name = excluded.name
WHERE _id = ?1;                                            -- upsert
```

Use `format!` only for the table name (from the `&'static str`), never for values. `excluded.lang` is equivalent to the Kotlin `:lang` parameter.

Blocking: every method runs `pool.get()` + `spawn_blocking`. Errors map to `SourceError::Store`.

## 5. Pure functions

### 5.1 Enabled sources (`kuro-sources/src/enabled.rs`)

```rust
pub struct SourceSettings {
    pub enabled_languages: BTreeSet<String>,   // source_languages
    pub disabled: BTreeSet<i64>,               // hidden_catalogues / hidden_anime_catalogues (decimal in storage)
    pub pinned: BTreeSet<i64>,                 // pinned_catalogues / pinned_anime_catalogues
    pub data_saver_excluded: BTreeSet<i64>,    // data_saver_excluded (manga only)
    pub last_used: i64,                        // -1 = none
}

pub fn enabled_sources(all: &[SourceEntry], s: &SourceSettings) -> Vec<SourceEntry> {
    let mut rows: Vec<&SourceEntry> = all.iter()
        .filter(|x| s.enabled_languages.contains(&x.lang) || x.id == LOCAL_SOURCE_ID)
        .filter(|x| !s.disabled.contains(&x.id))
        .collect();
    // Kotlin: sortedWith(compareBy(CASE_INSENSITIVE_ORDER) { name }). Stable sort.
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    let mut out = Vec::with_capacity(rows.len() * 2);
    for x in rows {
        let mut e = x.clone();
        e.pin = if s.pinned.contains(&x.id) { PinFlags::PINNED } else { PinFlags::UNPINNED };
        e.data_saver_excluded = s.data_saver_excluded.contains(&x.id);
        out.push(e.clone());
        if x.id == s.last_used {
            let mut dup = e;
            dup.is_used_last = true;
            dup.pin = dup.pin.without(PinFlags::ACTUAL_BIT);   // Proposal (XOR in Kotlin)
            out.push(dup);
        }
    }
    out
}
```

Notes:
- `to_lowercase` is an approximation of Java `CASE_INSENSITIVE_ORDER` (which compares per `char` with `toUpperCase` then `toLowerCase`). They differ for a few special characters (e.g., German sharp s). Accepted difference; list in edge cases.
- The `enabled_languages.contains(&x.lang)` test is an exact string match, as in Kotlin.

Toggles (pure, operate on the `BTreeSet`s, persisted by `prefs.rs`):

```rust
pub fn toggle_pin(set: &mut BTreeSet<i64>, id: i64) -> bool { if !set.remove(&id) { set.insert(id) } else { false } }
pub fn set_disabled(set: &mut BTreeSet<i64>, ids: &[i64], disable: bool) {
    for id in ids { if disable { set.insert(*id); } else { set.remove(id); } }
}
pub fn toggle_language(set: &mut BTreeSet<String>, lang: &str) { if !set.remove(lang) { set.insert(lang.to_string()); } }
pub fn toggle_data_saver_excluded(set: &mut BTreeSet<i64>, id: i64) { if !set.remove(&id) { set.insert(id); } }
pub fn set_incognito_pkg(set: &mut BTreeSet<String>, pkg: &str, enable: bool) { /* insert / remove */ }
```

Single-source flip used by the UI: `enable = set.contains(&id)` (currently disabled → enable), i.e., `set_disabled(set, &[id], !currently_disabled)`. Name the helper `flip_disabled`; do not reproduce Kotlin's misleading `isEnabled` name.

Persistence of a toggle: read, change, write under one `prefs` lock so two quick toggles cannot lose one update (Kotlin `getAndSet` is not atomic across the read; behavior 6.2 and 6.3).

### 5.2 Language grouping (`kuro-sources/src/languages.rs`)

```rust
pub struct LanguageGroup { pub lang: String, pub sources: Vec<SourceEntry>, pub enabled: bool }

pub fn group_by_language(online: &[SourceEntry], s: &SourceSettings, cmp: &dyn Fn(&str, &str) -> Ordering)
    -> Vec<LanguageGroup>
{
    let mut sorted: Vec<&SourceEntry> = online.iter().collect();
    sorted.sort_by(|a, b| {
        let da = s.disabled.contains(&a.id);
        let db = s.disabled.contains(&b.id);
        da.cmp(&db).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let mut groups: Vec<LanguageGroup> = Vec::new();
    for x in sorted {
        if let Some(i) = groups.iter().position(|g| g.lang == x.lang) {
            groups[i].sources.push(x.clone());
        } else {
            groups.push(LanguageGroup {
                lang: x.lang.clone(),
                sources: vec![x.clone()],
                enabled: s.enabled_languages.contains(&x.lang),
            });
        }
    }
    // enabled first; then comparator (all first, then localized display name); then raw code (PROPOSAL tie-break)
    groups.sort_by(|a, b| (!a.enabled).cmp(&!b.enabled).then_with(|| cmp(&a.lang, &b.lang)).then_with(|| a.lang.cmp(&b.lang)));
    groups
}
```

`groups.iter().position` is O(n²) on the number of languages (typically < 50); acceptable. Use `BTreeMap` if this ever matters.

Language comparator (`locale.rs`): `fn language_cmp(a, b) -> Ordering` = `"all"` first; else compare `display_name(a)` with `display_name(b)` (see 6.2).

### 5.3 Name rules (`kuro-sources/src/naming.rs`)

```rust
pub fn name_for_info(src: &SourceEntry, enabled: &BTreeSet<String>) -> String {
    let active: Vec<&String> = enabled.iter().filter(|l| l.as_str() != "all" && l.as_str() != "other").collect();
    let one = active.len() == 1;
    let in_enabled = enabled.contains(&src.lang);
    if one && in_enabled { src.name.clone() } else { src.visual_name_with_stub_rule() }
}
```

Where `visual_name_with_stub_rule()` is `StubSource::display()` for stubs and `visual_name()` for live. Careful: Kotlin's `toString()` for a LIVE source is `"$name (LANG)"` (the extension's own `toString`) and the rule above returns it for both. The Rust version must use the same `"Name (LANG)"` string for live sources (no id fallback) to match. Test vector: name "Foo", lang "en", enabled {"en","all"} → "Foo".

### 5.4 Migration sort (`kuro-sources/src/migration.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationMode { Alphabetical, Total }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationDirection { Ascending, Descending }

pub fn sort_migration_sources(rows: &mut Vec<(SourceEntry, u64)>, mode: MigrationMode, dir: MigrationDirection, collator: &Collator) {
    rows.retain(|(s, _)| s.id != LOCAL_SOURCE_ID);
    let base = |a: &(SourceEntry, u64), b: &(SourceEntry, u64)| -> Ordering {
        match (a.0.is_stub, b.0.is_stub) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
        match mode {
            MigrationMode::Alphabetical => collator.compare(&a.0.name.to_lowercase(), &b.0.name.to_lowercase()),
            MigrationMode::Total => a.1.cmp(&b.1),
        }
    };
    // stable sort; Descending = reverse of the whole comparator (stubs go last), as Kotlin reverseOrder
    match dir {
        MigrationDirection::Ascending => rows.sort_by(base),
        MigrationDirection::Descending => rows.sort_by(|a, b| base(b, a)),
    }
}
```

Notes: `rows.sort_by` is stable (merge sort), matching Kotlin TimSort stability. `base(b, a)` is exactly `Collections.reverseOrder` (`Comparator.reverseOrder` of a comparator = swap arguments). Stub-first and total-ascending rules are reproduced as written.

## 6. Locale services (`kuro-sources/src/locale.rs`)

### 6.1 Default enabled languages

```rust
pub fn default_enabled_languages(env_locale: Option<&str>) -> BTreeSet<String> {
    // env_locale = first non-empty of LC_ALL, LC_MESSAGES, LANG
    let lang = env_locale
        .map(|s| s.split(['.', '@']).next().unwrap_or(""))
        .map(|s| s.split(['_', '-']).next().unwrap_or(""))
        .filter(|s| !s.is_empty() && *s != "C" && *s != "POSIX")
        .unwrap_or("en");
    ["all", "en", lang].into_iter().map(String::from).collect()
}
```

Behaviour: `ja_JP.UTF-8` → `{all, en, ja}`; `C` or unset → `{all, en}`. Store as the default only while `source_languages` is unset.

### 6.2 Localized display names and collator

Requirement from Kotlin: `getLocalizedDisplayName(lang)` in the UI language, first letter uppercased; `zh-CN` → `zh-Hans`, `zh-TW` → `zh-Hant`; `""` → system default language.

Proposal: `icu` (ICU4X) crate with `compiled_data`:
- Display names: `icu_displaynames` `LocaleDisplayNamesFormatter` in the UI locale (the UI locale is the same env-derived tag). Verify the exact constructor against the pinned version at implementation time. Fallback when a tag is not recognised: return the raw code (Kotlin would print the BCP tag too for unknown tags).
- Collator: `icu::collator::Collator` with primary strength (Kotlin `Collator.PRIMARY`), locale from the env. Used by the ALPHABETICAL migration sort.

Why ICU and not std: std has no locale-aware collation or display names. Cost: compiled ICU data adds binary size (acceptable for a desktop app; say so in the release notes). Alternative considered and rejected: a hand-written table of ~200 language names (rejected: wrong for many languages and no collation).

## 7. Breaking the cycle with the download manager

Kotlin resolves `MangaDownloadManager` lazily (`injectLazy`), because the download manager depends on the source managers. Rust: `SourceSystem::set_renamer` (3.6) fills a `OnceLock<Arc<dyn SourceRenamer>>` after both objects exist. Until it is set, renames are skipped and logged at `warn`; this can only happen during start-up, before any stub row changed. The `kuro-sources` crate never depends on the download crate; the download crate implements `SourceRenamer`.

## 8. Incognito and data-saver (`incognito.rs`, `datasaver.rs`)

```rust
pub fn is_incognito(global: bool, source_pkg: Option<&str>, set: &BTreeSet<String>) -> bool {
    global || source_pkg.map_or(false, |p| set.contains(p))
}
pub enum DataSaver { None, BandwidthHero, WsrvNl, ResmushIt }   // stored by name, default None
```

`source_pkg` comes from `ExtensionCatalog::package_for_source(id)` (returns None for local and stub sources, as Kotlin). The `subscribe` variant is replaced by re-evaluating on the settings `watch`.

## 9. Remote entry update (`remote_update.rs`)

Interface (replaces `UpdateAnimeFromRemote`):

```rust
pub struct FetchFlags { pub fetch_details: bool, pub fetch_episodes: bool, pub manual_fetch: bool, pub fetch_window: (i64, i64) }

pub struct RemoteEpisodeUpdate { pub anime: Anime, pub new_episodes: Vec<Episode> }   // RemoteAnimeEpisodeUpdate
pub struct RemoteSeasonUpdate  { pub anime: Anime, pub new_seasons: Vec<Anime> }      // RemoteAnimeSeasonUpdate

#[async_trait::async_trait]
pub trait RemoteUpdater: Send + Sync {
    async fn episodes(&self, anime: &Anime, src: &dyn AnimeSourceLike, f: FetchFlags)
        -> Result<RemoteEpisodeUpdate, RemoteError>;
    async fn seasons(&self, anime: &Anime, src: &dyn AnimeSourceLike, f: FetchFlags)
        -> Result<RemoteSeasonUpdate, RemoteError>;
}

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error(transparent)] Source(#[from] SourceError),     // NotInstalled for stub
    #[error(transparent)] Db(#[from] rusqlite::Error),
    #[error("cancelled")] Cancelled,
    #[error("source failed: {0}")] Fetch(String),
}
```

- `episodes` reads episodes ordered by the source-order column ascending (Kotlin `sortedBy { it.sourceOrder }`; the SQL column name is owned by area 07, verify in its `.sq`), maps to the source model, calls the source's `getAnimeEpisodeUpdate`, applies the metadata rules (`apply_remote_metadata`), then calls the episode sync (area 07), then re-reads the anime row.
- `seasons` reads seasons ordered by `seasonSourceOrder` ascending (Kotlin `sortedBy { it.anime.seasonSourceOrder }`; SQL column owned by area 04, verify).
- The rules for covers, backgrounds, titles, and the coalesce semantics are in `p1-topic-remote-entry-update.md` and are implemented as pure functions:

```rust
pub fn cover_last_modified(local: &Anime, remote_thumb: Option<&str>, manual: bool, has_custom: bool, now_ms: i64) -> CacheAction;
pub fn background_last_modified(local: &Anime, remote_bg: Option<&str>, manual: bool, has_custom: bool, now_ms: i64) -> CacheAction;
pub enum CacheAction { Keep, Set(i64), DeleteCacheThenKeep, DeleteCacheThenSet(i64) }   // see topic file
pub fn title_update(local: &Anime, remote_title: Option<&str>) -> Option<String>;      // only when !favorite
```

`apply_remote_metadata` builds an `AnimeUpdate` (Rust struct mirroring the Kotlin fields, with `Option` = keep) and writes it through `AnimeRepo::update` (area 04/05). The `bool` returned by the repository is checked: `false` returns `RemoteError::Db`. (Kotlin discards it, which hides write failures; this is a deliberate fix.)

Cancellation: the calling task is cancelled by dropping the future; the function must check `tokio::task::yield_now()`-style cancellation only at await points. Do not catch `Cancelled` as `Fetch`.

## 10. Async and threading model

| Concern | Choice |
|---|---|
| Runtime | one multi-threaded tokio runtime, started by area 01, handle passed into `SourceSystem::start` |
| UI thread | GTK main loop; never calls a blocking function in this crate |
| Reads for UI | synchronous, memory only (`get`, `get_or_stub`, `enabled_sources` input snapshot) |
| Change notification | `watch::Receiver<u64>` (generation). UI loop: `glib::spawn_future_local(async move { while rx.changed().await.is_ok() { recompute_and_redraw(); } })` |
| Settings changes | settings store publishes a `watch` per key set (area 02); `enabled_sources` is recomputed on any change, and the result is compared with `PartialEq` before publishing (Kotlin `distinctUntilChanged`) |
| DB | `spawn_blocking` around rusqlite (pool from area 03/04) |
| File I/O (renames, icons) | `spawn_blocking` |
| Registration serialisation | `register_lock: tokio::sync::Mutex<()>` per registry |
| Cancellation | by dropping futures; the registration tasks are detached and are not cancelled by a newer snapshot |

Never call `block_on` from the GTK thread (Kotlin's `runBlocking` in `getOrStub` is the anti-pattern this design removes).

## 11. Android API → Linux mapping (this area only)

| Android/Kotlin | Linux/Rust replacement |
|---|---|
| `Context` parameter (`AndroidMangaSourceManager(context, ...)`) | constructor parameters: `Paths` from area 01 (XDG dirs) |
| `Injekt.get<T>()` / `injectLazy()` | constructor injection on `SourceSystem`; the cycle with downloads uses `OnceLock` (section 7) |
| `MutableStateFlow` / `StateFlow` | `tokio::sync::watch` |
| `ConcurrentHashMap` | `RwLock<Arc<HashMap>>` (copy-on-write for the live map) |
| `CoroutineScope(Job() + Dispatchers.IO)` | tokio tasks + `spawn_blocking` |
| `runBlocking` | removed (memory map loaded at start) |
| `Flow.combine(...).distinctUntilChanged()` | a recompute function + `watch::send_if_modified` |
| `collectLatest` | "run latest snapshot"; detached per-source tasks |
| `Drawable` (`getAppIconForSource`) | `gdk::Texture` from the extension package icon file (PNG/WebP via gdk-pixbuf loaders) |
| Compose `ImageBitmap` | dropped (no equivalent needed; GTK consumes `gdk::Texture`) |
| `LocaleListCompat`, `Locale.getDefault()` | environment parse (`LC_ALL`, `LC_MESSAGES`, `LANG`) |
| `Locale.getDisplayName` | `icu_displaynames` |
| `java.text.Collator` (PRIMARY) | `icu::collator` with primary strength |
| `String.lowercase()`, `uppercase()` | `str::to_lowercase`, `str::to_uppercase` |
| `compareToWithCollator` | ICU collator (section 6.2) |
| SQLDelight generated queries | rusqlite prepared statements (text in section 4) |
| Enum storage by name | same: `enum.name()` strings in the settings store |
| `logcat(LogPriority.ERROR, e)` | `tracing::error!` |

## 12. Mockup integration (`/home/blackbox/kurobayomi/src/`)

Current state (OBSERVED in the mockup):
- `src/entry.rs:283-300` `sources(anime)`: hard-coded sections `"Last used"` (`Source A`), `"Pinned"` (`Source B`), `"English"` (`Source A`, `Source C`, `Source D`), `"Other"` (local). Rows are placeholders; no state comes from a registry.
- `src/entry.rs:360-372` `migrate_sources`: hard-coded `Source A` (12), `Source B` (4), `Source C` (1) counts.
- `src/entry.rs:444` browse-tab list: hard-coded `["Source A", "Source B", "Source C"]`.
- `src/screens.rs:359`: hard-coded `[("Source A", 3), ("Source B", 2)]`.
- `src/settings.rs:229`: `"Source {i}"` rows with fake counts.

Plan:
1. `kurobayomi` depends on `kuro-sources`. The Browse tab (`src/entry.rs` `sources`) takes `enabled_sources(&registry.all_live_entries(), &prefs.snapshot())` and renders sections in the order of the Kotlin `MangaSourcesScreenModel.collectLatestSources` (`:71-107`): `LAST_USED` first, then `PINNED`, then language groups (alphabetical by key, `""` last), with the local source under its `other` group.
2. Sections are built with `group_by_language` for the Extensions/Language screens (`languages.rs`).
3. Migration list (`src/entry.rs:360`) takes `sort_migration_sources` output and shows the favourite count per row.
4. The `Pinned` icon uses `PinFlags::is_pinned()`; the pin badge is shown only on `is_actual()` rows in the pinned section.
5. Loading state: `ready_watch()` false → `gtk::Spinner` page (`LoadingScreen` equivalent), as in Kotlin `ifMangaSourcesLoaded()`.
6. Verify `spec/settings.json` source-related entries against the key table in `p1-data-and-config.md` section 1 (the mockup spec was extracted from the Kotlin; key names must match).

## 13. Crates (with reasons)

| Crate | Use | Why |
|---|---|---|
| `tokio` (sync, rt, macros) | watch, Mutex, spawn, spawn_blocking | required by the plan |
| `rusqlite` | stub store | required by the plan |
| `thiserror` | error enums | standard |
| `tracing` | error logs | standard; replaces logcat |
| `async-trait` | `StubStore`, `RemoteUpdater` | only if the rest of the workspace already uses it; otherwise write `Pin<Box<dyn Future + Send>>` returns (no dependency) |
| `icu` (ICU4X: collator, displaynames) | locale display names and collation | std has neither; see 6.2 |
| `gtk4`/`gdk4` (through the UI crate) | `gdk::Texture` for icons | required by the plan |
| `serde` | settings (area 02) | required by the plan |

No new crate for: sets (`std::collections::BTreeSet`), locks (`std::sync::RwLock`), locale parsing (env parse), sorting (`slice::sort_by`).

## 14. Pitfalls (must be handled in review)

1. Reference equality on stubs (Kotlin): make `StubSource` `PartialEq` (2.3), or every snapshot rewrites the table.
2. `XOR` pins (Kotlin): use `without` (2.1).
3. A `Result` that hides write failure (Kotlin `updateAnime` boolean): check it (section 9).
4. A `Cancelled` error that looks like a failure (Kotlin `catch (e: Exception)`): distinct variant (2.5).
5. Stub map never filled from DB (Kotlin quirk): load at start (3.5).
6. `as CatalogueSource` on a stub id → `NotCatalogue` error (2.5), never a panic.
7. `!!` on the owning extension (torrent helper): return `false` (section 8 and topic note in behavior 2.7).
8. Language group key collisions when two codes have the same display name: tie-break on the raw code (5.2).
9. `runBlocking` on the GTK thread: forbidden (section 10).
10. Settings read-modify-write races on toggles: one lock per set (section 5.1).
11. The local source appearing in online lists: `online_sources()` filters `is_http()`, and the local source is not HTTP.
12. Download folder rename during first start: `renamer` is `None` until `set_renamer` runs; rename is skipped and logged, which is safe because no stub row changed yet.

## 15. Open decisions for the author of area 17 (resolved here, listed for review)

- Decided: one `SourceEntry` type for both kinds; the anime flag `data_saver_excluded` is always false.
- Decided: stubs are loaded into memory at start; no DB call from UI code.
- Decided: `LocaleHelper.comparator` gains a raw-code tie-break (a deliberate fix).
- Open for area 02: whether `__APP_STATE_*` keys are exported in backups (affects `last_used` and `incognito_mode` only).
- Open for area 14: the `LocalSource` constructor signature used by `SourceSystem` (the registry asks for `local_source()`).
