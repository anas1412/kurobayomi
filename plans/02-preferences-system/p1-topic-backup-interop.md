# Topic: preference backup, restore and Aniyomi interoperability

Scope files: `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionPreferences.kt`, `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt`, `AndroidPreferenceStore.kt`, `domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/utils/Preferences.kt`. Related code outside the scope (read for exactness): `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt`, `.../restore/restorers/PreferenceRestorer.kt`, `.../restore/BackupRestorer.kt`, `.../create/BackupCreator.kt`, `.../create/BackupOptions.kt`, `.../restore/RestoreOptions.kt`, `.../models/BackupPreference.kt`, `.../models/Backup.kt`, `domain/src/main/java/tachiyomi/domain/backup/service/PreferenceValues.kt`. The backup file itself (gzip, proto layout of the rest of `Backup`) belongs to area:21-backup-format-and-create and area:22-backup-restore; this topic defines the preference part.

## 1. Goal

A Kurobayomi user can (a) restore the preferences of an `.tachibk` produced by Aniyomi on Android, and (b) produce an `.tachibk` whose preference part Aniyomi accepts. Android-only keys survive the round trip untouched.

## 2. Backup options involved (OBSERVED)

| Option | Default | Effect on preferences |
|---|---|---|
| `BackupOptions.appSettings` | true | include app preferences (`Backup.backupPreferences`, ProtoNumber 104) |
| `BackupOptions.sourceSettings` | true | include per-source preferences (`Backup.backupSourcePreferences`, ProtoNumber 105) |
| `BackupOptions.privateSettings` | false | keep keys with the `__PRIVATE_` prefix |
| `RestoreOptions.appSettings`, `sourceSettings` | true | apply the respective sections |
| `RestoreOptions.categories` | true | when false, `restoreAppPreferences(prefs, null)` is called and all category remapping becomes a no-op |

## 3. Proto messages (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupPreference.kt`:

```kotlin
@Serializable data class BackupPreference(@ProtoNumber(1) val key: String, @ProtoNumber(2) val value: PreferenceValue)
@Serializable data class BackupSourcePreferences(@ProtoNumber(1) val sourceKey: String, @ProtoNumber(2) val prefs: List<BackupPreference>)
@Serializable sealed class PreferenceValue
@Serializable data class IntPreferenceValue(val value: Int) : PreferenceValue()
@Serializable data class LongPreferenceValue(val value: Long) : PreferenceValue()
@Serializable data class FloatPreferenceValue(val value: Float) : PreferenceValue()
@Serializable data class StringPreferenceValue(val value: String) : PreferenceValue()
@Serializable data class BooleanPreferenceValue(val value: Boolean) : PreferenceValue()
@Serializable data class StringSetPreferenceValue(val value: Set<String>) : PreferenceValue()
```

The subclass `value` properties have no `@ProtoNumber`, so kotlinx.serialization numbers them starting at 1 in declaration order (the single property is field 1). The sealed parent is encoded polymorphically by kotlinx-serialization-protobuf: a nested message with field 1 = the subclass serial name (default: the fully qualified class name, here `eu.kanade.tachiyomi.data.backup.models.IntPreferenceValue` and so on) and field 2 = the subclass message. Resulting wire layout of one preference:

```
BackupPreference {
  1: string key
  2: message {            // PreferenceValue wrapper
       1: string "eu.kanade.tachiyomi.data.backup.models.<Kind>PreferenceValue"
       2: message { 1: <value> }     // Int/Long varint (Int negative = 10-byte sign-extended varint), Float fixed32,
                                     // Boolean varint 0/1, String length-delimited, Set<String> = repeated string
     }
}
```

A second, byte-for-byte identical copy of these classes exists in package `eu.kanade.tachiyomi.data.backup.full.models` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/BackupPreference.kt` and `Backup.kt`); no code outside that folder references it (grep). Its serial names would carry `...backup.full.models...`. Proposal: the decoder identifies the kind by the segment after the last `.` of the serial name and requires it to be one of `IntPreferenceValue`, `LongPreferenceValue`, `FloatPreferenceValue`, `StringPreferenceValue`, `BooleanPreferenceValue`, `StringSetPreferenceValue`, so backups from both packages (and from forks that moved the package) decode. The encoder always writes the `eu.kanade.tachiyomi.data.backup.models.` prefix.

Unused model: `BackupExtensionPreferences(name @1, prefs @2: List<BackupPreference>)` (`BackupExtensionPreferences.kt`) is not referenced by `Backup` or any creator/restorer. The decoder ignores unknown fields; nothing is written.

## 4. Backup creation algorithm (OBSERVED, `PreferenceBackupCreator.kt`)

`createApp(includePrivatePreferences)`:

1. `entries = preferenceStore.getAll()` (every key of the default SharedPreferences file).
2. Drop keys where `Preference.isAppState(key)` (prefix `__APP_STATE_`).
3. For each remaining `(key, value)`, map by runtime type: `Int` -> `IntPreferenceValue`, `Long` -> `LongPreferenceValue`, `Float` -> `FloatPreferenceValue`, `String` -> `StringPreferenceValue`, `Boolean` -> `BooleanPreferenceValue`, `Set<*>` -> `StringSetPreferenceValue` only if the cast `as? Set<String>` succeeds (it does for SharedPreferences), anything else is dropped (`mapNotNull`).
4. If `!includePrivatePreferences`, drop keys where `Preference.isPrivate(key)`.

`createSource(includePrivatePreferences)`: for every anime source that is a `ConfigurableAnimeSource` (`animeSourceManager.getAll()`), then every manga catalogue source that is a `ConfigurableSource` (`mangaSourceManager.getCatalogueSources()`), build `BackupSourcePreferences(sourceKey = source.preferenceKey(), prefs = source.sourcePreferences().all.toBackupPreferences().withPrivatePreferences(include))`, then keep only those with at least one preference (`prefs.isNotEmpty()`). Anime entries come first, then manga. App-state filtering applies here too (same `toBackupPreferences`).

Side effect of creation: none on preferences. The auto-backup path (`BackupCreator`) writes `lastAutoBackupTimestamp` (`__APP_STATE_last_auto_backup_timestamp`) with `Instant.now().toEpochMilli()` after a successful auto backup and keeps `MAX_AUTO_BACKUPS = 4` files.

Proposal (Rust `Prefs::export_app` / `export_sources`):

1. Call `store.flush()` first so the file and memory agree, then `store.all()` filtered to `Namespace::Normal` (plus `Private` only when `include_private`, read from the keyring). Registry keys only come from `config.toml` plus `[extra]`.
2. Keys are written with the original Aniyomi key strings (the registry key) and the Kotlin kind of the registry (Int/Long/Float/...). Because defaults are not written to `config.toml` unless the user changed them, the export contains only explicitly set values, exactly like Aniyomi (SharedPreferences also only holds explicitly set values).
3. Sets are exported sorted.
4. Per-source: one `BackupSourcePreferences` per `sources/source_<id>.toml` that has at least one key; `sourceKey` = `source_<id>`; ordering: ascending id.
5. Dropped, Android-only keys that the user never had are simply absent.

## 5. Restore algorithm (OBSERVED, `PreferenceRestorer.kt`)

`restoreApp(preferences, backupCategories)`:

1. `restorePreferences(preferences, preferenceStore, backupCategories)`.
2. Then `AnimeLibraryUpdateJob.setupTask(context)`, `MangaLibraryUpdateJob.setupTask(context)`, `BackupCreateJob.setupTask(context)` so interval/restriction preferences take effect (each re-reads its interval pref and (re)schedules or cancels the periodic job).

`restoreSource(preferences)`: for each `BackupSourcePreferences`, open `AndroidPreferenceStore(context, sourcePreferences(sourceKey))` (that is the SharedPreferences file named `sourceKey`, created if missing, even if the source is not installed) and run `restorePreferences(prefs, sourcePrefs)` without category mapping.

`restorePreferences(toRestore, store, backupCategories)`:

1. If `backupCategories != null`, load all manga categories and all anime categories (`getMangaCategories.await()`, `getAnimeCategories.await()`); else both lists are empty. Build `mangaCategoriesByName`, `animeCategoriesByName` (name -> Category) and `backupCategoriesById` = `backupCategories.associateBy { it.id.toString() }`.
2. `prefs = store.getAll()` (snapshot at start; used only for the type test).
3. For each `(key, value)` inside `try { ... } catch (e: Exception) { Log.e("PreferenceRestorer", "Failed to restore preference <$key>", e) }`:
   * `IntPreferenceValue`: if `prefs[key] is Int?` (existing value is an Int or absent) then
     * `key == "default_category"` (`DEFAULT_MANGA_CATEGORY_PREF_KEY`): `newValue = backupCategoriesById[value.toString()]?.let { mangaCategoriesByName[it.name]?.id?.toInt() }`;
     * `key == "default_anime_category"`: same with `animeCategoriesByName` (BUT `backupCategoriesById` is built from the manga category list only, see Q11 in `p1-topic-quirks-and-bugs.md`);
     * otherwise `newValue = value`;
     * `newValue?.let { store.getInt(key).set(it) }` (null mapping means nothing is written; in particular `-1` ("ask") has no entry in `backupCategoriesById`, so a backup default of -1 never overwrites a device default).
   * `LongPreferenceValue`: if `prefs[key] is Long?` set `getLong(key)`.
   * `FloatPreferenceValue`: if `prefs[key] is Float?` set `getFloat(key)`.
   * `StringPreferenceValue`: if `prefs[key] is String?` set `getString(key)`.
   * `BooleanPreferenceValue`: if `prefs[key] is Boolean?` set `getBoolean(key)`.
   * `StringSetPreferenceValue`: if `prefs[key] is Set<*>?`: `restored = restoreCategoriesPreference(...)`; if not restored, `getStringSet(key).set(value)`.
4. `restoreCategoriesPreference(key, value, store, backupCategoriesById, mangaByName, animeByName)`: `categoryPreferences = LibraryPreferences.categoryPreferenceKeys + DownloadPreferences.categoryPreferenceKeys` (12 keys, listed in section 6). If `key !in categoryPreferences` return false. Otherwise for each element `it` of the backup set compute two candidates: `backupCategoriesById[it]?.name?.let { mangaCategoriesByName[name]?.id?.toString() }` and the same with `animeCategoriesByName`; flatten, drop nulls; if the list is non-empty do `store.getStringSet(key) += ids` (union with the device's current set, NOT replacing); return true (even when nothing was added, so the unmapped ids are discarded).
5. Because a key absent on the device satisfies `prefs[key] is X?` for every X, an absent key is restored with whatever type the backup says. Keys the app never reads (Android-only, future, or from a fork) are therefore carried into the file.

Observed gaps: the type test runs against the snapshot taken before any write, so a key repeated twice in a backup would be written twice; no range validation (a restored `download_slots` of 99 is accepted; UI clamps later); restore happens on a coroutine launched concurrently with `restoreCategories`, so the category lookup can run before the categories exist (Q11 in the quirks file).

### Proposal: Rust restore (`Prefs::import_app`)

Inputs: `Vec<PrefEntry>`, `CategoryRemap { manga: HashMap<backup_id, device_id_by_name>, anime: HashMap<backup_id, device_id_by_name> }` built by area:22-backup-restore after categories are restored (so the ordering race disappears).

1. Run only after categories have been restored and committed.
2. For each entry, look up the registry: 
   * known key, kind equal: write it (after the special cases below);
   * known key, kind different: skip and count `skipped_kind_mismatch` (stricter than Kotlin, avoids writing a value that the next `get()` would delete);
   * unknown key (not in registry): keep verbatim in `[extra]` with its kind (round-trip), count `kept_unknown`;
   * key with prefix `__APP_STATE_`: skip (these are never exported, a hostile file must not set them) and count `skipped_app_state`;
   * key with prefix `__PRIVATE_`: only accepted when the user ticked "Private settings" on restore; written to the keyring.
3. Special cases (match Kotlin): `default_category` and `default_anime_category` remap through the respective map by the backup category's name; unmapped values are not written (including -1). The six library and six download category-set keys (list in section 6) are remapped per element through manga and anime maps and unioned into the existing set. Proposal change: remap manga keys only through the manga map and anime keys only through the anime map (Kotlin tries both for every element).
4. All writes go through one `store.replace_many` so UI listeners get a single `Changed { key: None }`.
5. After import: reschedule the three periodic jobs (library update manga, library update anime, auto backup) via the scheduler of area:26-library-update-jobs/22, then `flush()`.
6. Return `ImportReport { written, skipped_kind_mismatch, skipped_app_state, skipped_private, kept_unknown, category_unmapped }`; the restore summary dialog shows non-zero counts under the existing restore-error message mechanism of area:22-backup-restore.

## 6. Category-id preference keys (remapped on restore)

| Constant | Key | Class |
|---|---|---|
| `DEFAULT_MANGA_CATEGORY_PREF_KEY` | `default_category` (Int) | `LibraryPreferences` |
| `DEFAULT_ANIME_CATEGORY_PREF_KEY` | `default_anime_category` (Int) | `LibraryPreferences` |
| `LIBRARY_UPDATE_MANGA_CATEGORIES_PREF_KEY` | `library_update_categories` (set) | `LibraryPreferences` |
| `LIBRARY_UPDATE_ANIME_CATEGORIES_PREF_KEY` | `animelib_update_categories` (set) | `LibraryPreferences` |
| `LIBRARY_UPDATE_MANGA_CATEGORIES_EXCLUDE_PREF_KEY` | `library_update_categories_exclude` (set) | `LibraryPreferences` |
| `LIBRARY_UPDATE_ANIME_CATEGORIES_EXCLUDE_PREF_KEY` | `animelib_update_categories_exclude` (set) | `LibraryPreferences` |
| `REMOVE_EXCLUDE_MANGA_CATEGORIES_PREF_KEY` | `remove_exclude_categories` (set) | `DownloadPreferences` |
| `REMOVE_EXCLUDE_ANIME_CATEGORIES_PREF_KEY` | `remove_exclude_anime_categories` (set) | `DownloadPreferences` |
| `DOWNLOAD_NEW_MANGA_CATEGORIES_PREF_KEY` | `download_new_categories` (set) | `DownloadPreferences` |
| `DOWNLOAD_NEW_ANIME_CATEGORIES_PREF_KEY` | `download_new_anime_categories` (set) | `DownloadPreferences` |
| `DOWNLOAD_NEW_MANGA_CATEGORIES_EXCLUDE_PREF_KEY` | `download_new_categories_exclude` (set) | `DownloadPreferences` |
| `DOWNLOAD_NEW_ANIME_CATEGORIES_EXCLUDE_PREF_KEY` | `download_new_anime_categories_exclude` (set) | `DownloadPreferences` |

Category id 0 is the implicit "Default" category (the library's uncategorised group) and has no row in the backup category list; ids stored as strings in the sets can therefore include `"0"`, which the restorer discards (no mapping). Proposal: map `"0"` to `"0"` explicitly.

## 7. Legacy keys the importer must understand (not restored by Kotlin as such)

Aniyomi rewrites some old preference keys through migrations (`app/src/main/java/mihon/core/migration/migrations/`), which run after a restore only if the restored `last_version_code` is lower; `last_version_code` is app-state and never in a backup, so the migrations run on the device's own version counter. Consequences for Kurobayomi: an old Tachiyomi/Aniyomi backup can contain legacy keys that no longer exist in the registry; they are kept in `[extra]` and a `legacy.rs` table maps the ones that matter: `anime_extension_repos` and `extension_repos` (string sets of repo base URLs, replaced by DB tables; imported into the repo tables by area:20-extension-strategy-redesign), `bottom_nav_style` (legacy of `bottom_rail_nav_style`), and the `secure_screen` boolean (legacy of `secure_screen_v2`; true maps to `SecureScreenMode.ALWAYS`). Per-key semantics of other legacy keys are in `plans/catalogs/preferences-01.md` to `preferences-08.md`.

## 8. Settings of Kurobayomi that Aniyomi cannot read

Kurobayomi-only keys (anything not in the registry) are exported in the same `BackupPreference` list with a `kuro_` key prefix (for example `kuro_theme_name`). Aniyomi's restorer writes them into its SharedPreferences, where they are never read (harmless, one stray key each). They are marked as normal-namespace so they follow the "App settings" option. The importer re-reads them in Kurobayomi.

## 9. Sync

Aniyomi's cloud/sync feature (`isSync` flag in `BackupRestorer`) calls the same preference restorer; the Rust import above is idempotent: importing the same list twice leaves the same `config.toml` (sets are unions, scalars are overwritten).
