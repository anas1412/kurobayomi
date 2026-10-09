# Topic: extension stores (anime) and extension repos (manga) — wire formats, fetch algorithm, persistence, screens

Scope files: `data/src/main/java/mihon/data/extension/anime/{model,repository,service}/*.kt` (6 files), `data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt`. Domain companions read for exactness: `mihon/domain/extension/anime/**`, `mihon/domain/extensionrepo/**`. Citations use the abbreviations of `p1-behavior.md` (`ESS` = `AnimeExtensionStoreService.kt`, `ESR` = `AnimeExtensionStoreRepositoryImpl.kt`, `NES`, `NLE`, `NLR`, `MER`).

The *discovery* half of the extension system lives here (what extensions exist and where from). What an extension IS on Linux (APK/dex cannot run) and how it is installed, verified and loaded is area:20-extension-strategy-redesign; this plan keeps the index formats and the persistence faithful so whichever artifact format area:20 picks can be advertised through the same index (`apkUrl` is just a URL; `signingKey` is just a string).

## 1. Two generations, four document shapes

| Shape | First byte | Where it comes from | Handled as |
|---|---|---|---|
| Legacy index (`index.min.json`): JSON **array** of `NetworkLegacyAnimeExtension` | `[` (0x5B) | User typed `https://host/path/index.min.json` | Not a store itself: the store is described by the sibling `repo.json` |
| Legacy `repo.json`: JSON **object** `{"index_v2"?: url, "meta": {name, shortName?, website, signingKeyFingerprint}}` | `{` (0x7B) | User typed `.../repo.json`, or derived from an `index.min.json` | `NetworkLegacyAnimeExtensionRepo` -> store with `isLegacy = true` |
| New store, JSON object (`NetworkAnimeExtensionStore`) | `{` | User typed a new-format index | store with `isLegacy = false` |
| New store, protobuf (same message) | anything else (typically a low byte such as 0x0A = field 1 length-delimited) | idem | store with `isLegacy = false` |

Each of these may additionally be gzip-compressed *as a payload* (the file itself is a `.gz`, magic bytes `1f 8b`); HTTP `Content-Encoding: gzip` is separate and handled transparently by the HTTP client.

## 2. Wire models

See `p1-data-and-config.md` section 1.5 for the field table. Rust declarations (module `kuro_core::data::extension::store::wire`):

```rust
// Protobuf + JSON in one set of structs. prost derives the wire format from the tags; serde derives JSON.
#[derive(Clone, PartialEq, prost::Message, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WireStore {
    #[prost(string, tag = "1")] pub name: String,
    #[prost(string, tag = "2")] pub badge_label: String,
    #[prost(string, tag = "3")] pub signing_key: String,
    #[prost(message, optional, tag = "4")] pub contact: Option<WireContact>,
    #[prost(message, optional, tag = "101")] pub extension_list: Option<WireExtensionList>,
    #[prost(string, optional, tag = "102")] pub extension_list_url: Option<String>,
}
#[derive(Clone, PartialEq, prost::Message, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WireContact { #[prost(string, tag = "1")] pub website: String, #[prost(string, optional, tag = "2")] pub discord: Option<String> }
#[derive(Clone, PartialEq, prost::Message, serde::Deserialize)]
#[serde(default)]
pub struct WireExtensionList { #[prost(message, repeated, tag = "1")] pub extensions: Vec<WireExtension> }
#[derive(Clone, PartialEq, prost::Message, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WireExtension {
    #[prost(string, tag = "1")] pub name: String,
    #[prost(string, tag = "2")] pub package_name: String,
    #[prost(message, optional, tag = "3")] pub resources: Option<WireResources>,
    #[prost(string, tag = "4")] pub extension_lib: String,
    #[prost(int64, tag = "5")] pub version_code: i64,
    #[prost(string, tag = "6")] pub version_name: String,
    #[prost(enumeration = "ContentWarning", tag = "7")]
    #[serde(deserialize_with = "de_content_warning")] pub content_warning: i32,
    #[prost(bool, tag = "8")] pub is_torrent: bool,
    #[prost(message, repeated, tag = "9")] pub sources: Vec<WireSource>,
}
#[derive(Clone, PartialEq, prost::Message, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WireResources { #[prost(string, tag = "1")] pub apk_url: String, #[prost(string, tag = "2")] pub icon_url: String }
#[derive(Clone, PartialEq, prost::Message, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WireSource {
    #[prost(int64, tag = "1")] pub id: i64, #[prost(string, tag = "2")] pub name: String, #[prost(string, tag = "3")] pub language: String,
    #[prost(string, tag = "4")] pub home_url: String, #[prost(string, repeated, tag = "5")] pub mirror_urls: Vec<String>,
    #[prost(string, optional, tag = "6")] pub message: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum ContentWarning { Unspecified = 0, Safe = 1, Mixed = 2, Nsfw = 3 }

#[derive(serde::Deserialize)]
pub struct WireLegacyRepo { #[serde(rename = "index_v2")] pub index_v2: Option<String>, pub meta: WireLegacyMeta }   // `meta` REQUIRED (discriminator)
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireLegacyMeta { pub name: String, pub short_name: Option<String>, pub website: String, pub signing_key_fingerprint: String }
#[derive(serde::Deserialize)]
pub struct WireLegacyExtension { pub name: String, pub pkg: String, pub apk: String, pub lang: String, pub code: i64,
                                 pub version: String, pub nsfw: i32, pub sources: Option<Vec<WireLegacySource>> }
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireLegacySource { pub id: i64, pub lang: String, pub name: String, pub base_url: String }
```

Decisions:
* JSON ignores unknown keys (serde default) and treats absent nullable keys as `None` (parity with `ignoreUnknownKeys` + `explicitNulls = false`).
* `contentWarning` in JSON accepts exactly the strings `UNSPECIFIED`, `SAFE`, `MIXED`, `NSFW` and the alternates `CONTENT_WARNING_UNSPECIFIED`, `CONTENT_WARNING_SAFE`, `CONTENT_WARNING_MIXED`, `CONTENT_WARNING_NSFW`; any other string is a decode error (kotlinx: `coerceInputValues` is off). A missing key becomes `UNSPECIFIED` (Proposal: lenient; upstream raises `MissingFieldException`).
* Required-field strictness: upstream requires every non-default, non-nullable property and fails the whole document if one is missing. Proposal: **proto3 semantics** (absent scalar = default value) because third-party encoders omit zero values and the Kotlin app's own encoder (if any) emits defaults; the only presence check kept is `contact` (needed for `contact.website`) — absent -> `StoreError::Invalid("contact")`. `resources` missing in an extension -> that extension is skipped with a warning (Kotlin would fail the entire list).
* Protobuf details of kotlinx `ProtoBuf` (library behaviour): `Long` = plain varint (`int64`, not zigzag), `Boolean` = varint, `String` = length-delimited UTF-8, nested classes = length-delimited, `List<String>`/`List<message>` = repeated non-packed fields, enum = varint of its `@ProtoNumber`. prost matches. Unknown tags are skipped by both.
* `extensionLib` stays a string until mapping (`parse::<f64>()`); a non-numeric value fails that extension list (`StoreError::Invalid("extensionLib")`), identical to Kotlin's `NumberFormatException` aborting the store's list.

## 3. `fetch(indexUrl)` — algorithm (OBSERVED `ESS:28-68`, with Rust notes)

```text
fetch(indexUrl):
  updatedIndexUrl = indexUrl
  try:
    response = GET updatedIndexUrl            # awaitSuccess: non-2xx -> HttpException(code)
    source   = body, decompressed if its first two bytes are 0x1F 0x8B       # decompressIfGzipped (ESS:111-121)
    first    = peek one byte                  # empty body -> EOF exception -> Result.failure
    case first of
      0x5B '[':                               # legacy index.min.json
          if not indexUrl.endsWith("/index.min.json"): throw IllegalArgumentException("Provided legacy store url is not valid")
          updatedIndexUrl = indexUrl.replace("/index.min.json", "/repo.json")     # String.replace: ALL occurrences
          networkStore = decode<NetworkLegacyAnimeExtensionRepo>(GET updatedIndexUrl .body)   # NOT gzip-sniffed
      0x7B '{':
          try   networkStore = decode<NetworkLegacyAnimeExtensionRepo>(peek of source)   # needs "meta"
          catch IllegalArgumentException (any SerializationException):
                networkStore = decode<NetworkAnimeExtensionStore>(source)                # JSON, same body
      else:
          networkStore = protobuf decode<NetworkAnimeExtensionStore>(all bytes)
    if networkStore is NetworkLegacyAnimeExtensionRepo and networkStore.indexV2 != null:
        return fetch(networkStore.indexV2)    # recursion, result returned as is (keyed by the v2 URL); NO depth limit
    return Success(networkStore.toExtensionStore(updatedIndexUrl))
  catch CancellationException: rethrow
  catch Exception e: log ERROR "Failed to add extension store '<updatedIndexUrl>'"; return Failure(e)
```

Observed consequences:
* A new-format store keeps the URL the user typed as its `index_url` (primary key). A legacy store typed as `index.min.json` is stored under the derived `.../repo.json` URL; typing `.../repo.json` directly stores that same URL.
* A legacy `repo.json` that advertises `index_v2` is never stored itself: the row is the v2 store (possibly at a different host).
* The UI's duplicate check compares the typed text with stored `index_url` strings (exact match, no normalisation), so adding the `index.min.json` form of an existing `repo.json` store is *not* flagged as a duplicate; the upsert simply overwrites the same row. Proposal: also compare after the same normalisation (`/index.min.json` -> `/repo.json`, trailing-slash trim).
* The `[` branch and the `{` branch use different HTTP bodies for the same typed URL: first body is only peeked in the `[` case.
* `fetch` is also the refresh primitive (`refreshAll` calls `fetch(store.index_url)`), so a legacy store refreshes through `repo.json` (`{` branch) and picks up an `index_v2` that appears later: the old row (keyed by the `repo.json` URL) remains and a NEW row (keyed by the v2 URL) is inserted by `upsert` — the user then sees two stores. Proposal: when a refresh of store S yields a store with a different `index_url`, delete S after inserting the new one (inside one transaction) — flagged improvement, task T-06a-053.

Rust (module `kuro_core::data::extension::store::service`):

```rust
pub struct ExtensionStoreService { http: Arc<HttpClient /* area:12 */> }
const MAX_INDEX_V2_HOPS: u8 = 3;     // Kotlin: unbounded recursion (a self-referencing index_v2 loops forever)

impl ExtensionStoreService {
    pub async fn fetch(&self, index_url: &str) -> Result<ExtensionStore, StoreError> {
        let mut url = index_url.to_owned();
        for _ in 0..=MAX_INDEX_V2_HOPS {
            let body = sniff_decompress(self.http.get_bytes(&url).await?)?;          // gzip magic -> flate2::read::GzDecoder, cap 32 MiB
            let body = strip_bom_and_ws(&body);                                        // Proposal: tolerate BOM/leading whitespace
            let (wire, effective_url) = match body.first() {
                None => return Err(StoreError::EmptyBody),
                Some(b'[') => {
                    if !url.ends_with("/index.min.json") { return Err(StoreError::LegacyUrlInvalid); }
                    let repo_url = url.replace("/index.min.json", "/repo.json");
                    let repo_body = self.http.get_bytes(&repo_url).await?;             // no gzip sniff, like Kotlin
                    (Wire::Legacy(serde_json::from_slice(&repo_body)?), repo_url)
                }
                Some(b'{') => match serde_json::from_slice::<WireLegacyRepo>(body) {
                    Ok(l) => (Wire::Legacy(l), url.clone()),
                    Err(_) => (Wire::Modern(serde_json::from_slice::<WireStore>(body)?), url.clone()),
                },
                Some(_) => (Wire::Modern(WireStore::decode(body)?), url.clone()),
            };
            if let Wire::Legacy(l) = &wire { if let Some(v2) = &l.index_v2 { url = v2.clone(); continue; } }
            return wire.into_store(&effective_url);
        }
        Err(StoreError::TooManyIndexHops)
    }
}
```

`StoreError` implements `Display` with the messages shown to the user (the dialog shows `throwable.message ?: "unknown error"`): `Http(code)` -> `"HTTP error <code>"`; `LegacyUrlInvalid` -> `"Provided legacy store url is not valid"` (exact Kotlin text); `EmptyBody` -> `"Empty response"`; `Json(e)` -> `serde_json` message prefixed `"Invalid store index: "`; `Proto(e)`; `TooManyIndexHops` -> `"Too many index_v2 redirects"`; `Invalid(field)`; `Network(e)` -> the reqwest error text.

## 4. `getExtensions(store)` — algorithm (OBSERVED `ESS:70-109`)

```text
if store.extensionListUrl != null:
    body = GET extensionListUrl; decompressIfGzipped
    list = if first byte == '{' then json<ExtensionList> else protobuf<ExtensionList>
    return list.toAvailableExtensions(store)
else if not store.isLegacy:
    body = GET store.indexUrl; decompressIfGzipped
    full = if first byte == '{' then json<NetworkAnimeExtensionStore> else protobuf<NetworkAnimeExtensionStore>
    return full.extensionList!!.toAvailableExtensions(store)       # NPE (caught) if the store has no inline list
else:  # legacy
    storeBaseUrl = store.indexUrl.removeSuffix("/repo.json")
    body = GET "$storeBaseUrl/index.min.json"                       # no gzip detection
    return json<List<NetworkLegacyAnimeExtension>>(body).map { it.toAvailableExtension(store, storeBaseUrl) }
any Exception (not cancellation) -> Result.failure(e)              # NOT logged here; the repository logs it
```

Notes: for a `getExtensions` of a store whose rows came from `insertFromPreference` (`isLegacy = false`, no list URL), the second branch is taken and fails (decoding a legacy `repo.json` object as `NetworkAnimeExtensionStore` fails because `name`, `badgeLabel`, `signingKey` and `contact` are absent, so a decode error is raised before `extensionList!!` is reached). It heals on the next `refreshAll` (which re-sniffs and sets `isLegacy = true`). `AnimeExtensionApi.checkForUpdates` calls `updateExtensionStores()` before listing for exactly that reason, but `AnimeExtensionManager.findAvailableExtensions()` (called after adding a store) does not — right after the migration the first listing can be empty until the first refresh. Proposal: run `refresh_all` once at the end of the migration (task T-06a-054).

`toAvailableExtensions` mapping table: `p1-data-and-config.md` section 1.5. Edge facts: `lang` = the single language when all sources share it, else `"all"`, which includes the zero-source case; `isNsfw` = `contentWarning >= MIXED`; `mirrorUrls`/`message` unused (a later Aniyomi release may use them; keep them in the wire struct and drop at mapping); `baseUrl` = `homeUrl` (default `""`).

`fetchExtensions()` (`ESR:69-88`): load stores (`SELECT * FROM extension_store`), launch `getExtensions` for all concurrently (`supervisorScope` + `async`), each failure logged as `"Failed to fetch extensions for store '<name> (<indexUrl>)'"`, flatten in store order, `emptyList()` if anything throws. Rust: `futures::stream::iter(stores).map(...).buffered(8).collect()`.

No de-duplication across stores happens here: the same `pkgName` from two stores yields two `Available` entries (area:19 resolves with `store` and version rules).

## 5. Persistence (`ESR`, `extension_store.sq`)

| Function | Rust (`StoreRepo`) | Notes |
|---|---|---|
| `insert(indexUrl): Result<Unit>` | `async fn add(&self, index_url) -> Result<(), StoreError>` = `fetch` then `upsert` | error text is user visible |
| `insertFromPreference(indexUrl, name)` | `async fn insert_from_preference` | `signing_key = "NO_SIGNING_KEY"`, `contact_website = indexUrl`, `badge_label = name`, `is_legacy = 0`, `extension_list_url = NULL`; uses the same upsert SQL (so re-running the migration is idempotent) |
| `refreshAll()` | `async fn refresh_all(&self)` | sequential; per-store failure logged `"Failed to refresh extension store '<name> (<index_url>)'"`; whole body in catch-all |
| `fetchExtensions()` | `async fn fetch_extensions(&self) -> Vec<AvailableExtension>` | section 4 |
| `getAll()`, `getAllAsFlow()`, `getCountAsFlow()` | `all()`, `watch_all()`, `watch_count()` | `ORDER BY rowid` Proposal |
| `remove(indexUrl)` | `async fn remove` | extensions already installed from that store stay installed (area:19) |

Mapper order for `SELECT *`: `index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url`.

Trust note (OBSERVED, preserved): refreshing a store overwrites `signing_key` without comparing the old value (the manga repo path rejects a changed fingerprint, `UpdateMangaExtensionRepo.kt`). Where the signing key is consumed (extension trust, area:19/20) is outside this area; Proposal: log a WARN when a refresh changes `signing_key` (task T-06a-053) so the divergence is at least visible.

## 6. Manga extension repos (`MER`, domain interactors)

Algorithms are in `p1-behavior.md` section 8. Rust notes (module `kuro_core::data::extension::repo`):

* `normalize_repo_index_url(input) -> Option<String>`: `url::Url::parse(input)`, require scheme `http|https` (OkHttp's `toHttpUrlOrNull` rejects every other scheme and empty hosts), serialise with `Url::as_str()` (WHATWG normalisation: lowercase scheme/host, default port removed, `.`/`..` segments resolved, percent-encoding normalised). Then require the regex `^https://.*/index\.min\.json$` (simple `starts_with("https://") && ends_with("/index.min.json")` check suffices; `.*` can be empty so `https:///index.min.json` cannot occur after parsing). Differences between WHATWG and OkHttp (e.g. backslashes, userinfo) are irrelevant for the https-only, `.json`-suffixed inputs accepted.
* `fetch_repo_details(base_url)`: GET `"{base_url}/repo.json"`, parse `{"meta": {"name", "shortName"?, "website", "signingKeyFingerprint"}}` (`ExtensionRepoMetaDto`), `None` on any error (logged `"Failed to fetch repo details"`).
* `SaveExtensionRepoException` = `RepoError::Save(DbError)`; created for **any** `rusqlite::Error` from `insert`/`upsert` (not only constraint errors), preserving Kotlin's behaviour that every `SQLiteException` becomes `Save`.
* Conflict classification after a failed insert uses two follow-up reads (`get_repo`, `get_repo_by_fingerprint`); between the failed insert and the reads another writer could change state (Kotlin has the same race); Rust does both reads in one read transaction.
* `update_all`: Kotlin runs refreshes concurrently and a failing one cancels the rest. Rust: `join_all` of independent futures, errors collected and logged, the rest continue (strictly better: no crash path).

## 7. Screens' state machines (what the Rust screens must reproduce; widgets are area:47)

Anime store screen (`AnimeExtensionStoresScreenModel.kt`, components under `presentation/more/settings/screen/browse/components/anime/`):

| State | Content |
|---|---|
| `Loading` | loading indicator until the first `getAllAsFlow` emission |
| `Success(stores, dialog?)` empty | `EmptyScreen("You haven't added an extension store yet")` |
| `Success` non-empty | `LazyColumn` of cards: label icon + store name (title medium); row of icon buttons right-aligned: open website (`action_open_in_browser`), Discord (only if `contact.discord != null`, no label), copy index URL (`action_copy_to_clipboard`), delete (`action_delete`) |
| toolbar | title `extensionStores` ("Extension stores"), back, refresh action (`action_webview_refresh` "Refresh" -> `refreshAll()` only) |
| FAB | "add" (a `CategoryFloatingActionButton`, hides on scroll down) -> `Create` dialog |

Dialogs: `Create(processing=false, errorMessage=null)`: title `extensionStoresScreen.addStore.title`; single-line URL field labelled `extensionStoresScreen.addStoreInput.inputLabel`, supporting text = `errorMessage ?: (alreadyExists ? extensionStoresScreen.addStore.alreadyExists : information_required_plain)`, error styling when `errorMessage != null || alreadyExists`; confirm button text `action_add` (or `extensionStoresScreen.addStore.processing` while processing) enabled iff `!processing && text.nonEmpty && !alreadyExists`; cancel `action_cancel`; the field is focused 100 ms after opening. `createRepo(url)`: set `processing = true` on the open Create/Confirm dialog; call `add`; on success -> `extensionManager.findAvailableExtensions()` then dismiss; on failure -> `processing = false`, `errorMessage = throwable.message ?: "unknown error"`. `Delete(store)`: title `extensionStoresScreen.deleteStore.title`, body `extensionStoresScreen.deleteStore.body` with name and index URL, buttons `action_ok` / `action_cancel`; confirm -> `remove(indexUrl)` then `findAvailableExtensions()`, dismiss. `Confirm(url, alreadyExists)` (from a deep link): same title, body `extensionStoresScreen.addStoreDeeplink.bodyText`, read-only URL field showing `alreadyExists` or the error text, confirm disabled when already exists or processing.

Deep links (manifest `app/src/main/AndroidManifest.xml:68-95`, handler `MainActivity.kt:549-565`): `aniyomi://add-repo?url=<u>` and `aniyomi://extension-store?url=<u>` open the anime store screen with the `Confirm` dialog; `tachiyomi://add-repo?url=<u>` opens the manga repo screen with a confirm dialog (`add_repo_confirmation`). Linux replacement: `x-scheme-handler/aniyomi` and `x-scheme-handler/tachiyomi` registered via the `FileAssociations` trait (area:53/area:01) and delivered through the single-instance forwarder; `url` is the percent-decoded query value.

Manga repo screen: list of repos (label icon + name, buttons open website `AutoMirrored.OpenInNew`, copy `baseUrl`/index url, delete), FAB Add, refresh (calls `updateExtensionRepo.awaitAll()`), dialogs Create (`ExtensionRepoCreateDialog`: title `action_add_repo`, body AYMR `action_add_repo_message`, field `label_add_repo_input`, `error_repo_exists` when the typed text equals an existing `baseUrl`, confirm enabled iff non-empty and not existing; on confirm `onCreate(name)` then dialog closes immediately — the result events arrive as toasts), Delete (`action_delete_repo`, `delete_repo_confirmation` with the base URL), Conflict (section 8 of the behaviour file), Confirm (`action_add_repo`, `add_repo_confirmation`). Events `InvalidUrl` / `RepoAlreadyExists` are shown as toasts with `invalid_repo_name` / `error_repo_exists`.

## 8. Risks and decisions

* There is no public specification in this repository for the *new* store format beyond the Kotlin classes; real-world stores (if any) may differ. Task T-06a-050 requires a fixture-driven decoder test with hand-built JSON/protobuf documents derived from the tables above; no network access is needed.
* The Linux extension strategy (area:20) may replace `apkUrl` with another artifact; the wire struct keeps the JSON/protobuf names, the Rust domain struct keeps `apk_url`.
* Gzip bombs: cap the decompressed size (32 MiB) — Kotlin has no cap.
* All store fetches use the shared HTTP client of area:12 (User-Agent, DNS-over-HTTPS preference, Cloudflare interceptor). Proposal: never send cookies/credentials to store URLs other than what the client does by default.
