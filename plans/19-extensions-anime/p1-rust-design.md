# 19-extensions-anime: Rust design

Target: the Kurobayomi single crate (`/home/blackbox/kurobayomi`, `Cargo.toml` package `kurobayomi`, edition 2024, `gtk4` 0.11 with feature `v4_16`). The existing mockup is `src/main.rs`, `src/settings.rs`, `src/entry.rs`, `src/ui.rs`, `src/screens.rs`, `src/spec.rs`, `src/clamp.rs`, `src/player.rs`, `src/entry.rs`.

Decisions this design depends on, taken from sibling plans and not reopened here:
- Anime extensions are Rust-loaded `.kuroext` packages, a zip with `manifest.json` and `source.wasm`, run in `wasmtime`. The same container and verification are used for manga (area:18-extensions-manga sections 7 and 16). The anime WIT world is owned by area:16-source-api-anime. The mechanism choice is area:20-extension-strategy-redesign.
- Builtin anime sources are a static list in `src/source/registry.rs` (area:16-source-api-anime). Builtin sources do not go through this area's install path.
- Only the Android-only mechanisms are dropped. The store protocol, lib gate, trust model, install queue semantics, and notifications are ported.

Sections referenced from the other files: section 6 is the install pipeline and queue. Section 9 is the Android-to-Linux mapping.

---

## 1. Module layout

Proposal. Everything lives under `src/extensions/anime/`. If the workspace is later split (area:20-extension-strategy-redesign may decide this), the same tree moves to a crate `kuro-extensions` (area:18-extensions-manga naming).

```
src/extensions/
  mod.rs                  // pub mod anime; (manga is area:18-extensions-manga)
  anime/
    mod.rs                // facade: AnimeExtensions service handle, re-exports
    model.rs              // Installed, Available, Untrusted, Kind, LoadResult, InstallStep, StoreRef
    abi.rs                // AbiVersion (major, minor), SUPPORTED_ABI, parse rules
    wire.rs               // serde structs for store JSON, legacy JSON, prost structs for protobuf
    store/
      mod.rs              // IndexFetcher: fetch(), extension list, redirects, gzip
      decode.rs           // first-byte format detection, gzip sniffing
      map.rs              // wire -> Available mapping (lang, nsfw, abi, sources)
      repo.rs             // rusqlite table access (get, list, count, upsert, delete)
      refresh.rs          // refresh_all, fetch_all_available
    package/
      mod.rs              // KuroextPackage: open zip, read manifest, validate
      manifest.rs         // Manifest struct and validation rules
      verify.rs           // sha256 of file, ed25519 of digest, signer fingerprint
    loader.rs             // discover installed dir, select, load -> LoadResult
    trust.rs              // trust rule, trust entry format, revoke
    install/
      mod.rs              // InstallService: queue, steps, cancel
      download.rs         // reqwest streaming download to .part, progress, cancel token
      place.rs            // downgrade and signer-continuity checks, atomic rename into dir
    uninstall.rs          // remove installed file, emit event
    manager.rs            // AnimeExtensionManager: state maps, events, reconciliation
    browse.rs             // buckets, sorting, NSFW filter, split rows, languages, source toggles
    updates.rs            // checkForUpdates, update count, notification
    icons.rs              // icon cache paths
    backup.rs             // BackupExtensionStore <-> AnimeStore mapping
    deeplink.rs           // aniyomi://add-repo?url=... and extension-store
  ui/anime_stores.rs      // stores page (list, empty, FAB, refresh, actions)
  ui/anime_store_dialogs.rs // create, confirm (deep link), delete
  ui/anime_trust.rs       // untrusted dialog
```

Existing mockup code that changes: `src/settings.rs:13` (`"settings_anime_extension_stores" => cards(id, &["Store A", "Store B"], false)`) is replaced by a page built from the store list. `src/settings.rs:133` (`cards` helper) is reused for the list row styling.

---

## 2. Core types (`src/extensions/anime/model.rs`)

```rust
use std::sync::Arc;

/// Packaging kind of an index entry. Derived in `map.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `kuroext` block present, signature and key present and well-formed. Installable.
    Wasm,
    /// `kuroext` block present but `ed25519` or `publicKey` missing or malformed. Listed, not installable.
    Unsigned,
    /// Aniyomi APK entry without a `kuroext` block. Listed, not installable.
    ApkOnly,
}

/// Identity of the publisher key that signed a package: lowercase hex SHA-256 of the 32-byte ed25519 public key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SignerDigest(pub String);

#[derive(Debug, Clone)]
pub struct AnimeSourceRef {
    pub id: i64,            // Aniyomi source ids are 64-bit; never f64
    pub name: String,
    pub lang: String,
    pub base_url: String,   // store `homeUrl`, or legacy `baseUrl`
}

#[derive(Debug, Clone)]
pub struct StoreRef {
    pub index_url: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Installed {
    pub name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    pub abi: AbiVersion,
    pub lang: String,           // "", one language, or "all"
    pub nsfw: bool,
    pub torrent: bool,
    pub sources: Vec<AnimeSourceRef>,
    pub icon: Option<std::path::PathBuf>,
    pub signer: SignerDigest,
    pub has_update: bool,
    pub is_obsolete: bool,
    pub store: Option<StoreRef>,
}

#[derive(Debug, Clone)]
pub struct Available {
    pub name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    /// `Some` for `Kind::Wasm` (from `kuroext`), `None` for `Kind::ApkOnly`.
    pub abi: Option<AbiVersion>,
    /// Aniyomi `extensionLib` value (14.0, 16.0, 17.0, ...). `Some` for `Kind::ApkOnly`.
    /// Parsed with `str::parse::<f64>`; a parse error fails the whole store (WIRE:110).
    pub aniyomi_lib: Option<f64>,
    pub lang: String,
    pub nsfw: bool,
    pub torrent: bool,
    pub sources: Vec<AnimeSourceRef>,
    pub kind: Kind,
    pub apk_url: String,       // always present in the store; not downloaded on Linux
    pub icon_url: String,
    pub kuroext: Option<KuroextRef>,
    pub store: StoreRef,
}

#[derive(Debug, Clone)]
pub struct KuroextRef {
    pub url: String,           // absolute after resolution
    pub sha256: [u8; 32],
    pub ed25519: [u8; 64],     // signature over the raw 32-byte digest
}

#[derive(Debug, Clone)]
pub struct Untrusted {
    pub name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    pub abi: AbiVersion,
    pub signer: SignerDigest,  // Aniyomi: signatures.last()
}

/// Result of loading one installed package. Mirrors AnimeLoadResult.
#[derive(Debug, Clone)]
pub enum LoadResult {
    Success(Installed),
    Untrusted(Untrusted),
    Error(LoadError),
}

/// Install step, same order and same completion rule as InstallStep.kt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep { Idle, Pending, Downloading, Installing, Installed, Error }

impl InstallStep {
    pub fn is_completed(self) -> bool {
        matches!(self, InstallStep::Idle | InstallStep::Installed | InstallStep::Error)
    }
}

/// Bucket computed for the Browse list. Mirrors AnimeExtensions.kt.
#[derive(Debug, Clone, Default)]
pub struct Buckets {
    pub updates: Vec<Installed>,
    pub installed: Vec<Installed>,
    pub available: Vec<AvailableRow>,
    pub untrusted: Vec<Untrusted>,
}

#[derive(Debug, Clone)]
pub struct AvailableRow {
    pub ext: Available,        // the extension as listed
    pub source: Option<AnimeSourceRef>, // Some for split rows (one per enabled-language source)
    pub row_key: String,       // display key, "<pkg>-<sourceId>" for split rows, pkg otherwise
}
```

`AbiVersion`, `LoadError`, and `SignerDigest` are defined in their modules:

```rust
// abi.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiVersion { pub major: u32, pub minor: u32 }

impl AbiVersion {
    /// Tuple comparison. "1.10" is minor 10, never 1.1 (fixes the Double quirk of Aniyomi).
    /// Accepts exactly "<u32>.<u32>": one dot, both sides ASCII digits, no sign, no spaces.
    pub fn parse(s: &str) -> Option<AbiVersion> {
        let (a, b) = s.split_once('.')?;
        if b.contains('.') { return None; }
        Some(AbiVersion { major: a.parse().ok()?, minor: b.parse().ok()? })
    }
}
```

```rust
// model.rs (continued)
#[derive(Debug, Clone, thiserror::Error)]
pub enum LoadError {
    #[error("package not found: {0}")]
    NotFound(String),
    #[error("missing version name for {0}")]
    MissingVersionName(String),
    #[error("unsupported abi {0}")]
    UnsupportedAbi(String),
    #[error("package is not signed: {0}")]
    NotSigned(String),
    #[error("nsfw package not allowed: {0}")]
    NsfwNotAllowed(String),
    #[error("manifest invalid: {0}")]
    Manifest(String),
    #[error("load failed for {pkg}: {reason}")]
    Load { pkg: String, reason: String },
}
```

`LoadError` variants carry the exact log text of the Kotlin code (behavior section 2.7). The log text uses the same wording with the Rust names substituted.

---

## 3. Store index client (`src/extensions/anime/store/`)

### 3.1 Wire structs (`wire.rs`)

JSON (serde). Missing nullable fields decode to `None` (matches `explicitNulls = false`). Unknown fields are ignored (matches `ignoreUnknownKeys = true`). Required fields have no `#[serde(default)]`.

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StoreJson {
    pub name: String,
    #[serde(rename = "badgeLabel")]
    pub badge_label: String,
    #[serde(rename = "signingKey")]
    pub signing_key: String,
    pub contact: ContactJson,
    #[serde(rename = "extensionList", default)]
    pub extension_list: Option<ExtensionListJson>,
    #[serde(rename = "extensionListUrl", default)]
    pub extension_list_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ContactJson {
    pub website: String,
    #[serde(default)]
    pub discord: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtensionListJson {
    pub extensions: Vec<ExtensionJson>,
}

#[derive(Debug, Deserialize)]
pub struct ExtensionJson {
    pub name: String,
    #[serde(rename = "packageName")]
    pub package_name: String,
    pub resources: ResourcesJson,
    #[serde(rename = "extensionLib")]
    pub extension_lib: String,
    #[serde(rename = "versionCode")]
    pub version_code: i64,
    #[serde(rename = "versionName")]
    pub version_name: String,
    #[serde(rename = "contentWarning")]
    pub content_warning: ContentWarning,
    #[serde(rename = "isTorrent", default)]
    pub is_torrent: bool,
    pub sources: Vec<SourceJson>,
}

#[derive(Debug, Deserialize)]
pub struct ResourcesJson {
    #[serde(rename = "apkUrl")]
    pub apk_url: String,
    #[serde(rename = "iconUrl")]
    pub icon_url: String,
    /// Kurobayomi addition. Aniyomi ignores it.
    #[serde(default)]
    pub kuroext: Option<KuroextJson>,
}

#[derive(Debug, Deserialize)]
pub struct KuroextJson {
    pub url: String,
    pub sha256: String,
    pub ed25519: String,
}

#[derive(Debug, Deserialize)]
pub struct SourceJson {
    pub id: i64,
    pub name: String,
    pub language: String,
    #[serde(rename = "homeUrl", default)]
    pub home_url: String,
    #[serde(rename = "mirrorUrls", default)]
    pub mirror_urls: Vec<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// Accepts the short name ("SAFE") and the long name ("CONTENT_WARNING_SAFE").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum ContentWarning {
    Unspecified = 0,
    Safe = 1,
    Mixed = 2,
    Nsfw = 3,
}

impl<'de> serde::Deserialize<'de> for ContentWarning {
    /// Accepts "UNSPECIFIED"/"SAFE"/"MIXED"/"NSFW" and "CONTENT_WARNING_<NAME>" (Kotlin @JsonNames).
    /// Any other value is an error, which fails the whole store (same as kotlinx).
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        let name = raw.strip_prefix("CONTENT_WARNING_").unwrap_or(&raw);
        match name {
            "UNSPECIFIED" => Ok(ContentWarning::Unspecified),
            "SAFE" => Ok(ContentWarning::Safe),
            "MIXED" => Ok(ContentWarning::Mixed),
            "NSFW" => Ok(ContentWarning::Nsfw),
            other => Err(serde::de::Error::custom(format!("unknown content warning {other}"))),
        }
    }
}

/// Legacy repo document (`repo.json`).
#[derive(Debug, Deserialize)]
pub struct LegacyRepoJson {
    #[serde(rename = "index_v2", default)]
    pub index_v2: Option<String>,
    pub meta: LegacyMeta,
}

#[derive(Debug, Deserialize)]
pub struct LegacyMeta {
    pub name: String,
    #[serde(rename = "shortName", default)]
    pub short_name: Option<String>,
    pub website: String,
    #[serde(rename = "signingKeyFingerprint")]
    pub signing_key_fingerprint: String,
}

/// Legacy list entry (`index.min.json` array).
#[derive(Debug, Deserialize)]
pub struct LegacyExtJson {
    pub name: String,
    pub pkg: String,
    pub apk: String,
    pub lang: String,
    pub code: i64,
    pub version: String,
    pub nsfw: i32,
    #[serde(default)]
    pub sources: Option<Vec<LegacySourceJson>>,
}

#[derive(Debug, Deserialize)]
pub struct LegacySourceJson {
    pub id: i64,
    pub lang: String,
    pub name: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
}
```

Protobuf (prost). The protobuf messages mirror the Kotlin `@ProtoNumber` tags exactly. Field names are Rust-only.

```rust
// wire.rs (protobuf part)
#[derive(Clone, PartialEq, prost::Message)]
pub struct PbStore {
    #[prost(string, tag = "1")] pub name: String,
    #[prost(string, tag = "2")] pub badge_label: String,
    #[prost(string, tag = "3")] pub signing_key: String,
    #[prost(message, required, tag = "4")] pub contact: PbContact,
    #[prost(message, optional, tag = "101")] pub extension_list: Option<PbExtensionList>,
    #[prost(string, optional, tag = "102")] pub extension_list_url: Option<String>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct PbContact {
    #[prost(string, tag = "1")] pub website: String,
    #[prost(string, optional, tag = "2")] pub discord: Option<String>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct PbExtensionList {
    #[prost(message, repeated, tag = "1")] pub extensions: Vec<PbExtension>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct PbExtension {
    #[prost(string, tag = "1")] pub name: String,
    #[prost(string, tag = "2")] pub package_name: String,
    #[prost(message, required, tag = "3")] pub resources: PbResources,
    #[prost(string, tag = "4")] pub extension_lib: String,
    #[prost(int64, tag = "5")] pub version_code: i64,
    #[prost(string, tag = "6")] pub version_name: String,
    #[prost(enumeration = "ContentWarning", tag = "7")] pub content_warning: i32,
    #[prost(bool, tag = "8")] pub is_torrent: bool,
    #[prost(message, repeated, tag = "9")] pub sources: Vec<PbSource>,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct PbResources {
    #[prost(string, tag = "1")] pub apk_url: String,
    #[prost(string, tag = "2")] pub icon_url: String,
}
#[derive(Clone, PartialEq, prost::Message)]
pub struct PbSource {
    #[prost(int64, tag = "1")] pub id: i64,
    #[prost(string, tag = "2")] pub name: String,
    #[prost(string, tag = "3")] pub language: String,
    #[prost(string, tag = "4")] pub home_url: String,
    #[prost(string, repeated, tag = "5")] pub mirror_urls: Vec<String>,
    #[prost(string, optional, tag = "6")] pub message: Option<String>,
}
```

Protobuf notes. Kotlin `Boolean` and `Long` are varints, `enum` is a varint with the `@ProtoNumber` value. `prost` `enumeration` on `ContentWarning` must map to the same numbers 0..3. A protobuf value outside 0..3 is a decode error in Kotlin. In Rust, `ContentWarning::try_from(i32)` returns `Err`, and the whole store fails (same as Kotlin).

Kotlin `Source.homeUrl` defaults to `""` and `mirrorUrls` defaults to empty. Protobuf decode of an absent field uses the same default. Rust `prost` default for `String` is `""`, which matches.

### 3.2 Fetch and decode algorithm (`store/mod.rs`, `store/decode.rs`)

```rust
pub struct IndexFetcher { http: reqwest::Client, /* no retries */ }

pub enum StoreDoc {
    Store(StoreJson),                 // JSON {...} that decoded as store
    StorePb(PbStore),                 // protobuf
    Legacy(LegacyRepoJson),           // repo.json shape
    LegacyList(Vec<LegacyExtJson>),   // index.min.json array shape
}

impl IndexFetcher {
    /// Mirrors AnimeExtensionStoreService.fetch (SSVC:28-68).
    pub async fn fetch_store(&self, index_url: &str) -> Result<FetchedStore, StoreError>;
    /// Mirrors getExtensions (SSVC:70-109).
    pub async fn fetch_extensions(&self, store: &StoreRow) -> Result<Vec<Available>, StoreError>;
}
```

`fetch_store` steps (exact):
1. `GET index_url`. Non-2xx → `StoreError::Http(status)`.
2. Read the body into memory (bounded: 16 MiB, see 3.4). If the first two bytes are `0x1f 0x8b`, gunzip with `flate2::read::GzDecoder`.
3. First byte:
   - `b'['` (`0x5B`): if `index_url` does not end with `/index.min.json`, return `StoreError::Invalid("Provided legacy store url is not valid")`. Otherwise `repo_url = index_url.replace("/index.min.json", "/repo.json")`, `GET repo_url` (no gunzip, matching SSVC:40-42), parse as `LegacyRepoJson`. The stored URL becomes `repo_url`.
   - `b'{'` (`0x7B`): try `LegacyRepoJson` first. On failure, try `StoreJson`. Both fail → `StoreError::Decode`.
   - anything else: `PbStore::decode`.
4. If the result is `LegacyRepoJson` with `index_v2 = Some(next)`: follow the redirect by calling `fetch_store(next)` with `depth + 1`. A redirect is followed only while `depth < 4`, so at most four redirects are followed. A redirect from a document at `depth == 4` returns `StoreError::RedirectLoop`. **Deviation from Aniyomi:** Aniyomi has no limit and overflows the stack on a cycle (behavior 7.1 step 4).
5. Map to `StoreRow` (the stored shape): `index_url` = final URL, `name`, `badge_label`, `signing_key`, `contact_website`, `contact_discord`, `is_legacy`, `extension_list_url`.
   - For `StoreJson`, `is_legacy = false`.
   - For `LegacyRepoJson`, `is_legacy = true`, `extension_list_url = None`, `badge_label = short_name.unwrap_or(name)`, `signing_key = signing_key_fingerprint`.

`fetch_extensions` steps (exact):
1. If `store.extension_list_url` is `Some(u)`: `GET u`. Gunzip if needed. First byte `b'{'` → `ExtensionListJson`, else `PbExtensionList`. Map (3.3).
2. Else if `!store.is_legacy`: `GET store.index_url`. Gunzip if needed. First byte `{` → `StoreJson`, else `PbStore`. `extension_list` is `None` → `StoreError::Invalid("extensionList is missing")` (Kotlin throws NPE on `!!`, the same failure).
3. Else (legacy): `base = store.index_url.trim_end_matches("/repo.json")`, `GET format!("{base}/index.min.json")`. **No gunzip.** Parse `Vec<LegacyExtJson>`. Map each with `base` (3.3).

Cancellation: all `await` points are cancel-safe. A cancelled fetch returns `StoreError::Cancelled` and never a partial result (see section 7.5 for the manager's rule).

### 3.3 Mapping to domain (`store/map.rs`)

Exact rules, from behavior 7.5 and 7.7:

| Output field | Rule (Aniyomi source) |
|---|---|
| `pkg` | `packageName` (WIRE:107) |
| `apk_url` | `resources.apkUrl` (WIRE:108). Legacy: `"{base}/apk/{apk}"` (LEG:32) |
| `icon_url` | `resources.iconUrl` (WIRE:109). Legacy: `"{base}/icon/{pkg}.png"` (LEG:33) |
| `abi` (Aniyomi lib) | `AbiVersion` cannot hold `14.0`. For Aniyomi entries, `abi` is not used. Store `extensionLib` as `aniyomi_lib: Option<f64>` in the `Available` struct, parsed with `str::parse::<f64>` and **a parse error fails the whole store** (WIRE:110 `toDouble()`). Legacy: `version.rsplit_once('.')` prefix, `toDouble()` (LEG:34) |
| `version_code` | `versionCode` (WIRE:113 uses `extension.versionCode`) |
| `version_name` | `versionName` |
| `lang` | `sources.map(language).distinct()`. Exactly one → that language. Otherwise `"all"` (WIRE:113). Zero sources → `"all"` |
| `nsfw` | `content_warning >= ContentWarning::Mixed` (WIRE:114) |
| `torrent` | `isTorrent` (WIRE:115) |
| `sources` | `AnimeSourceRef { id, name, lang: language, base_url: homeUrl }` (WIRE:116-123) |
| `kind` | `Wasm` iff `resources.kuroext` is `Some` and every field is valid (3.5); `Unsigned` or `ApkOnly` as described in 3.5 |
| legacy `nsfw` | `nsfw == 1` (LEG:38). Legacy `torrent` = false (LEG:39) |
| legacy `sources` | empty or absent → one synthetic `AnimeSourceRef { id: 0, name, lang, base_url: "" }` (LEG:40-48) |
| legacy `lang` | the `lang` field as is (LEG:37) |

Legacy `name` is `name.strip_prefix("Aniyomi: ").unwrap_or(&name)`. This is `substringAfter` semantics: the whole string when the prefix is missing (LEG:30).

Store-level `name` (`Available.name` for a non-legacy entry) is `extension.name` as is (WIRE:106). The `"Aniyomi: "` prefix is stripped only in the legacy path. This is an inconsistency in Aniyomi (behavior 7.7). Kurobayomi strips the prefix in both paths for display, and keeps the original in `raw_name` for debugging (Proposal).

### 3.4 Limits (Proposal)
- Index and list bodies: at most 16 MiB after gunzip. A larger body is `StoreError::TooLarge`. Aniyomi has no limit.
- Gunzip output: at most 64 MiB. Protects against zip bombs.
- Redirect depth: at most 4 (3.2 step 4).
- Connection and read timeout: 30 s per request (reqwest client, area:12-network-stack). Aniyomi uses the OkHttp defaults.

### 3.5 `kuroext` block validation (Proposal, part of `map.rs`)
- `sha256` must be exactly 64 lowercase hex characters. Decode to `[u8; 32]`.
- `ed25519` must be standard base64 with padding, decoding to exactly 64 bytes.
- `url` must parse as `http` or `https`. A relative `url` is resolved against `index_url` with the `url` crate.
- A `kuroext` block whose `sha256` or `url` is malformed makes the entry `ApkOnly` with a logged warning. A block whose `ed25519` or `publicKey` is missing or malformed makes it `Unsigned` (listed, not installable; area:18-extensions-manga section 16.2 calls the same case `Untrusted`, see T-19a-013). The entry is never dropped.

---

## 4. Package container and verification

### 4.1 Manifest (`package/manifest.rs`)
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]           // unknown keys are an error here, unlike the store
pub struct Manifest {
    pub schema: u32,                     // must be 1
    pub kind: String,                    // must be "anime"
    pub pkg: String,
    pub name: String,
    #[serde(rename = "versionName")] pub version_name: String,
    #[serde(rename = "versionCode")] pub version_code: i64,
    pub abi: String,
    pub lang: String,
    pub nsfw: bool,
    #[serde(default)] pub torrent: bool,
    pub sources: Vec<ManifestSource>,
    #[serde(default)] pub icon: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestSource { pub id: i64, pub lang: String, pub name: String, #[serde(rename = "baseUrl")] pub base_url: String }
```

Validation (all required, in this order; first failure wins):
1. `schema == 1`.
2. `kind == "anime"`.
3. `pkg` is 1 to 255 bytes of `[a-z0-9_.]`.
4. `name` trimmed is 1 to 128 Unicode scalar values and does not start with `"Aniyomi: "`.
5. `version_name` matches `^\d+\.\d+\.\d+$`.
6. `version_code` is in `0..=i64::MAX` (serde already enforces the type).
7. `abi` parses with `AbiVersion::parse` and the major and minor of `abi` equal the `version_name` prefix (`"1.0"` against `"1.0.7"`).
8. `abi` is in `SUPPORTED_ABI` (section 4.5).
9. `lang` is 2 to 8 ASCII characters `[A-Za-z-]` or `"all"`.
10. Every `sources[].id` is unique. Every `baseUrl` parses as absolute `http` or `https`.
11. `icon`, when present, is a relative path inside the archive, and the file exists.

Archive entries (`zip` crate): `manifest.json` (required), `source.wasm` (required), `icon.png` (optional, ≤ 262144 bytes). Any other entry is ignored. Paths with `..` or absolute paths are rejected (zip-slip).

### 4.2 Verification (`package/verify.rs`)
```rust
pub struct Verified { pub signer: SignerDigest, pub sha256: [u8; 32] }

/// Steps from behavior/data 8.2 and area:18-extensions-manga section 7.4.
pub fn verify_download(path: &Path, expect: &KuroextRef, store_signing_key: &str) -> Result<Verified, VerifyError>;
```
1. Stream the file through SHA-256. Compare with `expect.sha256`. Mismatch → `VerifyError::DigestMismatch`.
2. The public key comes from the index entry: `resources.kuroext.publicKey` (standard base64, exactly 32 bytes; data section 8.2). Area:18 section 7.4 does not carry a key, so this field is a required addition to the shared format (recorded in T-19a-012).
   - `key_bytes = base64_decode(publicKey)`. Length not 32 → `VerifyError::BadKey`.
   - `digest = hex(sha256(key_bytes))`. `digest != store_signing_key` → `VerifyError::UntrustedSigner(digest)`. The key is not this store's key, so the package is refused before any signature check.
   - `VerifyingKey::from_bytes(key_bytes)` (failure → `BadKey`). Then `verify_strict(&digest_bytes, &Signature::from_bytes(&expect.ed25519))`. Failure → `BadSignature`.
3. `signer = SignerDigest(digest)`.

```rust
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("sha256 mismatch")] DigestMismatch,
    #[error("signature does not verify")] BadSignature,
    #[error("signer {0} is not the store signer")] UntrustedSigner(String),
    #[error("public key missing or malformed")] BadKey,
    #[error("io: {0}")] Io(#[from] std::io::Error),
}
```

### 4.3 Trust decision (`trust.rs`)
```rust
pub struct TrustStore { /* reads state.toml key trusted_extensions */ }
impl TrustStore {
    /// Repo rule OR per-version rule (behavior 6.2).
    pub fn is_trusted(&self, pkg: &str, version_code: i64, signers: &[SignerDigest], stores: &[StoreRow]) -> bool;
    pub fn trust(&mut self, pkg: &str, version_code: i64, signer: &SignerDigest);  // replaces older entries for pkg
    pub fn revoke_all(&mut self);
}
```
- Repo rule: any `StoreRow.signing_key` equals any `signers[i].0` (exact string compare, lowercase).
- Per-version rule: the entry `"<pkg>:<version_code>:<last signer>"` is in the set.
- `signers` for a `.kuroext` package is always one element (the verifier's signer). The list type is kept so that the rule matches Aniyomi's "any" semantics.

### 4.4 Lib and ABI gate (`abi.rs`)
- Kotlin accepts `14.0`, `16.0`, `17.0` (Aniyomi APK). Those are listed as `ApkOnly` and never loaded. The gate does not apply to them.
- Kurobayomi `kind = "anime"` packages: `abi` must be in `SUPPORTED_ABI`.

### 4.5 Supported ABI (`abi.rs`)
```rust
pub const SUPPORTED_ABI: &[AbiVersion] = &[AbiVersion { major: 1, minor: 0 }];
```
Pinned by T-19a-002 against area:16-source-api-anime's ABI freeze. Changing the set is a one-line edit plus a fixture test.

---

## 5. Loader (`loader.rs`)

```rust
pub struct PackageDir { pub root: PathBuf }   // $XDG_DATA_HOME/kurobayomi/extensions/anime

impl PackageDir {
    /// Lists `<pkg>.kuroext` regular files. Makes them read-only. Parses each one.
    pub fn scan(&self) -> Vec<(String, PathBuf)>;
}

/// Mirrors AnimeExtensionLoader.loadExtension steps 4 to 13 (behavior 2.7).
pub fn load_package(path: &Path, trust: &TrustStore, stores: &[StoreRow], nsfw_allowed: bool) -> LoadResult;
```

Load order (behavior 2.7, unchanged except where noted):
1. Open the zip and read `manifest.json`. Failure → `LoadError::Manifest`.
2. Validate (4.1). Version name empty → `MissingVersionName`. Unsupported ABI → `UnsupportedAbi`. These are the Kotlin steps 4 and 5.
3. Signer: verify again? **No.** The installed file was verified at install time (section 6.4). Loading re-computes the signer digest from the stored `signer.json` sidecar written by the installer (Proposal). A missing sidecar → `LoadError::NotSigned`.
4. Trust (4.3). Not trusted → `LoadResult::Untrusted`.
5. NSFW: `manifest.nsfw && !nsfw_allowed` → `LoadError::NsfwNotAllowed`. **Deviation:** Aniyomi checks `show_nsfw_source` once per process (LDR:36-38). Kurobayomi reads the setting on every `scan` (behavior 2.1).
6. Build `Installed` with `sources` from the manifest. `sources` are **not instantiated** (section 7.3).
7. `lang`: `manifest.lang` as given. The Aniyomi derivation from sources is replaced by the manifest value because sources are not instantiated. Proposal: the installer checks that `manifest.lang` equals the derivation of the manifest's sources, and rejects the package when they differ.

`scan` is synchronous and runs on the worker runtime. The main thread receives `Vec<LoadResult>` through a channel (section 10).

Deviation from the Kotlin lib behavior (recorded): Aniyomi's loader for an APK fails the whole batch on a `NullPointerException` (behavior 2.9 step 1). In Kurobayomi each package is loaded in its own `Result`. A bad package never affects another.

---

## 6. Install pipeline and queue (`install/`)

### 6.1 Pipeline (per entry)
State machine. Each transition emits `Event::Step { pkg, step }` to the manager.

```
Idle --start--> Pending --GET starts--> Downloading --body complete--> Installing
Installing --verify ok and placed--> Installed
any --verify fails / placed fails / I/O error--> Error
Downloading or Pending --cancel--> Idle (file removed, nothing placed)
Installing --cancel--> Idle only if not yet placed; otherwise the placement finishes and the entry is Installed
```

Differences from Aniyomi, all intentional (behavior 8.10):
- `Installing` is set for the verify and place phase. Aniyomi's private path never sets it.
- Cancel always produces `Idle`. Aniyomi leaves the flow open in one case (behavior 8.6).
- No DownloadManager. A single reqwest stream writes `<pkg>-<versionCode>.kuroext.part` under the cache dir (section 5.2 of data).

### 6.2 Queue
- One `InstallService` per process. A `VecDeque<Entry>` owned by the manager (main thread) holds the pending entries. One tokio task runs at a time; it receives the next entry through an `async_channel`.
- Entries run in FIFO order. Downloads run in parallel (Aniyomi downloads are not queued, behavior 9). The **place** step is serialised by a `tokio::sync::Mutex<()>` so two installs never write the same directory.
- `Entry { pkg: String, version_code: i64, kuroext: KuroextRef, store: StoreRow, cancel: CancellationToken }`.
- Cancel: `cancel.cancel()`. The task returns `Idle`. A queued entry that is cancelled before it starts returns `Idle` without downloading.
- Shutdown (`onDestroy` analogue): every queued entry gets `Error`. The active entry is cancelled and gets `Idle`. (Aniyomi leaves the active entry's step alone, behavior 9. Kurobayomi fixes that.)

### 6.3 Download (`download.rs`)
```rust
pub async fn download(
    http: &reqwest::Client,
    url: &str,
    dest_part: &Path,
    cancel: &CancellationToken,
    progress: &async_channel::Sender<u64>,   // bytes so far
) -> Result<(), DownloadError>;
```
- Streams with `bytes_stream()`. Writes to `dest_part`. Sends progress at most every 250 ms.
- Checks `cancel` between chunks. On cancel, deletes `dest_part` and returns `Cancelled`.
- Non-2xx → `DownloadError::Http`. The partial file is deleted.
- Size cap: 64 MiB (Proposal; extension packages are small). Over the cap → `DownloadError::TooLarge`.

### 6.4 Place (`place.rs`)
Runs after verification (4.2). Mirrors `installPrivateExtensionFile` (behavior 3):
1. Open the package and read `manifest.json`. Validate (4.1). Pkg must equal the store entry's pkg (`PkgMismatch`).
2. `current` = the installed package for the same pkg, if any (read its sidecar signer and version).
3. If `current` exists:
   - a. `new.version_code < current.version_code` → `DowngradeRefused`. Equal is allowed.
   - b. Signer continuity: `new.signer == current.signer` is required. (Aniyomi requires `containsAll`, which for one signer per package is equality. Behavior 3.3 step 3c.) Otherwise `SignerChanged`.
4. Write `<pkg>.kuroext.part` next to the target, `fsync`, `rename` to `<pkg>.kuroext`, set mode `0444`, write `<pkg>.signer` (the signer digest, plain text), `fsync` the directory.
5. Return `Installed`.

Errors are logged with the same texts as Aniyomi: `"Installed extension version is higher. Downgrading is not allowed."`, `"Installed extension signature is not matched."`. For a missing signer on a new install, Aniyomi logs `"Extension to be installed is not signed."`.

### 6.5 Uninstall (`uninstall.rs`)
Deletes `<pkg>.kuroext` and `<pkg>.signer`. Emits `Event::Removed { pkg }`. The manager removes the entry from its maps (behavior 5.11). There is no system uninstall.

### 6.6 Cancel and the active entry
`cancel_install(pkg)` looks up the entry by pkg (not by row key, which is a split key in the UI, behavior 13.1). It cancels the token. The task emits `Idle`. The manager removes the download state. This fixes the hang in behavior 8.6.

---

## 7. Manager (`manager.rs`)

### 7.1 State (main thread only)
```rust
pub struct AnimeExtensions {
    inner: Rc<RefCell<State>>,
    tx: async_channel::Sender<Cmd>,   // to the worker runtime
}

struct State {
    installed: BTreeMap<String, Installed>,      // key pkg
    untrusted: BTreeMap<String, Untrusted>,
    available: BTreeMap<String, Available>,      // last wins (behavior 5.5 step 3)
    available_order: Vec<String>,                // deterministic order: store order, then index order
    stores: Vec<StoreRow>,                       // mirror of table (rowid order)
    steps: HashMap<String, InstallStep>,         // key pkg (not split row key)
    source_stub_index: HashMap<i64, AnimeSourceRef>,   // for stub display
    update_count: usize,                          // derived, not persisted
    initialized: bool,
}
```

Notes:
- The maps are `BTreeMap` for deterministic iteration. Aniyomi uses insertion-ordered `Map`, which is unstable after updates.
- `available` keeps the last match, like `associateBy` (behavior 5.5). Reconciliation uses the first match (behavior 5.7). Kurobayomi picks **first match** in both places and documents it (Proposal, resolves the inconsistency).
- `update_count` replaces `animeext_updates_count`. It is recomputed after every state change.

### 7.2 Events (worker to main thread)
```rust
pub enum Event {
    Loaded(Vec<LoadResult>),            // startup scan finished
    StoresChanged(Vec<StoreRow>),
    AvailableRefreshed(Result<Vec<Available>, String>),  // Err carries a user-safe message
    Step { pkg: String, step: InstallStep },
    Installed { pkg: String, version_code: i64 },
    Removed { pkg: String },
    Trusted { pkg: String, result: LoadResult },
    UpdatesFound(Vec<String>),          // names, for notification
}
```
The main thread consumes events with:
```rust
glib::MainContext::default().spawn_local(async move {
    while let Ok(ev) = rx.recv().await { manager.apply(ev); }
});
```

### 7.3 Sources are not instantiated at discovery
Aniyomi builds `AnimeSource` objects during load (behavior 2.9). Kurobayomi keeps only `AnimeSourceRef` values from the manifest. A WASM instance is created on first use by the source runtime (area:16-source-api-anime and area:18-extensions-manga). `getSourceData(id)` (behavior 5.4) returns a stub `AnimeSourceRef` for listed-but-not-installed sources. Calling a fetch on a stub returns `SourceError::NotInstalled`, the same as `AnimeSourceNotInstalledException`.

### 7.4 Reconciliation (`apply_available`)
Runs after an `AvailableRefreshed(Ok(list))`. Exact rules (behavior 5.7, with the sticky-obsolete fix):
1. If the list is empty, set `update_count = 0`, return. No installed-map change (matches Aniyomi).
2. For each installed `pkg`:
   - `hit = list.iter().find(|a| a.pkg == pkg)` (first match).
   - `hit == None` → `is_obsolete = true`, `has_update = false`, `store = previous store`.
   - `hit == Some(a)` → `is_obsolete = false` (**fix**), `has_update = a.version_code > installed.version_code || a.abi.is_some_and(|x| x > installed.abi)` (an `ApkOnly` entry has `abi = None` and never sets `has_update` through the abi branch), `store = Some(a.store.clone())`.
3. Recompute `update_count = installed.values().filter(|i| i.has_update).count()`. Behavior 5.12 counts obsolete updates too. Kurobayomi excludes obsolete ones (Proposal). When the count is 0, withdraw the update notification.

The `has_update` rule uses the abi tuple comparison. Aniyomi uses `libVersion > libVersion` (Double). For Kurobayomi `Wasm` packages, `abi` is a tuple (4.4).

### 7.5 Refresh rules
- `refresh_all_and_list()` is the only refresh path. It (a) refreshes every store row (section 8.2), then (b) fetches every store's extension list. Both steps happen in one job. (Aniyomi does the store refresh and the list fetch in two separate network passes, behavior 11.1 step 2-3. Kurobayomi uses the store documents fetched in step (a) as the list source when `extension_list_url` is `None`, so the second pass is avoided. Proposal.)
- A cancelled job never writes partial data. The worker returns `Err(Cancelled)` and the main thread ignores the result. Aniyomi returns an empty list on cancellation and wipes the available map (behavior 7.8). This is fixed.
- Per-store failures are returned as `Vec<StoreFailure { store, message }>` alongside the list, so the UI can show them. Aniyomi only logs (behavior 7.8).

### 7.6 Browse buckets (`browse.rs`)
Exact rules from behavior 13.1, with two fixes:

```rust
pub fn buckets(state: &State, enabled_langs: &BTreeSet<String>, show_nsfw: bool) -> Buckets;
```
1. Installed: filter `show_nsfw || !nsfw`. Sort with `sort_by_key(|i| (!i.is_obsolete, i.name.to_lowercase()))`. Because `false < true`, obsolete entries sort first (behavior 13.1). Partition: `has_update` → `updates`, else `installed`.
2. Untrusted: sort by `name.to_lowercase()`. No NSFW filter (Aniyomi behavior, kept).
3. Available: filter `!installed.contains(pkg) && !untrusted.contains(pkg) && (show_nsfw || !nsfw)`. **Fix:** an extension with zero sources is kept as one row with `source = None` and `row_key = pkg` (Aniyomi drops it, behavior 13.1). For each source with `lang ∈ enabled_langs`, one `AvailableRow { source: Some(src), row_key: "<pkg>-<id>" }`.
4. Sort available by `name.to_lowercase()`. The `name` of a split row is the **source** name (behavior 13.1 step 2 copies `name = it.name`).
5. `Kind::ApkOnly` rows are included, with install disabled. A preference `show_not_runnable` (default false) hides them from the list. Counts are shown in the store page instead. (Proposal.)

### 7.7 Languages and source toggles
- Languages (`GetAnimeExtensionLanguages`, behavior 13.2): distinct `source.lang` over `available`, sorted by `(!enabled, locale_display_name)`, with `"all"` first. Zero-source extensions contribute nothing (kept as Aniyomi).
- Source toggles (`GetAnimeExtensionSources`, behavior 13.3): `labelAsName` rule as written. Stored in `hidden_anime_catalogues` (default empty set). Toggling writes the set and emits a refresh of the bucket.

### 7.8 Stub lookup
`source_for_id(id: i64) -> Option<SourceHandle>`:
- Installed source with that id → the installed package's source handle.
- Otherwise `Available` source with that id → `SourceHandle::Stub(AnimeSourceRef)`.
- Otherwise `None`.

This replaces `getExtensionPackage` and `getSourceData` (behavior 5.3, 5.4). The Kotlin `!!` NPE is removed: missing packages return `None`.

---

## 8. Update checks, store refresh and notification (`updates.rs`)

### 8.1 Trigger
- On application start, after the startup scan completes: run `refresh_all_and_list()` once (Aniyomi does this in `MainActivity` at start, behavior 11.3).
- Then every 24 hours while the app runs, using a `glib::timeout_add_seconds_local(86_400, ...)`. Proposal: this makes the daily intent of `1.days` real. Aniyomi's gate is dead code (behavior 11.1 step 1).
- The throttle timestamp is stored under key `anime_ext_last_check` (Proposal, not `last_ext_check`, which the manga side also uses).

### 8.2 Store refresh (`store/refresh.rs`)
For every `StoreRow` in rowid order: `fetch_store(row.index_url)`. On success, `upsert` the row with the new fields (the same as Aniyomi's `upsert`, behavior 7.9). On failure, log and continue. The row is left unchanged.

Aniyomi's manager refresh path (behavior 11.1) does this for every store and then fetches every list again. Kurobayomi fetches each store document once (7.5).

### 8.3 Update notification (`updates.rs`)
- Only when `UpdatesFound(names)` is non-empty. The text is built from the plural rule: `"Extension update available"` for one, `"%d extension updates available"` for more (the plural strings in the data file, section 12).
- Body: `names.join(", ")` unless `hide_notification_content` is true.
- Delivered with GIO: `gtk::gio::Notification::new(title)`, `set_body(Some(&body))`, default action opens the Anime Extensions page (`app.open-anime-extensions` action). Id `"anime-ext-updates"` (replaces Aniyomi's `-401` numeric id, which has no meaning in GIO).
- Withdraw with `app.withdraw_notification("anime-ext-updates")` when the count drops to 0.
- Install progress is in the page, not a notification. Replaces the `-402` foreground service (behavior 15).

---

## 9. Android API to Linux mapping

Every Android or Kotlin mechanism in scope, the Linux replacement, and the decision. Decisions use the vocabulary Port, Rewrite, Replace-with-crate, Drop.

| Android or Kotlin mechanism | Used in | Linux replacement | Decision |
|---|---|---|---|
| `PackageManager.getInstalledPackages`, `getPackageArchiveInfo`, `getPackageInfo` | LDR:128-234 | Directory scan of `$XDG_DATA_HOME/kurobayomi/extensions/anime` and zip read of `manifest.json` | Rewrite |
| `PackageInfo.reqFeatures` `tachiyomi.animeextension` | LDR:399-401 | `manifest.json` `kind = "anime"` | Rewrite |
| `ApplicationInfo.metaData` (`aniyomix.*`, `tachiyomi.animeextension.*`) | LDR:249-297 | `manifest.json` fields | Rewrite |
| `PackageInfoCompat.getLongVersionCode` | LDR:77, 252, 385 | `manifest.version_code: i64` | Port |
| `Hash.sha256` (`eu.kanade.tachiyomi.util.lang.Hash`) | LDR:421 | `sha2::Sha256`, `hex::encode` | Replace-with-crate |
| Signing certificates (`signingInfo`, `signatures`, `GET_SIGNING_CERTIFICATES`) | LDR:409-423 | ed25519 public key digest (`SignerDigest`) | Rewrite |
| `ChildFirstPathClassLoader`, `PathClassLoader`, `Class.forName`, `newInstance` | LDR:298-346 | none. WASM instance per source, created on first use | Drop (APK-only) |
| `AnimeSourceFactory.createSources()` | LDR:319, 335 | Manifest `sources[]` lists the sources | Drop (area:16-source-api-anime registry) |
| `ApplicationInfo.loadIcon`, `iconMap` | MGR:100-112, LDR:366 | `icon.png` from the package, cached under `ext-icons` | Replace-with-crate (`gdk-pixbuf` via gtk) |
| `ApplicationInfo.sourceDir`, `fixBasePaths` | LDR:153, 210, 429-436 | not needed | Drop |
| `Context.filesDir/exts` | LDR:63 | `$XDG_DATA_HOME/kurobayomi/extensions/anime` through the `Paths` trait | Port |
| `copyAndSetReadOnlyTo`, `setReadOnly` | LDR:102, 146-149 | `std::fs::copy` to a `.part` file, `fsync`, `rename`, `set_permissions(0o444)` | Rewrite |
| `DownloadManager` (`enqueue`, `query`, `remove`, `getUriForDownloadedFile`) | INST:47-151 | `reqwest` byte stream with a `CancellationToken` | Replace-with-crate |
| `DownloadManager.Query` polling (`delay(1.seconds)`) | INST:128-151 | Stream progress over `async_channel`, no polling | Rewrite |
| `ACTION_DOWNLOAD_COMPLETE` broadcast receiver | INST:262-320 | Completion is a return value from the download future | Drop (replaced by return value) |
| `ContextCompat.registerReceiver`, `LocalBroadcastManager` | RCV:28-30, IQ:150-168, PKGI:107-114 | direct method calls on the manager | Drop |
| `ACTION_PACKAGE_ADDED`, `_REPLACED`, `_REMOVED`, self broadcasts | RCV:24-146 | `Event::Installed`, `Event::Removed` | Drop (replaced by events) |
| `ACTION_INSTALL_PACKAGE` activity with `startActivityForResult` | ACT:18-79 | none | Drop |
| `ACTION_UNINSTALL_PACKAGE` | INST:221-231 | file removal (section 6.5) | Drop |
| `PackageInstaller` session API | PKGI:19-117 | none | Drop |
| Shizuku (`rikka.shizuku.Shizuku`, `pm install-*` shell) | SHZ:21-124 | none | Drop |
| `Service` foreground notification `-402` | SVC:22-88 | in-page progress; no notification | Drop |
| `Notification` `-401`, `NotificationCompat.BigTextStyle` | ExtensionUpdateNotifier.kt | `gio::Notification` (GTK), id `"anime-ext-updates"` | Replace-with-crate (gtk4) |
| `context.toast(...)` | MGR:152 | In-app banner (GTK `Revealer` with a `Label`). libadwaita is not used | Replace-with-crate (gtk4) |
| `context.startActivity(ACTION_VIEW)` (open in browser) | SMODEL/SCREEN `openInBrowser` | `gtk::UriLauncher` (gtk 4.10+) or `gio::AppInfo::launch_default_for_uri` | Replace-with-crate (gtk4) |
| Clipboard `ClipboardManager.setPrimaryClip` | ContextExtensions.kt:39-55 | `gdk::Display::default().clipboard().set_text(...)`, then a banner `copied_to_clipboard` | Replace-with-crate (gtk4) |
| `Context.isPackageInstalled`, `hasMiuiPackageInstaller`, `isShizukuInstalled` | INST:222, ExtensionInstallerPreference | none | Drop |
| `getUriSize` (`UniFile`) | SHZ:51, PKGI:63 | `std::fs::metadata(path)?.len()` | Port |
| `FileProvider` (`getUriCompat`) | INST:316 | direct paths | Drop |
| `BasePreferences.extensionInstaller` | INST:62 | none | Drop |
| `Injekt` / `injectLazy` | MGR:47-48, API:20-24 | constructor injection (`AnimeExtensions::new(deps)`) | Rewrite |
| `kotlinx.serialization.Json` (`ignoreUnknownKeys`, `explicitNulls=false`) | SSVC, WIRE | `serde` with `#[serde(default)]` on nullable fields | Replace-with-crate |
| `kotlinx.serialization.protobuf.ProtoBuf` | SSVC:50, 78, 90 | `prost` with the same field tags (section 3.1) | Replace-with-crate |
| `okio` `gzip()` | SSVC:120 | `flate2::read::GzDecoder` | Replace-with-crate |
| OkHttp via `NetworkHelper` `GET()` and `awaitSuccess()` | SSVC, API | `reqwest` with the shared client (area:12-network-stack) | Replace-with-crate |
| `SQLDelight` `extension_storeQueries` | SREPO, BRESTORE | `rusqlite` (section 7.3 of data) | Replace-with-crate |
| `Preference<T>` store (`getStringSet`, `getBoolean`, `getInt`, `getLong`) | MGR, TRUST, API | `state.toml` in `$XDG_CONFIG_HOME/kurobayomi` through area:02-preferences-system | Port |
| `kotlinx.coroutines` (`async`, `awaitAll`, `supervisorScope`, `runBlocking`, `delay`, `transformWhile`, `merge`) | many | `tokio` tasks, `JoinSet`, `CancellationToken`, `tokio::time::sleep` | Replace-with-crate |
| `StateFlow` / `MutableStateFlow` | MGR:68-75 | `Rc<RefCell<State>>` plus a `Vec<Subscriber>` callback list on the main thread | Rewrite |
| `Voyager` `StateScreenModel` | SMODEL | `Rc<RefCell<StoresPageState>>` plus GTK signals | Rewrite |
| `LocaleHelper.comparator` / `getLocalizedDisplayName` | BYLANG | ICU-free: compare with a small table of ISO 639-1 names, `"all"` first (area:02-preferences-system) | Port |
| `Locale.getDefault().language` | MGR:183, LocaleHelper.kt:73 | `LANG` / `LC_MESSAGES` parsing (area:02-preferences-system) | Port |
| `Instant.now().toEpochMilli()` | API:40 | `std::time::SystemTime::now()` | Port |
| `Compose` UI: `AlertDialog`, `ElevatedCard`, `LazyColumn`, `ExtendedFloatingActionButton` | presentation files | GTK4 `gtk::Window`-based dialogs, `gtk::ListBox`, `gtk::Button` | Rewrite |
| `Voyager` `Navigator.push` (deep link) | MainActivity.kt:553-560 | `app.open` signal from `gio::Application` with `HANDLES_OPEN` flag, handled by `deeplink.rs` | Replace-with-crate (gtk4) |

---

## 10. Threading and runtime

Model: GTK on the main thread, tokio on a worker runtime, `async_channel` between them.

```rust
// main.rs (sketch)
let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
let (tx_ev, rx_ev) = async_channel::unbounded::<extensions::anime::Event>();
let svc = extensions::anime::AnimeExtensions::new(rt.handle().clone(), tx_ev);
glib::MainContext::default().spawn_local(async move {
    while let Ok(ev) = rx_ev.recv().await { svc.apply(ev); }
});
```

Rules:
- `AnimeExtensions` is `!Send` (it holds `Rc`). It lives only on the main thread.
- Background tasks get owned clones of `reqwest::Client`, paths, and `async_channel::Sender<Event>`. They never touch `State`.
- `worker_threads(2)`: enough for two concurrent downloads plus a store fetch. Aniyomi uses an unbounded default pool. The bound is Proposal.
- Blocking work (zip open, sha256 of a 16 MiB file) runs in `tokio::task::spawn_blocking`.
- Store refresh runs with at most 4 concurrent store fetches (`futures::stream::iter(...).buffer_unordered(4)`). Aniyomi runs them all in parallel (behavior 7.8). The limit is Proposal.
- Install placement is serialised by a `tokio::sync::Mutex<()>` (section 6.2).
- The worker never calls GTK. The main thread never blocks on the worker (no `block_on`).

---

## 11. UI integration (Kurobayomi mockup)

### 11.1 Stores page
- Replace the mockup card list `src/settings.rs:13` with a call to `ui::anime_stores::page(&svc)`.
- Header: title `Extension stores`, a refresh button (`view-refresh-symbolic`, tooltip `Refresh`), and an `Add` button (text `Add`) in the header end area. `src/ui.rs` `page()` already builds a header bar (`ui.rs:67`).
- Empty state: `status("network-server-symbolic", "You haven't added an extension store yet")` (`ui.rs:166`).
- List: one `gtk::ListBox` row per store (`ui.rs:147` `row()` with a subtitle).
  - Title: store `name`. Subtitle: `index_url`.
  - Suffix buttons, in order: open website (`web-browser-symbolic`, tooltip `Open in browser`), discord (only when present, tooltip empty, icon `user-available-symbolic`), copy (`edit-copy-symbolic`, tooltip `Copy to clipboard`), delete (`user-trash-symbolic`, tooltip `Delete`).
- The Aniyomi FAB (`Add`) is a header button in GTK (no FAB pattern in this app).

### 11.2 Dialogs
Use `ui.rs` `show_dialog` with `Dialog` specs from `spec/*.json` or direct `gtk::Window` construction. Each dialog below is a `gtk::Window` modal to the main window.

- Create: title `Add extension store`. Entry label `Store index URL`. Entry input purpose URL. Supporting text: `This extension store already exists` when the typed value matches a stored URL, else `*required`, or the error text when set. `Add` disabled while empty, duplicate, or processing. Button text `Processing…` while processing. Cancel.
- Confirm (deep link): title `Add extension store`, body `Do you wish to add the extension store below?`, read-only entry with the URL. Supporting text: duplicate message, error text, or none. `Add` disabled when duplicate or processing.
- Delete: title `Remove extension store`, body `Do you wish to remove the extension store "%s" (%s)?` with name and URL. `OK` and `Cancel`.
- Focus: the Create entry takes focus on open. Aniyomi waits 100 ms (`AnimeExtensionStoreDialogs.kt:97-100`). GTK needs no delay.
- Errors: show the **localised** message, not the raw exception text. Map `StoreError` variants to fixed strings (Proposal, behavior 12.1 shows raw text in Aniyomi). Fixed strings: `Provided legacy store url is not valid` (kept verbatim) and `Failed to add extension store` for the rest.

### 11.3 Untrusted dialog
Title `Untrusted extension`. Body `Malicious extensions can read any stored login credentials or execute arbitrary code.` + blank line + `By trusting this extension, you accept these risks.`. Buttons `Trust` and `Uninstall`. Outside click dismisses (no action).

### 11.4 Deep link
- `gio::Application` with `ApplicationFlags::HANDLES_OPEN` (`gtk::Application::builder().flags(...)`). `connect_open` receives `gio::File` values. For `aniyomi://add-repo?url=https%3A%2F%2Fexample.org%2Frepo.json` and `aniyomi://extension-store?url=https%3A%2F%2Fexample.org%2Frepo.json`, parse with the `url` crate, read the `url` query value, present the Confirm dialog (12.4).
- A `.desktop` file declares `MimeType=x-scheme-handler/aniyomi;` (installation step, area:36-ui-shell-navigation-security-webview).
- `main.rs` currently calls `app.run()`. `app.run_with_args` is not needed.

### 11.5 Mockup impact checklist
- `src/settings.rs:13`: replace the two fixed cards with the live list page.
- `src/screens.rs` (`body(id)` dispatch) and `src/settings.rs:9-27` (`body`) route the id `settings_anime_extension_stores`. The route stays. Only its body changes to the live page.
- `src/spec.rs` (`Spec::load()`) reads `spec/*.json` for the settings rows. The stores page does not use the spec file for store rows, so `spec/settings.json` keeps only the menu entry.
- `src/ui.rs` provides `page()` (header), `row()`, `status()`, and `show_dialog()`. The stores page uses them.
- `src/settings.rs:133-160` `cards()`: keep for other lists; anime stores use a dedicated `ui/anime_stores.rs` so that the `dialog_of` placeholder (`settings.rs:154`) is not reused.
- `spec/settings.json`: the entry `settings_anime_extension_stores` keeps its id. Its item list is replaced with live rows.
- `src/entry.rs:252-253` (browse tabs): the Extensions tab for anime is owned by area:41-ui-browse-anime. This area provides the data only.

---

## 12. Error handling

- Library errors use `thiserror`. Each module has one error enum: `StoreError`, `VerifyError`, `DownloadError`, `PlaceError`, `LoadError` (section 2), `ManifestError`.
- `anyhow` is not used in this area. Errors carry enough data for the UI to map them to fixed strings.
- A panic in a background task is caught by `tokio::task::JoinHandle` and mapped to `Event::Step { step: Error }` for install tasks, and to `AvailableRefreshed(Err)` for refresh. The main thread never panics because of a worker panic.
- User-facing messages come only from the fixed set in section 11.2. Raw error text is logged, never shown.
- Logging uses `tracing` with the Kotlin log texts (data section 11) as the message. Field names: `pkg`, `store`, `version_code`.

---

## 13. Crates

| Crate | Major line | Used for | Why this one |
|---|---|---|---|
| `tokio` | 1 | worker runtime, `spawn_blocking`, `Mutex`, `JoinSet`, timers | Standard; already in plan area:12-network-stack |
| `reqwest` | 0.12 | HTTP GET, streaming downloads | Shared client from area:12-network-stack |
| `serde`, `serde_json` | 1 | store JSON, legacy JSON, manifest | Matches `kotlinx.serialization` JSON |
| `prost` | 0.13 | protobuf store decode | Same field tags, no codegen needed (derive) |
| `flate2` | 1 | gzip for store bodies | Standard pure-Rust gzip reader |
| `zip` | 2 | `.kuroext` archive read | Standard |
| `sha2` | 0.10 | SHA-256 digest | Standard RustCrypto |
| `ed25519-dalek` | 2 | signature verification with `verify_strict` | Standard, no OpenSSL |
| `base64` | 0.22 | decode `ed25519` and `publicKey` | Standard |
| `hex` | 0.4 | lowercase hex for digests | Standard |
| `url` | 2 | URL parsing and resolution | Standard |
| `rusqlite` | current | `extension_store` table | Area:04 owns the connection |
| `async-channel` | 2 | worker to main thread events | No glib dependency needed on the worker |
| `tokio-util` | 0.7 | `CancellationToken` | Standard |
| `thiserror` | 2 | error enums | Standard |
| `tracing` | 0.1 | logs | Standard |
| `gtk4` (gtk crate) | 0.11 | UI, `gio::Notification`, clipboard, `UriLauncher` | Already in `Cargo.toml` |
| `wasmtime` | current | runs `source.wasm` (area:18-extensions-manga, area:20-extension-strategy-redesign) | Decided upstream, not here |

Crates not used: `async-trait` (no trait objects over async), `anyhow` (section 12), `directories` (paths come from the `Paths` trait, area:01-foundation-app-bootstrap and addenda).

---

## 14. Pitfalls

1. **`Rc` in `AnimeExtensions` is `!Send`.** Do not move it into `tokio::spawn`. Only `Sender<Event>` and owned data cross the boundary.
2. **`glib::MainContext::spawn_local` needs the main context thread.** It must run after `gtk::init`, which `gtk::Application::run` does.
3. **Do not block the main thread on store fetches.** A fetch of 20 stores can take tens of seconds. The page shows a spinner from `Event::Loading` (sent at the start of the job).
4. **`prost` and `serde` disagree on defaults for enums.** Protobuf value `0` is valid and `serde` needs the `alias`. Test both with the same fixture (T-19a-068).
5. **`ed25519_dalek::VerifyingKey::from_bytes` accepts small-order points.** Use `verify_strict` only. Reject keys whose digest does not match the store fingerprint before verifying.
6. **Zip slip and zip bombs.** Reject entry names with `..`, absolute paths, or backslashes. Cap the uncompressed size of `source.wasm` at 16 MiB and `manifest.json` at 64 KiB.
7. **Atomic rename across filesystems.** `.part` must be in the same directory as the target, or `rename` fails with `EXDEV`.
8. **Read-only files.** `0444` on the installed file blocks an overwrite by a plain `fs::write`. Replacement uses `rename`, which needs write permission on the directory, not the file.
9. **Split row keys.** The UI row key `"<pkg>-<id>"` never reaches the install layer. Install and step maps use the bare `pkg` (section 6.6).
10. **Tuple ABI comparison.** Never convert the ABI to a float. `"1.10"` is `(1, 10)`.
11. **Redirect loops.** The depth limit in 3.2 step 4 stops `StackOverflow`. A test with a self-redirecting fixture pins it (T-19a-016, T-19a-061).
12. **Store row order.** SQLite `SELECT *` without `ORDER BY` returns rowid order in practice, but the spec does not guarantee it. Add `ORDER BY rowid` in the Rust query so the "first match wins" rule is deterministic.
13. **`bool` and integer columns.** `is_legacy` is stored as `INTEGER`. Map `0` and `1` only. Any other value is a decode error.
14. **Shared `last_ext_check` key.** Use `anime_ext_last_check`. Reading the manga key would make the two throttles interfere.
15. **Clock changes.** The 24 h timer uses a monotonic `Instant` for the schedule and a wall-clock value only for the stored last-check time.

---

## 15. Test hooks

- `store::decode` and `store::map` take bytes and return values. No network. Fixtures in `tests/fixtures/anime_store/` (section 5 of the edge-case file).
- `package::manifest` takes a `&str`. No archive.
- `verify::verify_download` takes a path. A fixture `.kuroext` with a known key.
- `loader::load_package` takes a path and in-memory `TrustStore` and `StoreRow` slices.
- `browse::buckets` takes `State` by reference. Pure function, no GTK.
- `manager` reconciliation is a pure function over `State` and the list.

No GTK is needed for any of the above. GTK is needed only for `ui/*` and is covered by a smoke run (the project's `run` skill).
