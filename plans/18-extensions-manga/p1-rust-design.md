# Area 18 — Manga extensions: Rust design (PROPOSAL)

Everything here is a proposal unless it cites OBSERVED Kotlin. Behaviour references point to `p1-behavior.md` (section numbers). Data references point to `p1-data-and-config.md`. Task ids point to `p1-tasks.md`.

Target workspace: the existing `kurobayomi` package (`Cargo.toml`: `gtk4 0.11.5` with feature `v4_16`, `serde 1.0.229`, `serde_json 1.0.151`, edition 2024). This area adds two library crates and keeps the GTK app as the only binary.

---

## 1. Principles and lazier alternative

- The Linux app cannot run Android APKs. Porting the loader means replacing the runtime (section 2), not porting the code.
- Port the **rules** exactly: version gate, NSFW gate, language derivation, trust decision, downgrade and signer-continuity checks, install queue, status reconciliation, browse sort order and split rows. These are user-visible.
- Do not port the **mechanisms**: PackageManager, DEX class loading, DownloadManager, broadcasts, foreground services, Activities.
- Lazier alternative (named as required by the ponytail rule): ship only built-in Rust sources (no third-party loader, no repos, no trust, no installer). Cost: the user loses every third-party source and the install, update and trust features in this area. Taken: the full loader and installer, because the user asked for an app that replaces Aniyomi for daily use and this area is the only way third-party sources arrive. The built-in-only fallback stays available by not configuring any repo (the code path is the same).

---

## 2. Loading mechanism decision: WebAssembly component (Decided)

### 2.1 Options considered
| Option | Keeps existing Aniyomi APKs | Sandboxed | Cost | Decision |
|---|---|---|---|---|
| Run APK/DEX through an Android runtime (ART, Anbox-like) | yes | depends | very large; not a Rust crate; out of scope | Rejected |
| Native `.so` plugins loaded with `libloading` | no (needs rebuilt code) | no (in-process, full access) | ABI fragility across Rust versions; unsafe | Rejected |
| Subprocess per source speaking JSON over stdio | no | yes (OS process) | process per source, slow startup, IPC schema drift | Rejected |
| WASM component (`wasmtime`) with a host-provided HTTP bridge | no | yes (memory-safe, no ambient network or FS) | must write a guest SDK; source authors compile to `wasm32-wasip2` | **Chosen** |
| Built-in sources only | no | n/a | no third-party sources | Lazier fallback (section 1) |

Consequence stated plainly for the user: no existing Aniyomi APK extension works. Kurobayomi needs its own packages (section 2.3). Index listing of APK entries stays (behaviour 16.2) so the user can see what exists.

### 2.2 Host and guest contract (ABI 1.x)
The guest exports one function; the host provides HTTP and preference access. Payloads are JSON with the same field names as the `kuro-source` serde models of area:15-source-api-manga.

WIT world (file `kuro-source-wasm/wit/kuro-source.wit`):
```wit
package kuro:source@1.4.0;

interface host {
    /// Performs an HTTP request on behalf of the guest. Allowed only for hosts in the manifest allow-list.
    http: func(request-json: string) -> result<string, string>;
    /// Reads a source preference by key. Returns "" when unset.
    pref-get: func(key: string) -> string;
    /// Writes a source preference.
    pref-set: func(key: string, value-json: string) -> result<_, string>;
    /// Logs a message at level 0=debug, 1=info, 2=warn, 3=error.
    log: func(level: u8, message: string);
}

world source {
    import host;
    /// op is one of: "info", "create-sources", "popular", "search", "latest", "details", "chapters", "pages", "resolve-manga-uri", "resolve-chapter-uri", "prefs-schema".
    export invoke: func(op: string, input-json: string) -> result<string, string>;
}
```

Rules:
- `op` names are exact strings. Unknown op → guest returns `Err("unknown op: <op>")`.
- Input and output JSON are the serde forms of area:15-source-api-manga models (`SManga`, `SChapter`, `Page`, `MangasPage`, `FilterList`). Field names are `camelCase` where area:15-source-api-manga says so.
- Errors map to `SourceError` (area:15-source-api-manga) with the guest string as message.
- `http` request JSON: `{"method":"GET","url":"...","headers":[["k","v"]],"body":null}`; response JSON: `{"status":200,"headers":[["k","v"]],"body":"<utf-8 or base64>"}`. The host applies area:12-network-stack's client (cookies, user agent, timeouts). Non-UTF-8 bodies are base64 with `"bodyEncoding":"base64"`.
- The host rejects any `url` whose host is not in the manifest `hosts` array (exact match, or `*.` suffix match). This replaces Android's full network access for extensions.

Deviation from Aniyomi (documented): sources cannot read files or spawn processes; they cannot access the network except through `http`. This is stricter than APK extensions and is the point of the sandbox.

### 2.3 Package layout and manifest
Package = `.kuroext` zip (data file, section 7.3). Entries read in memory: `manifest.json` (max 64 KiB), `source.wasm` (max 32 MiB), `icon.png` (max 256 KiB). No extraction to disk.

Manifest Rust type:
```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManifest {
    pub schema: u32,               // must be 1
    pub pkg: String,               // [a-z0-9_.]{1,255}
    pub name: String,              // 1..=128 chars, trimmed
    pub version_name: String,      // ^\d+\.\d+\.\d+$
    pub version_code: i64,         // 0..=i64::MAX
    pub abi: String,               // "1.4" or "1.5"
    pub lang: String,
    pub nsfw: bool,
    pub factory: Option<String>,
    pub hosts: Vec<String>,        // allow-list for http bridge; may be empty (no network)
    pub signer_pub: Option<String>,// base64 of the 32-byte ed25519 public key; required once installed (9.4)
    pub icon: Option<String>,      // "icon.png" inside the archive, max 256 KiB
    #[serde(default)]
    pub sources: Vec<ManifestSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestSource { pub id: i64, pub lang: String, pub name: String, pub base_url: String }
```
`hosts` is required, may be empty. Empty means no network. `signerPub` is written by the installer, not by the package author (9.4). The manifest in data section 7.3 gains `hosts` and `signerPub`.

### 2.4 Why `wasmtime` and not an interpreter
Component model gives typed imports, memory isolation, deterministic traps and epoch interruption for timeouts. Cost: compile time on first load (mitigated by the lazier choice of no cache in v1, section 9.3).

---

## 3. Crate and module layout

```
Cargo.toml                       (workspace root = kurobayomi package; add [workspace] members)
crates/
  kuro-extensions/               no GTK; tokio; reqwest; rusqlite (via area:03-db-manga-schema-migrations handle); no wasmtime
    src/lib.rs
    src/model.rs                 Installed, Available, Untrusted, LoadResult, LibVersion, InstallStep, SourceStub
    src/manifest.rs              PackageManifest, validation
    src/index.rs                 index.min.json wire types, toExtensions() rules, ApkOnly classification
    src/repo.rs                  RepoRepository trait (impl over rusqlite), RepoRow, upsert/delete/findAll
    src/repo_refresh.rs          repo.json fetch and NOFINGERPRINT/fingerprint rule
    src/trust.rs                 Fingerprint, trust_key, TrustStore, is_trusted, trust, revoke_all
    src/store.rs                 ExtensionStore (BTreeMap state), reducer for ExtEvent, updates count
    src/loader.rs                PackageStore: scan, read_manifest, install_file, remove
    src/views.rs                 by_type(), languages(), source_items()
    src/install/mod.rs           Installer: download, verify, commit, queue, cancel
    src/install/step.rs          InstallStep
    src/update.rs                check_for_updates, gate, reconcile
    src/notify.rs                UpdateNotifier trait with GioNotifier impl lives in kurobayomi (see 3.2)
    src/events.rs                ExtEvent enum
    src/error.rs                 ExtError, LoadError
  kuro-source-wasm/              wasmtime host; depends on kuro-source (area:15-source-api-manga) and kuro-extensions models
    wit/kuro-source.wit
    src/lib.rs                   WasmSource (impl MangaSource + CatalogueSource), instantiate()
    src/bridge.rs                host impl of `http`, `pref-get`, `pref-set`, `log`, allow-list check
    src/limits.rs                StoreLimits, epoch ticker
src/                             existing GTK app
    entry.rs                     extensions() UI (replaces hard-coded rows, section 8)
    extensions_glue.rs           (new) maps ExtEvent to UI, builds notifications
```

Dependency direction (no cycles): `kurobayomi` → `kuro-extensions` → (`kuro-source` from area:15-source-api-manga for trait types only, only via `kuro-source-wasm`); `kuro-source-wasm` → `kuro-source`, `kuro-extensions::model`. `kuro-extensions` does not depend on `kuro-source-wasm`; the app wires them through the `SourceLoader` closure in section 5.5 so the loader crate stays free of wasmtime.

Rationale: keeps `kuro-extensions` building in CI without the wasm engine.

### 3.1 Why two crates
`kuro-source-wasm` is the only place that compiles and runs guest code. Keeping it separate isolates a heavy dependency (wasmtime) and its unsafe surface.

### 3.2 Notifications live in the app
`gio::Notification` needs a `gio::Application`, which the app owns. `kuro-extensions` exposes `trait UpdateNotifier { fn prompt(&self, names: &[String]); fn dismiss(&self); }`. The app implements it with `GApplication::send_notification(Some("extension-updates"), &notification)` and `withdraw_notification("extension-updates")`. The Kotlin `hideNotificationContent` rule is applied in the implementation (title only).

---

## 4. Threading and async model

| Thread or runtime | Owns | Work |
|---|---|---|
| GTK main thread (`glib::MainContext::default()`) | `ExtensionStore` in `Rc<RefCell<ExtensionStore>>`, all widgets, `gio::Notification` | Applies `ExtEvent`s, renders views, forwards commands |
| tokio multi-thread runtime, `worker_threads = 2` | HTTP fetches, downloads, file IO, DB calls via `spawn_blocking`, installer worker | Produces `ExtEvent`s |
| wasm blocking pool (tokio `spawn_blocking`) | Guest calls and wasm compilation | One guest call at a time per source (mutex in `WasmSource`) |
| epoch ticker thread (std, one per process) | `Engine::increment_epoch()` every 100 ms | Enables call deadlines |

Bridge: `async_channel::unbounded::<ExtEvent>()`. Tokio side holds `Sender`; the GTK side runs
```rust
glib::spawn_future_local(async move {
    while let Ok(ev) = rx.recv().await {
        store.borrow_mut().apply(ev);   // then re-render affected widgets
    }
});
```
Commands from GTK to tokio: `tokio::sync::mpsc::UnboundedSender<Command>` owned by `ExtensionService` (section 5.6). Commands never block the GTK thread.

Rules:
- No `block_on` on the GTK thread. No `std::fs` on the GTK thread for package files (only `gio` async APIs or tokio).
- Loader startup (section 5.4) runs on the tokio runtime and emits `Loaded` events; the window opens with an empty store and fills in (Aniyomi blocks on `runBlocking`, behaviour 3.5 step 6; the Rust version does not).
- Installer: a single `tokio::task` consumes `InstallJob`s from an `mpsc` queue (serial commit). Downloads run concurrently through a `tokio::sync::Semaphore` with 3 permits (Proposal; Kotlin has no cap, behaviour 7.2). Commit is serial through a `tokio::sync::Mutex<()>`.

Concurrency decisions:
- Per-repo index fetch: `futures::future::join_all` over repos, each with a 15 s total timeout (Proposal; Kotlin uses the OkHttp defaults from area:12-network-stack).
- Failure isolation per repo (behaviour 4.2): each repo's `Result` is mapped to `Vec<Available>` with the error logged; one failure never cancels siblings. (Fixes the cancellation defect of 9.4 for the index path.)

---

## 5. Key types and signatures

### 5.1 Model (`kuro-extensions/src/model.rs`)
```rust
/// Kotlin `Double` libVersion replaced by a tuple (behaviour 3.8 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LibVersion { pub major: u32, pub minor: u32 }

impl LibVersion {
    pub const MIN: LibVersion = LibVersion { major: 1, minor: 4 };
    pub const MAX: LibVersion = LibVersion { major: 1, minor: 5 };
    /// Kotlin rule: prefix before the last '.', then split on '.'. Returns None if not two numbers.
    pub fn from_version_name(version_name: &str) -> Option<LibVersion>;
    pub fn in_range(self) -> bool { self >= Self::MIN && self <= Self::MAX }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtension {
    pub name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    pub lib: LibVersion,
    pub lang: String,               // "", single lang, or "all"
    pub nsfw: bool,
    pub factory: Option<String>,
    pub sources: Vec<SourceInfo>,   // id, lang, name, base_url
    pub icon_path: Option<PathBuf>, // extracted on demand, see section 5.7
    pub has_update: bool,           // default false
    pub is_obsolete: bool,          // default false; reset rule in section 8.2
    pub repo_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableExtension {
    pub name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    pub lib: LibVersion,
    pub lang: String,
    pub nsfw: bool,
    pub sources: Vec<SourceInfo>,
    pub apk_name: Option<String>,   // Aniyomi entries; None for Kurobayomi wasm-only entries
    pub wasm_path: Option<String>,  // Kurobayomi entries
    pub sha256: Option<[u8; 32]>,
    pub ed25519: Option<Vec<u8>>,   // 64 bytes
    pub icon_url: String,
    pub repo_url: String,
    pub kind: PackageKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind { Wasm, ApkOnly }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntrustedExtension {
    pub name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    pub lib: LibVersion,
    pub signature_hash: String,     // digest hex
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInfo { pub id: i64, pub lang: String, pub name: String, pub base_url: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadResult {
    Success(InstalledExtension),
    Untrusted(UntrustedExtension),
    Error(LoadError),
}
```
Kotlin `MangaExtension.Available` has no `kind`; the `kind` field is added for the ApkOnly rule (behaviour 16.2). `Installed.isShared` is removed (data 12).

`SourceInfo.base_url` in Kotlin is a field of `Available.MangaSource` only; the installed `Installed.sources` carry `MangaSource` objects. Rust keeps one struct for both to avoid a mapping layer (Proposal).

### 5.2 Install step (`install/step.rs`)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep { Idle, Pending, Downloading, Installing, Installed, Error }

impl InstallStep {
    /// Kotlin isCompleted(): Installed, Error, Idle.
    pub fn is_completed(self) -> bool { matches!(self, Self::Idle | Self::Installed | Self::Error) }
}
```
Variant order matches Kotlin ordinals (data file section 1.5).

### 5.3 Events (`events.rs`)
```rust
#[derive(Debug)]
pub enum ExtEvent {
    /// Startup scan finished. Replaces `_isInitialized = true` (behaviour 2.2).
    Loaded { installed: Vec<InstalledExtension>, untrusted: Vec<UntrustedExtension> },
    /// Replaces onExtensionInstalled (behaviour 2.11). Reconciliation already applied by the sender.
    Installed(InstalledExtension),
    Updated(InstalledExtension),
    Untrusted(UntrustedExtension),
    Removed { pkg: String },
    /// Replaces the available map replacement (behaviour 2.3 step 3).
    AvailableReplaced { available: Vec<AvailableExtension>, failed_repos: Vec<String> },
    /// Replaces the updates count write (behaviour 2.12).
    UpdatesCount(usize),
    /// Install progress for one package key (behaviour 7.12).
    Step { key: String, step: InstallStep },
    /// Repo index failed completely (replaces the toast; behaviour 2.3 step 1).
    FetchFailed,
}
```

### 5.4 Store and reducer (`store.rs`)
```rust
#[derive(Default)]
pub struct ExtensionStore {
    pub installed: BTreeMap<String, InstalledExtension>,   // key = pkg
    pub untrusted: BTreeMap<String, UntrustedExtension>,
    pub available: BTreeMap<String, AvailableExtension>,   // last-wins (behaviour 2.3 step 3)
    pub stubs: BTreeMap<i64, SourceInfo>,                   // source id -> stub (behaviour 2.5)
    pub steps: BTreeMap<String, InstallStep>,               // in-flight per key
    pub updates_count: usize,
}

impl ExtensionStore {
    pub fn apply(&mut self, ev: ExtEvent) -> ChangeSet;     // which views must re-render

/// Bit set of changed views: 1 = installed, 2 = untrusted, 4 = available, 8 = steps, 16 = updates count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChangeSet(pub u8);
    pub fn source_package(&self, source_id: i64) -> Option<&str>;   // behaviour 2.4
    pub fn source_data(&self, source_id: i64) -> Option<&SourceInfo>; // behaviour 2.5
}
```
Reducer rules:
- `Installed` / `Updated`: insert by pkg (replace), then `updates_count` recomputed (behaviour 2.12). For `Installed` the sender calls `with_update_check` first.
- `Untrusted`: remove from `installed`, insert into `untrusted` (behaviour 2.11).
- `Removed`: remove from `installed` and `untrusted` (behaviour 2.10).
- `AvailableReplaced`: if `available` is empty: set `updates_count = 0` and keep `installed` unchanged (behaviour 6 step 1). Else replace `available` (last-wins) and reconcile (section 8.2). Stub map replaced only if non-empty (behaviour 2.5).
- `Step`: `Idle` on a completed step removes the key after the UI has rendered it; `Installed` also removes the key (Kotlin removes the download state on completion, behaviour 12.1).

### 5.5 Loader wiring (`kuro-extensions` ↔ `kuro-source-wasm`)
```rust
pub type SourceLoader = dyn Fn(&Path, &PackageManifest) -> BoxFuture<'static, Result<Vec<Arc<dyn CatalogueSource>>, LoadError>> + Send + Sync;
```
The app passes `kuro_source_wasm::instantiate` wrapped in this closure. The closure is the only point where guest code runs. Its failure produces `LoadResult::Error(LoadError::Instantiate(msg))`, matching the all-or-nothing rule of behaviour 3.8 step 9.

### 5.6 Service (`kuro-extensions/src/lib.rs`)
```rust
pub struct ExtensionService { cmd: tokio::sync::mpsc::UnboundedSender<Command>, /* shared read-only handles */ }

pub enum Command {
    LoadAll,
    FindAvailable { force: bool },            // force = pull-to-refresh (behaviour 12.1)
    CheckForUpdates { from_available: bool }, // behaviour 8.1
    Install { pkg_key: String, row: AvailableExtension },
    Update { pkg: String },                   // behaviour 2.8 updateExtension
    Cancel { key: String },
    Uninstall { pkg: String },
    Trust { pkg: String },                    // behaviour 2.9
    RevokeAllTrust,
    AddRepo { base_url: String },
    DeleteRepo { base_url: String },
}

impl ExtensionService {
    pub fn send(&self, cmd: Command);         // never blocks; drops only if the runtime has stopped
}
```
Replies go through `ExtEvent` only. No return values, so the GTK thread never awaits.

### 5.7 Icons
`icon_path` is the extracted `icon.png` from the package, written to `$XDG_CACHE_HOME/kurobayomi/extensions/icons/<pkg>.png` on first load. The UI loads it with `gdk::Texture::from_filename`. Missing icon → the UI shows a generic extension glyph (`application-x-addon-symbolic`, the same icon the mockup uses; `src/entry.rs:335`). Apk-only entries use the repo `iconUrl` through the area:17-source-app-layer image cache.

---

## 6. Trust and signatures

### 6.1 Fingerprint
```rust
/// SHA-256 digest of an ed25519 public key (32 bytes). Hex form is lowercase, 64 chars.
pub struct Fingerprint(pub [u8; 32]);
impl Fingerprint {
    pub fn of_public_key(key: &[u8; 32]) -> Self { Fingerprint(sha2::Sha256::digest(key).into()) }
    pub fn to_hex(&self) -> String;   // lowercase
    pub fn parse_hex(s: &str) -> Option<Fingerprint>;  // accepts lowercase only (behaviour 10.4)
}
```
Replaces the APK certificate digest (`Hash.sha256(signature bytes)`, behaviour 3.9). The digest is over the **public key bytes**, not over a certificate.

### 6.2 Trust store (`trust.rs`)
```rust
pub struct TrustStore {
    repos: Arc<dyn RepoRepository>,   // section 7
    prefs: Arc<dyn PrefStore>,        // area:02-preferences-system app-state store
}

impl TrustStore {
    /// Behaviour 10.1. `signer` is the single digest of the package signer (Kurobayomi packages have exactly one signer).
    pub async fn is_trusted(&self, pkg: &str, version_code: i64, signer: &Fingerprint) -> Result<bool, ExtError> {
        let repo_match = self.repos.all().await?.iter().any(|r| r.signing_key_fingerprint == signer.to_hex());
        let key = trust_key(pkg, version_code, &signer.to_hex());
        Ok(repo_match || self.prefs.app_state_set("trusted_extensions").contains(&key))
    }

    /// Behaviour 10.2: removes every entry starting with "<pkg>:", then inserts "<pkg>:<versionCode>:<digest>".
    pub async fn trust(&self, pkg: &str, version_code: i64, signer: &Fingerprint) -> Result<(), ExtError>;

    /// Behaviour 10.3.
    pub async fn revoke_all(&self) -> Result<(), ExtError>;
}

pub fn trust_key(pkg: &str, version_code: i64, digest_hex: &str) -> String {
    format!("{pkg}:{version_code}:{digest_hex}")
}
```
Deviation (documented): Kotlin's repo rule checks any of several digests; Kurobayomi checks the one signer. For `ApkOnly` entries (never loaded), trust is not evaluated.

Precision: the `startsWith("$pkg:")` filter in `trust` must match the full `pkg` followed by `:` so that `a.b` does not remove entries for `a.b.c`. The Kotlin code has this property because the separator is included.

### 6.3 Signature verification (installer, behaviour 16.4)
```rust
pub fn verify_package(bytes: &[u8], index_sha256: &[u8; 32], sig: &[u8], repo_key: &VerifyingKey) -> Result<(), VerifyError> {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    if &digest != index_sha256 { return Err(VerifyError::DigestMismatch); }
    let sig = ed25519_dalek::Signature::from_slice(sig).map_err(|_| VerifyError::BadSignatureLength)?;
    repo_key.verify_strict(&digest, &sig).map_err(|_| VerifyError::BadSignature)
}
```
- `verify_strict` rejects non-canonical signatures (malleability).
- `repo_key` is the public key whose `Fingerprint` equals the repo's `signing_key_fingerprint`. The repo's key is not stored in the database (the table holds only the fingerprint). Therefore the installer obtains the **public key bytes** from the package's own index entry field `ed25519Pub` (32-byte base64) and checks `Fingerprint::of_public_key(pub) == repo fingerprint` before verifying. Added to the index entry (data section 7.5).

Proposal: add optional `ed25519Pub` to the index entry. Without it, a `Wasm` entry is rejected with `LoadError::NotSigned` at install time.

---

## 7. Repo storage (`repo.rs`)

```rust
pub struct RepoRow {
    pub base_url: String,
    pub name: String,
    pub short_name: Option<String>,
    pub website: String,
    pub signing_key_fingerprint: String,
}

#[async_trait::async_trait]
pub trait RepoRepository: Send + Sync {
    async fn all(&self) -> Result<Vec<RepoRow>, ExtError>;               // ORDER BY rowid
    async fn find(&self, base_url: &str) -> Result<Option<RepoRow>, ExtError>;
    async fn find_by_fingerprint(&self, fp: &str) -> Result<Option<RepoRow>, ExtError>;
    async fn count(&self) -> Result<usize, ExtError>;
    async fn insert(&self, row: &RepoRow) -> Result<(), ExtError>;       // ExtError::RepoSave on constraint failure
    async fn upsert(&self, row: &RepoRow) -> Result<(), ExtError>;
    async fn delete(&self, base_url: &str) -> Result<(), ExtError>;
}
```
Implementation: `rusqlite::Connection` owned by area:03-db-manga-schema-migrations's database handle, every call in `tokio::task::spawn_blocking`. SQL is the `excluded.`-form in data section 9.3. `replace` is not implemented (data 9.3).

Behaviour 9.2 rule is a free function, testable without a database:
```rust
pub fn should_apply_refresh(stored_fp: &str, fetched_fp: &str) -> bool {
    stored_fp.starts_with("NOFINGERPRINT") || stored_fp == fetched_fp
}
```

---

## 8. Index, reconciliation and update check

`ExtensionCtx` (shared by the service tasks) holds: `Arc<dyn RepoRepository>`, `Arc<TrustStore>`, `PackageStore`, the `async_channel::Sender<ExtEvent>`, the `Arc<dyn UpdateNotifier>` (section 3.2), a `PrefStore` handle for `last_ext_check` and `ext_updates_count` (area:02-preferences-system keys), and a `reqwest::Client` from area:12-network-stack.

### 8.1 Index classification (`index.rs`)
For each wire entry (data section 7.1):
1. `lib` gate: `LibVersion::from_version_name(&version)`; accept iff `Some(v)` and `v.in_range()`. Note: the Kotlin `toDouble()` throws for non-numeric prefixes which drops the whole repo. Rust drops only the entry, with a warning (deliberate change; behaviour 4.4 step 2).
2. `kind`: `Wasm` when `wasm` and `sha256` are present; else `ApkOnly` (behaviour 16.2).
3. Required-field failures drop the entry, not the repo. Deliberate change (behaviour 4.3 observed defect).

### 8.2 Reconciliation (`store.rs`, behaviour 6)
Pseudo-code for `reconcile(installed, available) -> installed'`:
```rust
for (pkg, ext) in installed.iter_mut() {
    match available_first_match(pkg) {      // first match, to keep Kotlin find() semantics
        None => if !ext.is_obsolete { ext.is_obsolete = true; }
        Some(av) => {
            ext.is_obsolete = false;        // Kurobayomi fix: reset when found again (behaviour 6 defect)
            ext.has_update = update_exists(ext, av);
            ext.repo_url = Some(av.repo_url.clone());
        }
    }
}
```
`update_exists(installed, available) = available.version_code > installed.version_code || available.lib > installed.lib` (behaviour 2.12). Use the same `available` lookup rule (first match) as step 3 to remove the inconsistency of behaviour 6. Rule chosen for duplicate `pkg` across repos: **first repo in table order wins** (this is `find`); the `available` map is built in the same order with `entry().or_insert` (first wins). Deviation from Kotlin's last-wins `associateBy`, documented; Kotlin's two rules disagreed, Rust has one.

### 8.3 Update count
`updates_count = installed.values().filter(|e| e.has_update).count()` (obsolete included, behaviour 2.12). Persisted to area:02-preferences-system setting `ext_updates_count` on every change. Zero → `UpdateNotifier::dismiss()`.

### 8.4 Update check (`update.rs`, behaviour 8)
```rust
pub async fn check_for_updates(
    svc: &ExtensionCtx,
    from_available: bool,
    now_ms: i64,
) -> Result<Option<Vec<String>>, ExtError>   // None = gated; Some(names) = installed names with updates
```
Steps: gate (only when `from_available`, `now_ms < last_ext_check + 86_400_000` → `Ok(None)`); refresh all repos (section 9); fetch index unless `from_available` (then use the store's `available`); load installed packages from disk (`PackageStore::scan`, full reload as in Kotlin); compare; if non-empty → `UpdateNotifier::prompt(names)`; return names.

Deviation (documented): Kotlin's `checkForUpdates` never updates the badge count (behaviour 8.2). Rust emits `UpdatesCount` from the same call, so the badge and the notification agree.

### 8.5 Refresh of repos (`repo_refresh.rs`, behaviour 9)
`GET {base}/repo.json` (timeout 15 s), parse `ExtensionRepoMetaDto`, apply `should_apply_refresh`, then `upsert`. Failures are logged and the repo keeps its stored row. `join_all` over repos, each isolated (no `?` across repos; fixes behaviour 9.4).

---

## 9. Loader and install

### 9.1 `PackageStore` (`loader.rs`)
```rust
pub struct PackageStore { root: PathBuf }   // $XDG_DATA_HOME/kurobayomi/extensions

impl PackageStore {
    /// One file per package: "<pkg>.kuroext". Behaviour 3.5, single store (no shared).
    pub fn scan(&self) -> Vec<(PathBuf, Result<PackageManifest, LoadError>)>;
    pub fn path_for(&self, pkg: &str) -> PathBuf;
    /// Behaviour 3.3 install rules, applied before any file write.
    pub fn check_install(&self, new: &PackageManifest, new_signer: &Fingerprint) -> Result<(), LoadError>;
    /// Atomic commit (data section 5.3).
    pub fn commit(&self, src_bytes: &[u8], manifest: &PackageManifest) -> Result<PathBuf, LoadError>;
    /// Behaviour 7.8 private branch: delete the file; caller emits Removed.
    pub fn remove(&self, pkg: &str) -> io::Result<()>;
}
```

`check_install` rules, in order (behaviour 3.3):
1. If an installed version exists and `new.version_code < current.version_code` → `LoadError::Downgrade`.
2. If an installed version exists and `new_signer != current_signer` → `LoadError::SignerMismatch`. (Kotlin `containsAll`; with one signer, equality.)
3. Equal version codes are accepted (reinstall).

`scan` order: sorted by file name (deterministic; Kotlin's `listFiles()` order is unspecified). Files that are not `*.kuroext`, or whose manifest fails, are reported as `LoadError` and never make the batch fail (fixes behaviour 3.8 step 9 defect for the manifest; per-source failure is still all-or-nothing for that extension).

### 9.2 Load order per package (behaviour 3.8, Rust)
```rust
/// Shared load inputs: the trust store, the NSFW setting read for this load, and the source loader closure (5.5).
pub struct LoadCtx {
    pub trust: Arc<TrustStore>,
    pub allow_nsfw: bool,
    pub source_loader: Arc<SourceLoader>,
}

pub async fn load_one(ctx: &LoadCtx, path: &Path, manifest: PackageManifest) -> LoadResult {
    // 1. name normalisation: done in manifest validation (strip "Tachiyomi: ")
    // 2. version name present: done in manifest validation (non-empty, ^\d+\.\d+\.\d+$)
    // 3. lib gate: LibVersion::from_version_name(version_name) + in_range -> Error(LoadError::LibVersion)
    // 4. signed: signer_pub present and decodes to 32 bytes -> else Error(LoadError::NotSigned)
    //    signer = Fingerprint::of_public_key(signer_pub)
    // 5. trust: ctx.trust.is_trusted(pkg, version_code, &signer) -> false => Untrusted(UntrustedExtension { .. })
    // 6. nsfw gate: manifest.nsfw && !ctx.allow_nsfw -> Error(LoadError::NsfwDisabled)
    // 7. instantiate: (ctx.source_loader)(path, &manifest).await -> Err(e) => Error(LoadError::Instantiate(e))
    // 8. lang derivation over catalogue sources: 0 -> "", 1 -> that lang, else "all"
    // 9. Success(InstalledExtension { name, pkg, version_name, version_code, lib, lang, nsfw, factory, sources, icon_path, has_update: false, is_obsolete: false, repo_url: None })
}
```
Order is identical to behaviour 3.8: lib gate → signed → trusted → NSFW → load. The `Untrusted` result comes **before** the NSFW gate (behaviour 3.8 step 7 note).

`allow_nsfw` is read per load from the setting (Kotlin reads once per process; Rust reads per load: deliberate change, documented in behaviour 3.1).

### 9.3 Compile cache
v1: no cache. Each start compiles each `source.wasm` (Cranelift). Lazier choice recorded; upgrade path: `Module::serialize` into `$XDG_CACHE_HOME/kurobayomi/extensions/compiled/<sha256>.cwasm`, validated with `Engine::precompile_compatibility_hash`. Add when startup exceeds 2 s with 10 packages.

### 9.4 Signer storage
The installed package's signer is needed for signer-continuity (9.1) and trust. It is recomputed on every scan from the **embedded** `manifest.json` field `signerPub` (base64 of the 32-byte public key), which the installer writes only after verifying the package against the index (6.3). Because the installed file is read-only (`0444`) and the path is under the app data directory, tampering requires local write access, which already implies full compromise. Documented as the trust boundary.

Manifest field added: `signerPub: String` (required for installed packages; a package without it is `LoadError::NotSigned`).

### 9.5 Installer (`install/mod.rs`, behaviour 7 and 8)
```rust
pub struct InstallJob {
    pub key: String,                  // row key (pkg, or "<pkg>-<sourceId>" for split rows; see 9.6)
    pub pkg: String,
    pub name: String,
    pub url: Url,                     // <repo>/<wasm path>
    pub index_sha256: [u8; 32],
    pub index_ed25519: Vec<u8>,
    pub index_pub: [u8; 32],          // from ed25519Pub (6.3)
    pub repo_fingerprint: String,     // must equal Fingerprint::of_public_key(index_pub).to_hex()
    pub cancel: CancellationToken,
}

pub struct Installer {
    cmd: mpsc::UnboundedReceiver<InstallJob>,  // one consumer task
    events: async_channel::Sender<ExtEvent>,
    downloads: Semaphore,                      // 3 permits
    commit_lock: tokio::sync::Mutex<()>,
    active: std::sync::Mutex<HashMap<String, CancellationToken>>,
    store: PackageStore,
    trust: Arc<TrustStore>,
}
```

Per job, the state sequence emitted as `ExtEvent::Step`:
1. `Pending` on enqueue (behaviour 7.2 step 4).
2. `Downloading` when a permit is acquired and the first byte arrives (behaviour 7.3 `STATUS_RUNNING`).
3. Verify: digest, then signature, then `repo_fingerprint` check (6.3). Failure → `Error`.
4. `Installing` at the start of commit (the Kotlin `setInstalling`, behaviour 7.12; emitted for every route, a deliberate change so the UI shows the same states for all paths).
5. Trust-independent install check (`check_install`, 9.1). Failure → `Error`.
6. Atomic commit (`PackageStore::commit`). Then `ExtEvent::Installed` (the reconciled record) and `Step::Installed`.
7. Cancel at any point before step 6 → `Idle` (behaviour 7.7 says Kotlin does not do this; Rust does, deliberate). Cancel during step 6 is ignored (the commit is atomic and short).

Errors map to `Error`; partial files are removed (`.part`).

Downloads: `reqwest` GET, `bytes_stream()`, write to `$XDG_CACHE_HOME/kurobayomi/extensions/download/<pkg>-<versionName>.kuroext.part`. Max 64 MiB; exceeding → `Error`. Connect timeout 10 s; total timeout 120 s (data 6.5).

### 9.6 Keys and duplicate downloads
Key for queue, state and cancel = **package name** (`pkg`), not the row key with `-<sourceId>` (behaviour 11.1 proposal). A split row of the same APK reuses the active job if one exists; the second request for the same pkg is answered by the existing job (no second download). Deliberate change.

### 9.7 Uninstall (behaviour 7.8)
`Command::Uninstall { pkg }`: `PackageStore::remove(pkg)`; emit `Removed { pkg }`; the reducer removes it (behaviour 2.11 `onPackageUninstalled`). Confirmation dialog text is in area:17-source-app-layer (data 12.3).

### 9.8 Trust action (behaviour 2.9)
`Command::Trust { pkg }`: read the untrusted record from the store; `TrustStore::trust(pkg, version_code, signer)`; `PackageStore::scan` for this package; `load_one`; on `Success` emit `Installed`. On `Untrusted` again: nothing is emitted (matches behaviour 2.9 note).

---

## 10. Views (`views.rs`, behaviour 11)

```rust
pub struct BrowseBuckets {
    pub updates: Vec<InstalledExtension>,
    pub installed: Vec<InstalledExtension>,
    pub available: Vec<AvailableExtension>,   // split rows: pkg = "<pkg>-<sourceId>"
    pub untrusted: Vec<UntrustedExtension>,
}

pub fn by_type(store: &ExtensionStore, enabled_langs: &BTreeSet<String>, show_nsfw: bool) -> BrowseBuckets;
pub fn languages(store: &ExtensionStore, enabled_langs: &BTreeSet<String>) -> Vec<String>;
pub fn source_items(ext: &InstalledExtension, disabled: &BTreeSet<String>) -> Vec<SourceItem>;

pub struct SourceItem { pub source: SourceInfo, pub enabled: bool, pub label_as_name: bool }
```
Rules are the behaviour 11 rules, exactly:
- `by_type` installed sort: key `(is_obsolete as u8 == 0 ...)`. Precisely: sort by `(!is_obsolete)` ascending, then `name.to_lowercase()` ascending. Because `false < true`, obsolete items come first (behaviour 11.1). Implement as `sort_by_key(|e| (!e.is_obsolete, e.name.to_lowercase()))`. Partition by `has_update`.
- Untrusted: `sort_by_key(|u| u.name.to_lowercase())`; no NSFW filter (behaviour 11.1, observed).
- Available: exclude where `installed` has the pkg or `untrusted` has the pkg, or `nsfw && !show_nsfw`; then per source with lang in `enabled_langs` (or the top-level lang when there are no sources) produce a row; `pkg = format!("{}-{}", pkg, source.id)`; sort by lowercase name.
- Languages: distinct source langs (or `ext.lang` if none), sorted by `(!enabled, locale_compare)`, with `"all"` first among equals (LocaleHelper comparator, behaviour 11.2).
- `label_as_name = multi && !(distinct names == 1)`, where `multi = sources.len() > 1` (behaviour 11.3).

`show_nsfw` is passed per call. Rust re-evaluates on every render (deliberate change from subscribe-time read, behaviour 11.1).

Locale display names come from area:17-source-app-layer's `LocaleHelper` port.

---

## 11. Android to Linux mapping

| Android / Kotlin | Linux / Rust replacement | Notes |
|---|---|---|
| `PackageManager` (installed packages, archive info) | `PackageStore` over `$XDG_DATA_HOME/kurobayomi/extensions`, manifest in zip | Section 9.1 |
| `ChildFirstPathClassLoader`, `Class.forName`, `newInstance` | `wasmtime::component` instantiate, `invoke` export | Section 2 |
| `Signature`, `SigningInfo`, `Hash.sha256` | `ed25519-dalek` `VerifyingKey`, `sha2::Sha256` | Section 6 |
| `DownloadManager` | `reqwest` streaming + `tokio::fs` | Section 9.5 |
| `BroadcastReceiver`, `LocalBroadcastManager`, `ACTION_*` | `ExtEvent` over `async_channel`; `Command` over tokio mpsc | Section 4 |
| `ContentResolver.openInputStream`, `FileProvider` | Not needed (paths only) | — |
| `Service` foreground notification (-402) | None. In-app per-row progress | Dropped (behaviour 15) |
| `Notification`, channel `ext_apk_update_channel`, id -401 | `gio::Notification` id `extension-updates` via `GApplication` | Section 3.2 |
| `Activity` for install intent | None | Dropped |
| `SharedPreferences` / `PreferenceStore` | area:02-preferences-system settings store | Keys in data section 3 |
| `Injekt.get<T>()` | constructor injection (area:01-foundation-app-bootstrap) | — |
| `kotlinx.coroutines` (`Flow`, `StateFlow`, `runBlocking`) | tokio tasks, `ExtensionStore` on the GTK thread | Section 4 |
| `withUIContext` / `withIOContext` | `glib::spawn_future_local` / `tokio::spawn_blocking` | — |
| `Locale.getDefault().language` | parse `LC_MESSAGES` then `LANG` (env), take the part before `_` or `.` | No crate (lazier) |
| `Uri.toUri`, `lastPathSegment` | `url::Url::path_segments().last()` | — |
| `kotlinx.serialization` `Json` (`ignoreUnknownKeys`, `explicitNulls = false`) | `serde_json` with no `deny_unknown_fields`; `Option<T>` fields | Matches behaviour 4.3 |
| `substringAfter("Tachiyomi: ")` | `s.strip_prefix("Tachiyomi: ").unwrap_or(s)` | Behaviour 3.8 step 1 |
| `substringBeforeLast('.')` | `rsplit_once('.').map(|(a,_)| a)` or whole string | Behaviour 3.8 step 4 |
| `Double` lib compare | `LibVersion` tuple | Section 5.1 |
| `File.copyAndSetReadOnlyTo` | write `.part`, rename, `chmod 0444` | Data 5.3 |
| `Icon` `loadIcon` | extract `icon.png` to cache, `gdk::Texture` | Section 5.7 |
| Shizuku, PackageInstaller, MIUI | None | Dropped |

---

## 12. Error handling

Library errors use `thiserror`; the app logs with `tracing` and shows a toast only for the key listed in behaviour 15 (`extension_api_error`, text `Failed to fetch available extensions`).

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExtError {
    #[error("Failed to fetch available extensions")]
    FetchFailed,                        // index fetch failed for every repo and there is no cached list
    #[error("repo database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Error Saving Repository to Database")]
    RepoSave(#[source] rusqlite::Error), // data 9.2 SaveExtensionRepoException
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
    #[error("Package {0} isn't signed")]            NotSigned(String),
    #[error("Lib version {0} is not allowed")]      LibVersion(String),
    #[error("NSFW extension {0} not allowed")]      NsfwDisabled(String),
    #[error("Installed extension version is higher. Downgrading is not allowed.")] Downgrade,
    #[error("Installed extension signature is not matched.")] SignerMismatch,
    #[error("Extension load error: {0}")]           Instantiate(String),
    #[error("Invalid manifest: {0}")]               Manifest(String),
    #[error("Not an extension package")]            NotAnExtension,
}
```
Message text for `LoadError` variants is the Kotlin log text (data section 13) so logs stay greppable across the two codebases.

---

## 13. Integration with the existing Kurobayomi mockup

### 13.1 Current state (OBSERVED in `src/entry.rs`)
- `fn extensions(anime: bool) -> gtk::ScrolledWindow` at `src/entry.rs:311`.
- Hard-coded rows: `("", [("Extension A", "1.4.13", "Update")])`, `("Installed", [B, C])`, `("English", [D, E])`, `("Japanese", [F])` at `src/entry.rs:322-327`.
- Action buttons switch labels locally (`Install`/`Update` → `Installing`) at `src/entry.rs:332-345`. No state, no install.
- Trust dialog is opened by `dialog_of("browse", "extension_trust")` (`src/entry.rs:337`). Details by `open("extension_details")` (`src/entry.rs:340`, `:352`).
- Sidebar badge comes from `src/main.rs:153` ("extension updates" badge) with no data source.

### 13.2 Target state
- `extensions(anime: bool, store: &ExtensionStore, svc: &ExtensionService)` renders from `store` for the manga branch. The anime branch stays as the mockup until area:16-source-api-anime lands (this area fills manga only).
- Sections in this order, exactly as behaviour 11.1 buckets: header `Updates pending` with `Update all` (`Update all` sends `Command::Update` for each `updates` row); `Installed` (installed rows, obsolete first); `Untrusted` (appears inside `Installed`, label `Untrusted`, destructive style as in the mockup); then available rows grouped by language header from `languages()`.
- Row labels: name, then `"{version} • {LANG}"` (mockup format `src/entry.rs:343`) for installed and available; untrusted and obsolete rows add `UNTRUSTED` or `ORPHANED` tags (area:17-source-app-layer owns the wording).
- Row actions: available → `Install` (`Command::Install`); updates → `Update`; in-flight → `Cancel` (`Command::Cancel`); error → `Retry` (`Command::Install` again); installed → `Settings`; untrusted → `Trust` (dialog, then `Command::Trust`).
- Search entry filters `by_type` results by name (area:17-source-app-layer behaviour; this area provides the lists only).
- Pull to refresh (`Command::FindAvailable { force: true }`) shows a spinner for at least 1 s (behaviour 12.1 "fake slower refresh", kept).
- The `Installing` state label from the mockup is removed; it is replaced by real `InstallStep` values.

### 13.3 Glue
`src/extensions_glue.rs` (new): receives `ExtEvent` on the GTK thread, updates `ExtensionStore`, calls `extensions(...)` re-render through the existing `Rc` navigation helpers, and sets the sidebar badge from `updates_count` (main.rs:153).

---

## 14. Pitfalls

1. A package whose `Installed` record is never reconciled (e.g. installed while the index is down) keeps `has_update = false` until the next `FindAvailable`. Accepted; documented in the UI copy of area:17-source-app-layer.
2. `wasmtime` `Store` memory: set `StoreLimitsBuilder::memory_size(256 << 20)` per guest instance, or a bad source exhausts RAM.
3. Epoch ticker must run only once per process; create it in `kuro-source-wasm::init()` guarded by `std::sync::OnceLock`.
4. Source ids are `i64`. JSON numbers above 2^53 are exact in `serde_json` integers; never deserialise into `f64` (Kotlin uses `Long`, behaviour 4.3).
5. Zip reads: use `ZipArchive::by_name` only, never `extract`. Bound each entry size before reading (`take(limit)`).
6. `ed25519` `verify_strict` only. Plain `verify` accepts some malleable signatures.
7. `rename` is atomic only within one filesystem. Downloads go to the cache directory, which may be on another filesystem. The commit step therefore copies the verified bytes to `<data>/<pkg>.kuroext.part` in the target directory, fsyncs, then renames over `<data>/<pkg>.kuroext` (data 5.3).
8. GTK: `gio::Notification` requires a registered `GApplication` with an application id; the app already has one (area:01-foundation-app-bootstrap).
9. Do not hold the `ExtensionStore` `RefCell` borrow across an `await`.
10. The `apk` field of Aniyomi entries must never be fetched or executed; `kind = ApkOnly` short-circuits install with `LoadError::NotAnExtension` shown as "Not runnable on Linux".

---

## 15. Dependencies (crate, why, feature set)

| Crate | Version line | Why | Features |
|---|---|---|---|
| `tokio` | 1 | async runtime for fetch, download, file IO, installer queue | `rt-multi-thread`, `sync`, `time`, `fs`, `macros` |
| `tokio-util` | 0.7 | `CancellationToken` | default |
| `reqwest` | 0.12 | HTTP (area:12-network-stack owns the shared client) | `rustls-tls`, `stream`, `json` (no default TLS) |
| `async-channel` | 2 | bridge to the GTK main context | default |
| `futures-util` | 0.3 | `join_all`, `StreamExt` | default |
| `rusqlite` | 0.32 | `extension_repos` (area:03-db-manga-schema-migrations database) | `bundled` (area:03-db-manga-schema-migrations decides) |
| `serde`, `serde_json` | existing (1.0.229 / 1.0.151) | index, repo.json, manifest | `derive` |
| `zip` | 2 | read `.kuroext` in memory | `default-features = false`, `deflate` |
| `ed25519-dalek` | 2 | signature verification | default (`verify_strict` used) |
| `sha2` | 0.10 | SHA-256 for digests and fingerprints | default |
| `base64` | 0.22 | decode `ed25519`, `ed25519Pub`, `signerPub` | `std` |
| `wasmtime` | latest stable major at implementation time | guest runtime (component model) | `component-model`, `cranelift` |
| `thiserror` | 2 | error types | default |
| `tracing` | 0.1 | logs | default |
| `url` | 2 | URL building and host allow-list | default |
| `gtk4` (existing), `glib`, `gio` | existing (0.11.5 line) | main context, `gio::Notification` | existing features |

Not used, with reason:
- `anyhow`: library crates expose typed errors; the app binary may use it at the top level only.
- `notify` (inotify): no live directory watch; the store reloads on explicit commands.
- `sys-locale`: env parse is enough for the two-letter code (lazier, section 11).
- `libloading`, `dlopen`: not used; no native plugins.
- `async-trait` on `RepoRepository`: kept only because area:15-source-api-manga's traits already use it; repo trait may drop it if `Box<dyn Future>` is written by hand.

---

## 16. Test seams

- `LibVersion::from_version_name` is a pure function: table tests from behaviour 3.8 step 4 examples.
- `trust_key` and the `startsWith("<pkg>:")` filter: pure, unit-tested with `a.b` vs `a.b.c`.
- `should_apply_refresh`: pure table test (behaviour 9.2).
- `by_type`, `languages`, `source_items`: pure functions over `ExtensionStore` fixtures (edge cases file).
- `verify_package`: fixture key pair, valid and malleable signatures.
- `PackageStore::check_install`: temp directory tests for downgrade, equal version, signer mismatch.
- The GTK layer is not unit-tested; it is covered by the `run` skill smoke test.

Test fixtures are listed in `p1-edge-cases-and-tests.md`.
