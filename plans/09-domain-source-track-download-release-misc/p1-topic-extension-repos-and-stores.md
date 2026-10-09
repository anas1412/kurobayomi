# Topic: extension repositories (manga) and extension stores (anime)

Deep dive for area 09. Covers the two parallel subsystems that tell Kurobayomi where extensions come from. Sources of truth: the 15 scope files under `domain/src/main/java/mihon/domain/extensionrepo/`, `domain/src/main/java/mihon/domain/extension/anime/`, and the data-layer implementations they call (`data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt`, `data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt`, `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt`, `data/src/main/sqldelight/data/extension_repos.sq`, `data/src/main/sqldelightanime/dataanime/extension_store.sq`). Extension loading and installation are areas 18–20.

## 1. Why two systems

| | Manga extension repo | Anime extension store |
|---|---|---|
| Identity | `base_url` (PK), `signing_key_fingerprint` (UNIQUE) | `index_url` (PK) |
| Index URL shape | must match `^https://.*/index\.min\.json$` | any URL; legacy must end in `/index.min.json` |
| Metadata document | `GET <base>/repo.json` | `GET <index_url>`, format sniffed |
| Trust anchor | signing-key fingerprint (TOFU, pinned on first add) | `signing_key` stored, not enforced in this scope |
| Extension list | from the repo's index (area 18) | `extension_list_url` or the index (area 19) |
| Refresh | parallel, guarded by fingerprint | sequential |

The asymmetry is historical: manga repos were added with a fingerprint-based trust model; anime stores grew up from a preference list and carry a wire-format sniffer for legacy and protobuf indexes.

## 2. State machines

### 2.1 Add a manga repo (observed: `CreateMangaExtensionRepo.kt`)
```
input URL
  ├─ not parseable or fails ^https://.*/index.min.json$ ──────────────► InvalidUrl
  └─ base = URL minus "/index.min.json"
       └─ GET <base>/repo.json
            ├─ any error / bad JSON ─────────────────────────────────► InvalidUrl
            └─ ExtensionRepo parsed
                 └─ INSERT
                      ├─ ok ─────────────────────────────────────────► Success
                      └─ SQLite error (SaveExtensionRepoException)
                           ├─ base_url row exists ─────────────────► RepoAlreadyExists
                           ├─ fingerprint row exists ──────────────► DuplicateFingerprint(old, new)
                           └─ neither ─────────────────────────────► Error
```
User-visible outcomes (Proposal wording): InvalidUrl → "That is not a valid extension repository address."; RepoAlreadyExists → "This repository is already added."; DuplicateFingerprint → a dialog offering to replace the old entry (calls `ReplaceMangaExtensionRepo`); Error → "Could not save the repository."

### 2.2 Refresh a manga repo (observed: `UpdateMangaExtensionRepo.kt`)
```
for each repo in parallel:
  fetched = GET <base>/repo.json
  if fetched is None → leave row as is
  if stored.fingerprint startswith "NOFINGERPRINT" or stored.fingerprint == fetched.fingerprint
       → UPSERT fetched (name, short_name, website, fingerprint)
  else → leave row as is (fingerprint changed: possible hijack)
```
This is a trust-on-first-use guard. The "NOFINGERPRINT" prefix marks repos that never published a fingerprint; they are upgraded to a real fingerprint the first time one appears.

Proposal: the fingerprint-changed branch raises a visible warning on the repo row ("Signing key changed. Updates are paused until you review it.") with actions "Trust new key" (replace) and "Remove". Without this, a silently skipped repo looks like a dead repo.

### 2.3 Add an anime store (observed: `AnimeExtensionStoreService.fetch`)
```
GET indexUrl (gunzip if magic 1f 8b)
peek first byte
  '[' (0x5B) → legacy list
       requires URL ends with /index.min.json, else error
       fetch <base>/repo.json and decode legacy repo
  '{' (0x7B) → try legacy repo decode; on IllegalArgumentException → current format
  other      → protobuf current format
if decoded legacy has indexV2 → fetch(indexV2)          (one recursion in Rust; unbounded in Kotlin)
store = decoded.toExtensionStore(updatedIndexUrl)
UPSERT store keyed by store.indexUrl                    (Q-STORE-1: may differ from the typed URL)
```
Proposal: key by the typed URL and keep `resolved_index_url` in an extra column, so a legacy URL typed by the user is found again on refresh.

### 2.4 Refresh anime stores (observed)
```
for each row in extension_store (sequential):
  fetch(row.index_url) → UPSERT, or log "Failed to refresh extension store '<name> (<url>)'"
```
Proposal: add a per-store timeout from `kuro-net` (E-STO-05) so one slow host does not stall the loop.

### 2.5 Fetch available extensions across stores (observed)
```
parallel per store:
  if extension_list_url → GET it → decode list
  elif !is_legacy      → GET index_url → decode → .extensionList (must exist)
  else (legacy)        → GET <base>/index.min.json → decode legacy list
failed store → contributes nothing
```
Result: the union of successful stores. An overall exception yields an empty list (not an error), so the Extensions tab shows "no extensions" rather than an error state. Proposal: return per-store status so the UI can say which store failed.

## 3. Data

- `extension_repos` and `extension_store` schemas: `p1-data-and-config.md` sections 3.1 and 3.2.
- Insert vs upsert semantics: manga insert is plain (fails on conflict); manga upsert is `ON CONFLICT(base_url)`; replace is `ON CONFLICT(signing_key_fingerprint)`; anime upsert is `ON CONFLICT(index_url)`.
- Counts are exposed as `Flow<Int>` (manga) and `Flow<Long>` (anime); Rust uses `watch::Receiver<usize>` and `<i64>`.

## 4. Failure handling matrix

| Situation | Kotlin behaviour | Rust behaviour (task) |
|---|---|---|
| repo.json HTML with 200 | parse error → InvalidUrl | same (T-09a-026, TC-REPO-04) |
| One repo down during refresh | skipped silently | skipped, others continue (T-09a-028, TC-REPO-08) |
| One repo's upsert throws during refresh | cancels all siblings (Q-REPO-2) | isolated per task (T-09a-028) |
| Fingerprint changed upstream | skipped silently | skipped with a warning state (Proposal) |
| Store refresh with one failure | logged, others continue | same (T-09a-032) |
| Store removed during refresh | row re-created by upsert (RC-03) | re-check before upsert (Proposal) |
| `indexV2` loop | stack overflow (unbounded) | depth limit 1 (E-STO-07) |
| Protobuf store | decoded | rejected until area 19 schema (T-09a-031) |

## 5. Drop and keep decisions

| Item | Decision | Reason | What the user loses |
|---|---|---|---|
| Android `ProtoBuf` decode | Deferred to area 19 | needs the schema from the extension module | legacy protobuf-only stores until area 19 lands |
| `DiskUtil` / SAF for extension files | Not applicable here | extension installation is area 18/19 | none in this scope |
| `OkHttp` client | Replaced by reqwest | Rust stack | none |
| TOFU fingerprint model | Kept | it is the trust model users rely on | none |
| `insertFromPreference` migration | Kept (T-09a-035) | old preference-based stores still exist in backups | none |

## 6. Decisions for the user

1. Should a changed signing key pause updates (recommended, Proposal in section 2.2) or only warn?
2. Should anime stores keep the legacy `index.min.json` URL form, or normalise to `repo.json` on add? Normalising fixes Q-STORE-1 but changes which URL the user sees.
3. Maximum concurrent repo fetches: Proposal 4. Confirm, or leave unbounded as in Kotlin.
