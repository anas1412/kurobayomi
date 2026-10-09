# Area 10 topic: source and extension listing, ordering, pins, incognito, trust

Files: `app/src/main/java/eu/kanade/domain/source/**` (19 files), `app/src/main/java/eu/kanade/domain/extension/**` (10 files), `SourcePreferences.kt`. Consumers: browse tabs (area:40, area:41), settings (area:47), reader/player (incognito), extension loaders (`MangaExtensionLoader.kt:277`, `AnimeExtensionLoader.kt:276` call `isTrusted`).

## 1. Ordering primitives (must be reproduced exactly)

### 1.1 `String.CASE_INSENSITIVE_ORDER`
Java `String.CASE_INSENSITIVE_ORDER.compare(a, b)`: walk both strings by UTF-16 code unit up to `min(len)`; for each pair `c1 != c2`: compare `toUpperCase(c1)` and `toUpperCase(c2)`; if still different compare `toLowerCase` of those; if still different return `c1 - c2` (the lower-cased values); after the common prefix return `len1 - len2`. Not a locale collation: `"Z" < "a"` is false (`z` vs `a` compare after folding), accents sort after `z`.
Rust: `fn cmp_ignore_case_java(a: &str, b: &str) -> Ordering` iterating `a.encode_utf16()` zip `b.encode_utf16()` with a `fold(u16)` that does the same two-step fold using `char::from_u32(c).map(|c| c.to_uppercase().next())` for BMP scalar values (surrogates compared raw). Do NOT use `to_lowercase()` on the whole string (changes length for some characters).

### 1.2 `LocaleHelper.comparator` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt`)
`(a, b) -> if (a == "all") -1 else if (b == "all") 1 else getLocalizedDisplayName(a).compareTo(getLocalizedDisplayName(b))` where `String.compareTo` is Java lexicographic order on UTF-16 code units (case-sensitive, uppercase before lowercase). `"all"` always sorts first (when both are "all" it returns -1, irrelevant for unique keys).
`getLocalizedDisplayName(lang)`: `null` -> `""`; `""` -> the first system locale; `"zh-CN"` -> locale `zh-Hans`; `"zh-TW"` -> `zh-Hant`; else `Locale.forLanguageTag(lang)`; result `locale.getDisplayName(locale)` with the first character upper-cased in that locale (the language name written in its own language: `en` -> "English", `ja` -> "日本語", `es` -> "Español", `pt-BR` -> "Português (Brasil)").
Proposal: `kuro-i18n::lang_display_name(tag: &str) -> String` backed by a table generated at build time from CLDR native language names for every tag that appears in the extension index (about 90 tags); lookup tries the full tag, then `language-script`, then the bare language, finally the raw tag. The sort key is the resulting string compared by UTF-16 code units (`encode_utf16().cmp(...)`). A test pins the order for `all, ar, de, en, es, fr, id, it, ja, ko, pl, pt, pt-BR, ru, th, tr, uk, vi, zh-CN, zh-TW`.
Other helper functions in that file: `getSourceDisplayName(lang)`: `"last_used"` -> `MR.strings.last_used_source`, `"pinned"` -> `MR.strings.pinned_sources`, `"other"` -> `MR.strings.other_source`, `"all"` -> `MR.strings.multi_lang` ("Multi"), else localized display name. `getDefaultEnabledLanguages()` = `{"all", "en", Locale.getDefault().language}`.
Proposal for the default: `sys_locale::get_locale()` (crate `sys-locale`) -> first subtag lower-cased (`en-US` -> `en`; `C`/`POSIX` -> `en`).

## 2. GetEnabledMangaSources / GetEnabledAnimeSources

Inputs: `pinned_catalogues` (set of decimal ids), `source_languages`, `hidden_catalogues`, `__APP_STATE_last_catalogue_source` (long, -1 none), `data_saver_excluded` (manga only), and `repository.getMangaSources()` (every installed + stub + local source, area:06). Pipeline per emission:
1. `keep = source.lang in enabledLanguages || source.id == LocalMangaSource.ID (0)`.
2. `drop if source.id.toString() in disabled`.
3. `sortedWith(CASE_INSENSITIVE_ORDER by name)` (stable; equal names keep repository order).
4. For each source: `pin = if "$id" in pinned then Pins.pinned (0b11) else Pins.unpinned (0)`; manga also `isExcludedFromDataSaver = "$id" in dataSaverExcluded`. Emit it. If `source.id == lastUsedSource` emit a second copy with `isUsedLast = true` and `pin = source.pin - Pin.Actual` (XOR).
5. `distinctUntilChanged()` on the whole list (structural equality).
Note the local source is always visible even if its language is not enabled but it CAN be hidden through `hidden_catalogues`.

Rust:
```rust
pub fn enabled_sources(inputs: &EnabledInputs, sources: &[Source]) -> Vec<Source>   // pure, testable
pub fn watch_enabled_sources(d: &Domain, kind: EntryKind) -> impl Stream<Item = Arc<Vec<Source>>>
```
UI grouping (area:40/41 `MangaSourcesScreenModel.kt:70-110`): `TreeMap` with a comparator placing `last_used` first, then `pinned`, then everything else by key with `""` (no language) last; items grouped by `isUsedLast -> "last_used"`, `Pin.Actual in pin -> "pinned"`, else `lang`; within a group the sort of step 3 holds.

## 3. GetLanguagesWithMangaSources / AnimeSources

Input: enabled languages, disabled ids, `repository.getOnlineMangaSources()` (excludes the local source and stubs? - area:06 decides; it is the list of sources backed by installed extensions). Output: `SortedMap<String, List<Source>>`:
1. sort sources by `(id in disabled)` (false first) then name (`CASE_INSENSITIVE_ORDER`).
2. `groupBy(lang)` (insertion order inside groups follows the sort).
3. `toSortedMap(compareBy<String> { it !in enabledLanguage }.then(LocaleHelper.comparator))`: enabled languages first, each block ordered by `LocaleHelper.comparator`.
Used by the "Languages / sources" settings page (area:47) where the toggles call `ToggleLanguage` and `ToggleMangaSource(sourceIds, enable)`.

## 4. GetSourcesWithFavoriteCount (migration list)

`combine(migrationSortingDirection, migrationSortingMode, repository.getMangaSourcesWithFavoriteCount())` -> list of `(Source, favoriteCount: Long)` without the local source (`id == 0`). Comparator `sortFn(a, b)`:
- `ALPHABETICAL`: `a.isStub && !b.isStub -> -1`; `b.isStub && !a.isStub -> 1`; else `a.name.lowercase().compareToWithCollator(b.name.lowercase())`.
- `TOTAL`: the same stub rule, else `a.second.compareTo(b.second)` (Long).
- `ASCENDING` uses it as is; `DESCENDING` uses `Collections.reverseOrder(sortFn)`, which REVERSES the stub rule too (stubs last when descending).
`compareToWithCollator` = `java.text.Collator.getInstance()` default-locale comparison (`core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:13`). Proposal: `icu_collator` (crate `icu` with `collator`) created for the system locale, or `unicode-collation` as a lighter fallback; both are TERTIARY strength by default which matches `Collator.getInstance()` (TERTIARY).

## 5. Toggles

All mutate a string-set preference with get-then-set (non atomic):
- `ToggleMangaSource.await(source|id, enable = isEnabled(id))`: `isEnabled(id) = id.toString() in disabledSet` (the name is inverted: it answers "is currently DISABLED"). Calling without `enable` therefore flips the state. `enable = true` -> `disabled - id`; `false` -> `disabled + id`. The list overload `await(ids, enable)` does set subtraction/union with the decimal strings.
- `ToggleMangaSourcePin.await(source)`: reads `isPinned = "id" in pinned` then removes or adds.
- `ToggleLanguage.await(lang)`: removes or adds in `source_languages`.
- `ToggleMangaIncognito.await(extensionPackage, enable)`: `enable` adds, else removes, in `incognito_manga_extensions` (the key is the extension PACKAGE name, not a source id).
- `ToggleExcludeFromMangaDataSaver.await(source)`: toggles the decimal id in `data_saver_excluded`.
Proposal: use `Pref<BTreeSet<String>>::update(|set| ...)` which is atomic inside the area:02 store lock.

## 6. Incognito

`GetMangaIncognitoState.await(sourceId: Long?)`: (1) global `__APP_STATE_incognito_mode` true -> true; (2) `sourceId == null` -> false; (3) `extensionManager.getExtensionPackage(sourceId) ?: return false` (unknown source -> false); (4) `package in incognito_manga_extensions`.
`subscribe(sourceId)`: `sourceId == null` -> `incognito_mode.changes()`; else `combine(incognito_mode, incognitoExtensions, extensionManager.getExtensionPackageAsFlow(sourceId)) { i, set, pkg -> i || (pkg in set) }.distinctUntilChanged()`. Note the null package in the combine yields `null in set` = false.
Consumers: history writes are suppressed in incognito (area:43/45), progress is not saved, tracker updates are skipped (`ReaderViewModel.updateTrackChapterRead`).

## 7. GetMangaExtensionsByType / GetAnimeExtensionsByType

Inputs (all reactive except NSFW): `enabledLanguages`, `installedExtensionsFlow`, `untrustedExtensionsFlow`, `availableExtensionsFlow`; `showNsfwSources = preferences.showNsfwSource().get()` is read once at `subscribe()`.

1. `installed` = installed extensions filtered by `showNsfw || !isNsfw`, sorted by `(!isObsolete)` ascending (so OBSOLETE FIRST: `false < true`) then name case-insensitive; `partition { hasUpdate }` -> `(updates, installed)` (each keeps the sort).
2. `untrusted` = all untrusted sorted by name case-insensitive (the NSFW filter is NOT applied, `lang`/`isNsfw` unknown there).
3. `available` = available extensions where no installed ext has the same `pkgName`, no untrusted ext has the same `pkgName`, and `showNsfw || !isNsfw`; then `flatMap`:
   - manga: if `ext.sources.isEmpty()` -> `[ext]` when `ext.lang in enabledLanguages`, else `[]`.
   - both: for each `source` of `ext.sources` with `source.lang in enabledLanguages` -> `ext.copy(name = source.name, lang = source.lang, pkgName = "${ext.pkgName}-${source.id}", sources = listOf(source))` (each source becomes its own row; the anime variant has no empty-sources branch, so an anime extension without sources never appears in "available").
   - sort by name case-insensitive.
4. Emit `MangaExtensions(updates, installed, available, untrusted)`.
The suffixed `pkgName` is only a UI row key: install uses `extension.apkUrl` (`AnimeExtensionManager.installExtension` -> `installer.downloadAndInstall(extension.apkUrl, extension)`), and the screen model tracks download progress in `currentDownloads` keyed by `extension.pkgName` (`AnimeExtensionsScreenModel.kt:50,188,192`), so the per-source suffix keeps several rows of one multi-source extension from sharing a progress state. Proposal: keep `AvailableExtensionRow { ext: Arc<Available>, source: Option<SourceRef> }` instead of mutating `pkgName`, and derive the row key `format!("{}-{}", pkg, source_id)` for the list widget and the download-progress map only.

### 7.1 Extension languages
`GetMangaExtensionLanguages.subscribe`: `combine(enabledLanguages, availableExtensionsFlow)`; collect `ext.sources.map(lang)` per extension (manga: `[ext.lang]` if the extension has no sources; anime: just the sources' languages), `distinct`, then sort by `(lang !in enabled)` then `LocaleHelper.comparator`.

### 7.2 Extension sources
`GetExtensionSources.subscribe(extension: Installed)` (manga) / `GetAnimeExtensionSources`: `isMultiSource = sources.size > 1`; `isMultiLangSingleSource = isMultiSource && sources.map(name).distinct().size == 1`; on every change of `hidden_catalogues` / `hidden_anime_catalogues` emit `sources.map { Item(source, enabled = id.toString() !in disabled, labelAsName = isMultiSource && !isMultiLangSingleSource) }`. `labelAsName = true` makes the UI label each source row with the source's name (distinct names inside one extension); `false` labels by language (the usual multi-language extension where every source has the same name).

## 8. Trust

`TrustMangaExtension` / `TrustAnimeExtension` (p1-behavior.md section 11). OBSERVED details:
- `isTrusted(pkgInfo, fingerprints)`: `trustedFingerprints = repo.getAll().map(signingKeyFingerprint | signingKey).toHashSet()` (manga reads `MangaExtensionRepoRepository`, anime reads `AnimeExtensionStoreRepository`); `key = "${packageName}:${versionCode}:${fingerprints.last()}"` where `versionCode = PackageInfoCompat.getLongVersionCode`; return `trustedFingerprints.any { it in fingerprints } || key in trustedExtensions`. An empty `fingerprints` list throws `NoSuchElementException` from `last()` BEFORE the first condition is evaluated.
- `trust(pkgName, versionCode, signatureHash)`: `exts.filterNot { it.startsWith("$pkgName:") } + "$pkgName:$versionCode:$signatureHash"` - trust is per exact version; installing an update of an untrusted-origin extension asks again, because the older entry was removed and the new version code differs.
- `revokeAll()` = `trustedExtensions().delete()` (back to the empty default); repo/store trust (signing keys of user-added repositories) is NOT revoked by it.
Proposal (needs the final package model from area:20): replace `PackageInfo + signing certificate fingerprints` with `PackageIdentity { name: String, version_code: i64, fingerprints: Vec<String> }` where `fingerprints` are SHA-256 hex strings of whatever signature the Linux extension format uses (area:20 chooses: detached minisign/ed25519 public-key fingerprint of the repository index or a hash of the package). `fingerprints` must be non-empty; `is_trusted` returns `false` (and logs) for an empty list instead of panicking. The stored triple format `"<pkg>:<versionCode>:<hash>"` is kept so backups of the `trusted_extensions` preference stay compatible.

## 9. Icons (`AnimeSource.icon`, `Source.icon`)
OBSERVED: `Injekt.get<MangaExtensionManager>().getAppIconForSource(id)?.toBitmap()?.asImageBitmap()` - the installed extension APK's launcher icon. Linux replacement: area:20 downloads an icon file per extension (`iconUrl` in `Available`) into the cache directory; `SourceIcons::get(source_id) -> Option<gdk::Texture>` loads `~/.cache/kurobayomi/ext-icons/<pkg>.png` and falls back to a generic icon from the GTK icon theme (`application-x-addon-symbolic`). The local source uses the app icon.
