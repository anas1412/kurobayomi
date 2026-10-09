# Area 13 — behaviour specification

Every section gives: OBSERVED (what the Kotlin does, with file:line), then PROPOSAL (what Rust does) where they differ. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Constants are quoted exactly. Task ids are in `p1-tasks.md`.

Conventions used below:
- "Kotlin String" means UTF-16 code units. Every "character" operation in this area counts UTF-16 code units unless it says "code point" or "UTF-8 byte".
- "Java compare" means the `int` returned by a Java/Kotlin comparator, not its sign only, where the sign is all that matters the spec says so.

---

## 1. Preference system (`core/common/src/main/java/tachiyomi/core/common/preference/`)

### 1.1 Key model

OBSERVED
- `PreferenceStore` (`PreferenceStore.kt:79-101`) exposes typed factories with defaults: `getString(key, defaultValue = "")`, `getLong(key, defaultValue = 0)`, `getInt(key, defaultValue = 0)`, `getFloat(key, defaultValue = 0f)`, `getBoolean(key, defaultValue = false)`, `getStringSet(key, defaultValue = emptySet())`, `getObject(key, defaultValue, serializer, deserializer)`, `getAll(): Map<String, *>`.
- `getEnum` (`PreferenceStore.kt:103-119`) stores `it.name` (the enum constant name, e.g. `None`, `Tracker`) through `getObject`. Deserialization is `enumValueOf(it)` and on `IllegalArgumentException` it returns the default.
- Key prefixes (`Preference.kt:29-48`):
  - `isPrivate(key)` = `key.startsWith("__PRIVATE_")`; `privateKey(key)` = `"__PRIVATE_" + key`.
  - `isAppState(key)` = `key.startsWith("__APP_STATE_")`; `appStateKey(key)` = `"__APP_STATE_" + key`.
- Backup filters (other area, recorded here because they define the meaning of the prefixes): `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt:62` drops every key with `Preference.isAppState`; `:82` drops every key with `Preference.isPrivate`.

PROPOSAL
- Keep the two prefixes byte-identical (`__PRIVATE_`, `__APP_STATE_`) so that a backup written by Aniyomi and read by Kurobayomi keeps the same classification. Task T-13a-005.

### 1.2 Read, write, delete, change (AndroidPreference.kt)

OBSERVED (`AndroidPreference.kt:57-110`)
- `get()` (`:72-80`): `try { read(preferences, key, defaultValue) } catch (e: ClassCastException) { logcat { "Invalid value for $key; deleting" }; delete(); defaultValue }`.
  - The ClassCastException path only occurs for the six primitive classes (`StringPrimitive`, `LongPrimitive`, `IntPrimitive`, `FloatPrimitive`, `BooleanPrimitive`, `StringSetPrimitive`) when the stored type differs from the requested type. The stored value is deleted.
  - `Object<T>.read` (`:222-228`) catches every `Exception` and returns the default **without deleting** the stored value.
- `read` per primitive:
  - `StringPrimitive`: `preferences.getString(key, defaultValue) ?: defaultValue` (`:118-124`).
  - `LongPrimitive`: `getLong`; `IntPrimitive`: `getInt`; `FloatPrimitive`: `getFloat`; `BooleanPrimitive`: `getBoolean` (`:137-187`).
  - `StringSetPrimitive`: `getStringSet(key, defaultValue) ?: defaultValue` (`:201-207`).
  - `Object<T>`: `preferences.getString(key, null)?.let(deserializer) ?: defaultValue` inside try/catch (`:222-228`).
- `set(value)` = `preferences.edit(action = write(key, value))` (`:82-84`). Android `edit` calls `apply()`, which writes asynchronously.
- `isSet()` = `preferences.contains(key)` (`:86-88`).
- `delete()` = `preferences.edit { remove(key) }` (`:90-94`).
- `defaultValue()` returns the constructor default (`:96-98`).
- `changes(): Flow<T>` (`:100-106`): `keyFlow.filter { it == key || it == null }.onStart { emit("ignition") }.map { get() }.conflate()`. A `null` key from the listener means "all keys may have changed" (`SharedPreferences.clear()`).
- `stateIn(scope)` (`:108-110`): `changes().stateIn(scope, SharingStarted.Eagerly, get())`.
- `AndroidPreferenceStore.getAll()` returns `sharedPreferences.all ?: emptyMap()` (`AndroidPreferenceStore.kt:63-65`).
- `keyFlow` (`AndroidPreferenceStore.kt:68-79`) registers an `OnSharedPreferenceChangeListener` in `callbackFlow` and unregisters in `awaitClose`. Each callback sends the changed key (possibly `null`).

PROPOSAL
- `Pref<T>::get()`: read from the store; for typed primitive kinds, a value of the wrong type is removed and the default returned (same as the Kotlin). For enum/object kinds, a decode failure returns the default and keeps the stored value (same). Task T-13a-002.
- `Pref<T>::set()` writes synchronously to the store (SQLite or file), returns `Result`. The Kotlin `apply()` is fire-and-forget; Rust reports errors (no silent loss). Task T-13a-002.
- `Pref<T>::changes()` = `tokio::sync::watch::Receiver<T>`. `watch` conflates (equivalent to `conflate()`), starts with the current value (equivalent to `onStart { emit("ignition") }`), and is fed by a broadcast of changed keys. A `clear` event is modelled as "all keys changed". Task T-13a-002.
- The Kotlin keeps the Flow hot for the process lifetime. Rust keeps one `watch` sender per key in a `HashMap` guarded by `Mutex` (see p1-rust-design.md).

### 1.3 Enum preference (`getEnum`)

OBSERVED: name storage, default on unknown name, stored value kept on unknown name (because `Object.read` returns default on exception only for the deserializer path; an unknown name makes `enumValueOf` throw, which `getEnum` catches and maps to the default, so nothing is deleted).

PROPOSAL: `EnumPref<E>` with `serde`-free name mapping: `E::name()` stored as UTF-8 string; `E::from_name(&str) -> Option<E>`; unknown name returns default and keeps the stored value. Task T-13a-004.

### 1.4 Helpers (Preference.kt:52-76)

- `getAndSet(block)` = `set(block(get()))`.
- `deleteAndGet()` = `delete(); get()`.
- `Preference<Set<T>>.plusAssign(item)` = `set(get() + item)`; `plusAssign(items: Iterable<T>)` = `set(get() + items)`; `minusAssign(item)` = `set(get() - item)`. Each creates a new set (value semantics).
- `Preference<Boolean>.toggle()`: `set(!get()); return get()`.

PROPOSAL: the same four operations as methods on `Pref<T>` and `Pref<BTreeSet<String>>`. Task T-13a-007.

### 1.5 In-memory store (InMemoryPreferenceStore.kt)

OBSERVED
- `getX(key, default)` builds `InMemoryPreference(key, null, default)` and, when the initial map has a value of the same Kotlin type (`as? Long`, `as? Int`, `as? Float`, `as? Boolean`, `as? String`, `as? Set`), returns `InMemoryPreference(key, data, default)`.
- `InMemoryPreference.set(value)` only changes the object's own field (`data = value`). The store's map is not updated, so a second `getX` from the same store returns the initial value. Observed defect.
- `InMemoryPreference.changes()` = `flow { data }` emits the value once and never again.
- `getStringSet` = `TODO("Not yet implemented")` which throws `NotImplementedError`.
- `isSet()` = `data != null`; `delete()` sets `data = null`.

PROPOSAL: the test store writes through to its map and implements `getStringSet`. The write-through defect is fixed because tests depend on it. Task T-13a-003.

### 1.6 TriState and CheckboxState

- `enum class TriState { DISABLED, ENABLED_IS, ENABLED_NOT }` with `next()`: `DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED` (`TriState.kt:122-135`).
- `sealed class CheckboxState<T>(value)` (`CheckboxState.kt:82-114`):
  - `State<T>`: `Checked`, `None`; `isChecked` is `this is Checked`; `next()`: `Checked -> None`, `None -> Checked`.
  - `TriState<T>`: `Include`, `Exclude`, `None`; `next()`: `Exclude -> None`, `Include -> Exclude`, `None -> Include`. The cycle is `None -> Include -> Exclude -> None`.
- `asCheckboxState(condition)`: `Checked(this)` when the condition holds, else `None(this)`. `mapAsCheckboxState(condition)` maps a list.

PROPOSAL: `enum TriState { Disabled, EnabledIs, EnabledNot }` with `next()`. `CheckboxState<T>` becomes two enums with `next()` (`Checked|None` and `Include|Exclude|None`). Task T-13a-006.

---

## 2. Text helpers (`eu/kanade/tachiyomi/util/lang/StringExtensions.kt`, `tachiyomi/core/common/util/lang/SortUtil.kt`)

### 2.1 chop (`StringExtensions.kt:56-62`)

OBSERVED: `if (length > count) take(count - replacement.length) + replacement else this`. Default replacement is `"…"` (U+2026, one code unit). If `replacement.length > count` and the string is longer than `count`, `take(negative)` throws `IllegalArgumentException`.

PROPOSAL: same; the Rust function returns `Err(TextError::ReplacementTooLong)` in that case instead of panicking. Count is UTF-16 units (`encode_utf16().count()`), output cut at a UTF-16 boundary (never split a surrogate pair: if the cut lands on a low surrogate, cut one unit earlier). Task T-13a-008.

### 2.2 truncateCenter (`StringExtensions.kt:68-76`)

OBSERVED
- If `length <= count` return this.
- `pieceLength = floor((count - replacement.length).div(2.0)).toInt()`. Default replacement `"..."` (three ASCII dots).
- Result: `take(pieceLength) + replacement + takeLast(pieceLength)`.
- Result length is `2 * pieceLength + replacement.length`, which is `<= count`. With `count < replacement.length`, `pieceLength` is negative; `take(negative)` throws.

PROPOSAL: identical; with `count < replacement.length` return `Err`. Task T-13a-008.

### 2.3 Case-insensitive natural compare (`compareToCaseInsensitiveNaturalOrder`, `StringExtensions.kt:81-84`)

OBSERVED: delegates to `CaseInsensitiveSimpleNaturalComparator` from `net.greypanther.natsort` (`gpanther/java-nat-sort` 1.1, `natural-comparator-1.1` tag, dependency `libs.natural.comparator` in `gradle/libs.versions.toml`). The library source (fetched for this plan) is:

```
abstract compare(T seq1, T seq2):
  len1, len2; idx1 = 0, idx2 = 0
  while idx1 < len1 && idx2 < len2:
    c1 = seq1[idx1++]; c2 = seq2[idx2++]
    d1 = isDigit(c1); d2 = isDigit(c2)          // isDigit: '0' <= c && c <= '9'
    if d1 && !d2: return -1
    else if !d1 && d2: return 1
    else if !d1 && !d2:
        c = compareChars(c1, c2)                 // case-insensitive: Character.toLowerCase(c1) - Character.toLowerCase(c2)
        if c != 0: return c
    else:                                        // both digits
        num1 = c1 - '0'
        while idx1 < len1:
            digit = seq1[idx1++]
            if isDigit(digit): num1 = num1*10 + (digit - '0')     // long arithmetic, wraps on overflow
            else: idx1--; break
        num2 = c2 - '0'  (same loop on seq2)
        if num1 != num2: return compareUnsigned(num1, num2)       // unsigned 64-bit compare
  if idx1 < len1: return 1
  else if idx2 < len2: return -1
  return 0
```

Notes that matter for a port:
- Digits sort before non-digits at the position where they first differ (`"1" < "a"`, and `"a" > "1"`).
- Numeric runs compare by value, so `"file2" < "file10"`. Leading zeros do not change the value, so `"01"` equals `"1"` numerically and the scan continues with the characters after the run. Example: `"01a"` vs `"1b"` compares `a` with `b`.
- Numeric runs longer than 19 digits overflow silently (`long` wraps). The port must reproduce wrapping (`wrapping_mul`, `wrapping_add`) and unsigned compare (`u64`).
- Comparison is per UTF-16 code unit. `Character.toLowerCase(char)` is Java's simple per-char lowercase. Rust port uses `char::to_lowercase` on the BMP code unit when it is a scalar value, and falls back to the unit itself for lone surrogates.
- The return value is a difference, not just a sign. Callers use it as a comparator, so only the sign matters for sorting.

PROPOSAL: a 60-line function `natural_cmp_ci(a: &[u16], b: &[u16]) -> Ordering` that is a line-by-line port, plus the test vectors in p1-edge-cases-and-tests.md. Do not use the `natord` crate (it is case-sensitive by default and its digit rules differ). Task T-13a-009.

Callers (for ordering consequences): `ArchivePageLoader.kt` sorts archive image entries with it (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ArchivePageLoader.kt:18`). Other callers: 8 files reported by the caller scan in p1-inventory.md.

### 2.4 Collator (`SortUtil.kt:1-15`)

OBSERVED: `Collator.getInstance(Locale.getDefault())` with `strength = Collator.PRIMARY`, created lazily, shared. `compareToWithCollator` returns `collator.compare(this, other)`. Primary strength ignores case and diacritics: `"a" == "A" == "á"` at this level.

Callers: chapter sort by name (`domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt`, the `CHAPTER_SORTING_ALPHABET` branch uses `compareToWithCollator`), episode and season sorters, library screen models.

PROPOSAL: `icu_collator::Collator` with `CollatorOptions { strength: Some(Strength::Primary), .. }` and the locale from the environment (`LANG` / `LC_COLLATE`, via kuro-platform). Fallback to `en` when the locale is unavailable. Task T-13a-010.

### 2.5 byteSize and takeBytes (`StringExtensions.kt:89-104`)

OBSERVED
- `byteSize()` = `toByteArray(UTF_8).size`.
- `takeBytes(n)`: if the UTF-8 length is `<= n`, return this. Else `bytes.decodeToString(endIndex = n).replace("�", "")`.
- `decodeToString(endIndex = n)` decodes the first `n` bytes; a sequence cut in the middle becomes U+FFFD. The `replace` then removes **every** U+FFFD in the result, including any the original string contained. Observed defect (rare).

Consumers: `ReaderViewModel.kt` (page filename helper, near line 757) builds the page filename as `DiskUtil.buildValidFilename("${manga.title} - ${chapter.name}".takeBytes(DiskUtil.MAX_FILE_NAME_BYTES - filenameSuffix.byteSize())) + filenameSuffix` with `filenameSuffix = " - ${page.number}"`. `PlayerViewModel.kt:2021` does the same.

PROPOSAL: same byte budget; cut at a UTF-8 char boundary (`str::is_char_boundary`), no U+FFFD stripping (it removes legitimate characters). Task T-13a-011.

### 2.6 htmlDecode (`StringExtensions.kt:109-111`)

OBSERVED: `this.parseAsHtml().toString()`. `androidx.core.text.parseAsHtml` with the default `HtmlCompat.FROM_HTML_MODE_LEGACY`: parses HTML, decodes entities (HTML 4 named entities and numeric references), converts `<br>` to `\n`, drops tags and their attributes, and returns plain text. Callers: `app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURecord.kt`, `.../track/anilist/dto/ALAnime.kt`, `.../track/anilist/dto/ALManga.kt` (tracker title decoding).

PROPOSAL: Rust function `html_decode_legacy(s)`: decode HTML 4 entities via `html-escape` (or a table) plus numeric references, replace `<br>`/`<br/>`/`<br />` (case-insensitive) with `\n`, strip `<...>` tags with a small state machine, do not interpret scripts or styles. Known difference: Android's legacy parser also handles block elements by inserting newlines; the Rust port inserts `\n` only for `<br>` and `</p>`, `</div>`, `</li>` (documented deviation, verified against sample tracker payloads in tests). Task T-13a-012.

### 2.7 Hash helpers (`Hash.kt`)

OBSERVED
- `sha256(bytes)` and `md5(bytes)` return lowercase hex of the digest (`encodeHex` with table `0-9a-f`, high nibble first).
- `sha256(string)` / `md5(string)` use `string.toByteArray()` (UTF-8 on Android).
- `MessageDigest.getInstance` is called per call (no sharing). Cost is irrelevant for the callers.

Callers: `DiskUtil.hashKeyForDisk` (`md5`), `MangaExtensionLoader.kt:420` and `AnimeExtensionLoader.kt` (`Hash.sha256(it.toByteArray())` on signatures).

PROPOSAL: same outputs (`md-5` and `sha2` crates, hand-written hex). Task T-13a-013.

---

## 3. Storage (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/`, `tachiyomi/core/common/storage/`)

### 3.1 Filename sanitiser: DiskUtil.buildValidFilename (DiskUtil.kt:220-236, 241-249)

OBSERVED, in order:
1. `name = origName.trim('.', ' ')` removes leading and trailing `.` and space characters only (not tabs or other whitespace).
2. If `name` is empty, return `"(invalid)"`.
3. Each char `c` of `name` is kept when `isValidFatFilenameChar(c)` is true, else replaced with `_`.
   - Invalid: `0x00..0x1f` (control chars), and `"`, `*`, `/`, `:`, `<`, `>`, `?`, `\`, `|`, and `0x7f`.
   - Everything else is valid, including non-ASCII letters, emoji, and surrogate halves.
4. `return sb.toString().take(240)`. The comment says "Even though vfat allows 255 UCS-2 chars ... use that limit minus 15 reserved characters". `take` counts UTF-16 units, so a surrogate pair can be split at unit 240.
5. Trailing dots and spaces that appear after step 3 or after the `take` are **not** removed.
6. Leading dots are removed (so hidden files cannot be produced) but a leading `_` created by replacement is kept.

Missing from the Kotlin (Windows and Linux safety, required by the cross-platform rule in `plans/addenda/requirements-cross-platform.md` rule 5):
- Windows reserved names `CON, PRN, AUX, NUL, COM1-COM9, LPT1-LPT9` (with or without an extension) are not renamed.
- The 255 UTF-16 unit limit for NTFS is not enforced separately (240 is the only limit).

PROPOSAL (`kuro-core::storage::filename::sanitize_component`):
1. Same trim and replacement rules (identical output for ASCII input, so existing download folders keep their names).
2. After replacement, if the name (case-insensitive, before the first `.`) equals a Windows reserved device name, prefix `_`.
3. Trim trailing `.` and space again after truncation.
4. Truncate by UTF-16 unit count to 240, then move back to a char boundary if the cut splits a surrogate pair.
5. Empty result gives `(invalid)`.
Task T-13a-014. The test vectors for this function are in p1-edge-cases-and-tests.md.

### 3.2 Byte budget constant (DiskUtil.kt:253-254)

OBSERVED: `const val MAX_FILE_NAME_BYTES = 250` with the comment "Safe theoretical max filename size is 255 bytes and 1 char = 2-4 bytes (UTF-8)". Used only as the budget in `takeBytes` (see 2.5). Note: 250 is the budget for the **base name without the extension**, the extension `.cbz` or `.jpg` is added later, so the total can exceed 255 by the suffix length; the Kotlin accepts this.

PROPOSAL: keep 250 as the budget constant `MAX_FILE_NAME_BYTES` for parity, and make the total (base + suffix + extension) check explicit in the Rust code, because Linux (ext4) and NTFS both reject a 256-byte component. Task T-13a-014.

### 3.3 Free and total space (DiskUtil.kt:162-193)

OBSERVED
- `getTotalStorageSpace(file)`: `StatFs(file.absolutePath)`; returns `blockCountLong * blockSizeLong`; any `Exception` returns `-1L`.
- `getAvailableStorageSpace(file: File)`: `availableBlocksLong * blockSizeLong`; `-1L` on exception.
- `getAvailableStorageSpace(f: UniFile)`: `StatFs(f.uri.path)` (only works for file-backed URIs; a SAF `content://` URI has a `path` such as `/tree/...` that does not exist, so this returns `-1L` in practice). The `NullPointerException` on a null path is also caught.

PROPOSAL: `statvfs` via `rustix::fs::statvfs` on the path; `available = f_bavail * f_frsize` (non-root blocks, matching `availableBlocksLong`), `total = f_blocks * f_frsize`. Return `Option<u64>` (None replaces -1). The UniFile overload disappears because all paths are plain. Task T-13a-016.

Consumer: `MangaDownloader.kt:337` checks available space before downloading (`DiskUtil.getAvailableStorageSpace(mangaDir)`).

### 3.4 Directory size (DiskUtil.kt:147-157)

OBSERVED: recursive; a directory adds the sizes of its children (`listFiles().orEmpty()`); a file adds `length()`. Symlinks are followed (`File.listFiles`, `File.length`), so a symlink loop recurses until stack overflow. Observed defect.

PROPOSAL: iterative walk with `walkdir` (or an explicit stack), `follow_links(false)`, and a visited set on device+inode to be safe. Counts regular files only (`metadata().len()` of files; directory entries count 0). Task T-13a-016.

Consumers: `ChapterCache.kt:51` (`DiskUtil.getDirectorySize(cacheDir)`) for the reader cache size.

### 3.5 .nomedia and media scan (DiskUtil.kt:195-213)

OBSERVED
- `createNoMediaFile(dir, context)`: if `dir != null && dir.exists()` and `dir.findFile(".nomedia") == null`, call `dir.createFile(".nomedia")` and, when `context != null`, `scanMedia(context, dir.uri)`. Return value of `createFile` is ignored.
- `scanMedia(context, uri)` = `MediaScannerConnection.scanFile(context, arrayOf(uri.path), null, null)`.

Callers: `MangaDownloader.kt:432` (`DiskUtil.createNoMediaFile(tmpDir, context)`) for every new chapter download folder.

PROPOSAL: `create_nomedia(dir: &Path) -> io::Result<()>` creates the empty file `.nomedia` when the directory exists and the file is absent; no scan step on Linux (the file indexer is out of scope). Keep writing `.nomedia` so that backups copied from Android stay recognisable. Open question: whether GNOME Tracker or KDE Baloo honour `.nomedia` (not verified; do not claim they do). Task T-13a-018.

### 3.6 External storage enumeration (DiskUtil.kt:129-141)

OBSERVED: `ContextCompat.getExternalFilesDirs(context, null)`, root = `absolutePath.substringBefore("/Android/")`, kept when `Environment.getExternalStorageState(root)` is `MEDIA_MOUNTED` or `MEDIA_MOUNTED_READ_ONLY`. Consumer: `SettingsDataScreen` storage picker (Android SD cards).

PROPOSAL: Drop. The folder chooser is the GTK file dialog (`gtk::FileDialog::select_folder`, or the portal via `ashpd`). Task T-13a-056.

### 3.7 hashKeyForDisk (DiskUtil.kt:143-145)

OBSERVED: `Hash.md5(key)` (lowercase hex of UTF-8 bytes of the key). Used for cache file names: `ChapterCache.kt:67,90,118,132,150` and `MangaCoverCache.kt:40,51`.

PROPOSAL: same function (keeps cache paths reproducible if a cache directory is copied from Android). Task T-13a-013.

### 3.8 FAT and MIME-safe file copies: copyAndSetReadOnlyTo (FileExtensions.kt:52-91)

OBSERVED, in order:
1. If `!this.exists()`: throw `NoSuchFileException(file = this, reason = "The source file doesn't exist.")`.
2. If `target.exists()`:
   - `overwrite == false`: throw `FileAlreadyExistsException(file=this, other=target, reason="The destination file already exists.")`.
   - `overwrite == true` and `!target.delete()`: throw `FileAlreadyExistsException(..., reason="Tried to overwrite the destination, but failed to delete it.")`.
3. If `this.isDirectory`: `target.mkdirs()` must succeed or throw `FileSystemException(file=this, other=target, reason="Failed to create target directory.")`. **No content is copied for directories.**
4. Else: `target.parentFile?.mkdirs()` (result ignored); `this.inputStream().use { input -> target.outputStream().use { output -> target.setReadOnly(); input.copyTo(output, bufferSize) } }`.
   - `setReadOnly()` runs **before** the copy, inside the output stream block. A failure mid-copy leaves a read-only partial file.
   - `setReadOnly()`'s boolean result is ignored.
5. Return `target`.

Callers: `AnimeExtensionLoader.kt` and `MangaExtensionLoader.kt` (installing extension packages; the copy protects the installed file from modification).

PROPOSAL (behaviour change, flagged): copy into `target.tmp` then `rename` to `target`, then set the permission to read-only (`0o444`). This avoids a read-only partial file. Directories: return `Err(IsADirectory)` unless the caller asked for a directory copy (no caller does). Task T-13a-020.

### 3.9 Folder provider (AndroidStorageFolderProvider.kt, FolderProvider.kt)

OBSERVED: `directory() = File(Environment.getExternalStorageDirectory().absolutePath + File.separator + context.stringResource(MR.strings.app_name))`. `app_name` = `Aniyomi` (`i18n/src/commonMain/moko-resources/base/strings.xml:3`, `translatable="false"`). `path() = directory().toUri().toString()`, which yields `file:///storage/emulated/0/Aniyomi`.

PROPOSAL: `FolderProvider::directory()` returns the default data root from the `Paths` trait of kuro-platform (see p1-data-and-config.md for the Linux paths). `path()` returns the `file://` URL of that directory (`url::Url::from_file_path`). App name: `Kurobayomi`. Task T-13a-015.

### 3.10 UniFile name helpers (UniFileExtensions.kt, storage package)

OBSERVED
- `extension = name?.substringAfterLast('.')`: when the name has no `.`, `substringAfterLast` returns the whole name. So `"README"` has extension `"README"`. For `"a.tar.gz"` it is `"gz"`. For `".hidden"` it is `"hidden"`.
- `nameWithoutExtension = name?.substringBeforeLast('.')`: when there is no `.`, returns the whole name; `".hidden"` gives `""`.
- `displayablePath = filePath ?: uri.toString()`.

PROPOSAL: the same results when a caller needs them, but prefer `Path::extension()` (which returns `None` with no dot) at new call sites so the quirk does not spread. The quirk is kept only inside the helpers used by the ported UI text (task T-13a-017).

### 3.11 Temporary file manager (UniFileTempFileManager.kt)

OBSERVED
- `dir = File(context.externalCacheDir, "tmp")`.
- `createTempFile(file)`: `dir.mkdirs()`; `inputStream = contentResolver.openInputStream(file.uri)!!` (NPE if null); `File.createTempFile(file.nameWithoutExtension.orEmpty().padEnd(3), null, dir)` (prefix padded with spaces to at least 3 chars, suffix `.tmp`); copy with `FileUtils.copy(inputStream, tempFile.outputStream())` on API 29 and up, else an 8192-byte loop inside `use` blocks. The API 29 branch never closes the streams.
- `deleteTempFiles()`: `dir.deleteRecursively()`.
- No caller in this repository uses the class (the only match is its own definition), so it is dead code in this snapshot.

PROPOSAL: keep a `TempFiles` helper with `create(source: &Path) -> io::Result<NamedTempFile>` (`tempfile` crate, prefix padded to 3 bytes, suffix `.tmp`, directory `<cache>/tmp`) and `clear()`. Do not port the unused call path unless a feature needs it. Task T-13a-019. Note for the implementer: flagged as dead code, the user asked that unrelated dead code is not deleted in the source, but this is a new Rust module, so it is not created unless needed.

### 3.12 Default folder (see 3.9) and external-cache location

OBSERVED: `context.externalCacheDir` (app-specific external cache). PROPOSAL: `Paths::cache()/tmp`.

---

## 4. Archives (`core/archive/`)

### 4.1 Opening and listing (ArchiveReader.kt, ArchiveInputStream.kt)

OBSERVED
- `ArchiveReader(pfd: ParcelFileDescriptor)`: `size = pfd.statSize`; `address = Os.mmap(0, size, PROT_READ, MAP_PRIVATE, pfd.fileDescriptor, 0)`. The whole archive is mapped read-only.
- `useEntries(block)`: opens a fresh `ArchiveInputStream(address, size)`, passes `generateSequence { it.getNextEntry() }` to `block`, closes the stream after `block` returns (`use`).
- `ArchiveInputStream` init (`ArchiveInputStream.kt:18-28`):
  1. `Archive.readNew()`.
  2. `Archive.setCharset(archive, "UTF-8")` (sets the default charset for entry names).
  3. `Archive.readSupportFilterAll(archive)` (every compression filter compiled into the libarchive AAR; the complete list is the AAR's own build configuration, not in this repository, OQ-13a-6).
  4. `Archive.readSupportFormatAll(archive)` (every format compiled into the libarchive AAR; the complete list is the AAR's build configuration, OQ-13a-6).
  5. `Archive.readOpenMemoryUnsafe(archive, buffer, size)`.
  6. On `ArchiveException` in init, `close()` and rethrow.
- `getNextEntry()`: `Archive.readNextHeader(archive)`; `0L` (end) yields `null`. Name: `ArchiveEntry.pathnameUtf8(entry) ?: ArchiveEntry.pathname(entry)?.decodeToString() ?: return null`. `isFile = ArchiveEntry.filetype(entry) == AE_IFREG`.
  - Note the `?: return null` inside the lambda returns from `getNextEntry`, which ends iteration early for a nameless entry. Observed.
- `read(b, off, len)` reads through `Archive.readData(archive, buffer)` into a direct ByteBuffer and returns `buffer.remaining()` or `-1` when nothing was read. `read()` is one byte.
- `close()` is idempotent (`isClosed` guarded by `lock`) and calls `Archive.readFree`.

PROPOSAL
- `ArchiveReader::open(path)` maps the file with `memmap2::Mmap` (read only). The map is shared (`Arc<Mmap>`) by the reader and any entry stream.
- Each listing or entry read opens a new libarchive handle over the memory (same design as the Kotlin, so no shared mutable state). Use a libarchive binding; choose the binding during the implementation spike (task T-13a-031). The format and filter support set must match `readSupportFilterAll` and `readSupportFormatAll`.
- Entry names are UTF-8 (`pathnameUtf8`); a nameless entry is skipped, not treated as end of archive (deliberate deviation, recorded here).

### 4.2 Single entry by name (ArchiveReader.getInputStream, ArchiveReader.kt:18-33)

OBSERVED: scan entries from the start; return the first `ArchiveInputStream` whose `entry.name == entryName` (exact string comparison, case-sensitive, no normalisation). If the scan throws `ArchiveException`, close and rethrow. If not found, close and return `null`. The returned stream is positioned at the data of that entry.

PROPOSAL: same, returning `Result<Option<EntryReader>>`. The scan is O(entries) per call; `ArchivePageLoader` calls it once per page (`reader.getInputStream(entry.name)!!`), so a 300-page archive costs about 45,000 header reads. Recorded as a known ceiling; the optional index (name to header offset, built once per open) is a P2 follow-up (not a task in this area). Task T-13a-032.

### 4.3 Page filter for archives (ArchivePageLoader.kt:15-26, the consumer)

OBSERVED: `entries.filter { it.isFile && ImageUtil.isImage(it.name) { reader.getInputStream(it.name)!! } }`, then `sortedWith { f1, f2 -> f1.name.compareToCaseInsensitiveNaturalOrder(f2.name) }`, then indexes from 0. Entries whose extension is not an image extension are checked by sniffing the first 32 bytes (`ImageUtil.isImage` second argument). A sniff failure means the entry is skipped.

PROPOSAL: the same sequence in Rust. The sniff reads through the same libarchive reader (one extra header scan per non-image-extension entry, documented). Task T-13a-031.

### 4.4 UniFile to archive bridge (UniFileExtensions.kt, archive module)

OBSERVED: `openFileDescriptor(context, mode)` opens a `content://` or file URI with `contentResolver.openFileDescriptor(uri, mode) ?: error("Failed to open file descriptor: ...")`. `archiveReader(context)` opens with mode `"r"`, and `epubReader(context)` wraps it.

PROPOSAL: Drop (no SAF). `archive::open_path(&Path)` replaces all three functions. Task T-13a-035.

### 4.5 EPUB reader (EpubReader.kt)

OBSERVED (the complete algorithm)
1. `pathSeparator` is decided once in `getPathSeparator()` (`:95-103`): if the archive contains the entry `META-INF\container.xml` (backslash), the separator is `\`; else `/`. Entries written by some Windows tools use backslashes.
2. `getImagesFromPages()` (`:29-34`): `ref = getPackageHref()`, `doc = getPackageDocument(ref)`, `pages = getPagesFromDocument(doc)`, `return getImagesFromPages(pages, ref)`.
3. `getPackageHref()` (`:39-49`): reads `resolveZipPath("META-INF", "container.xml")`. If the stream exists, parses it with `Jsoup.parse(it, null, "")` (HTML parser) and returns the first `rootfile` element's `full-path` attribute if that attribute is non-null. Otherwise returns `resolveZipPath("OEBPS", "content.opf")`.
   - Observed: if `container.xml` exists but has no `rootfile` element, the code falls through to `OEBPS/content.opf`.
   - Observed: `getElementsByTag("rootfile").first()?.attr("full-path")`; `attr` returns `""` (not null) when the attribute is missing, so `path` is `""` and the function returns `""`. This is a defect: the package href becomes empty and the next read fails. Recorded; the port returns the fallback when the attribute is missing or empty.
4. `getPackageDocument(ref)` (`:54-56`): `getInputStream(ref)!!` parsed as HTML. A missing package throws `NullPointerException`.
5. `getPagesFromDocument(document)` (`:61-68`):
   - `pages = document.select("manifest > item").filter { media-type == "application/xhtml+xml" }.associateBy { id }`. Only XHTML items are considered, not `application/xhtml` or `text/html`.
   - `spine = document.select("spine > itemref").map { idref }`.
   - Returns `spine.mapNotNull { pages[it] }.map { href }`: spine order, skipping spine items that are not XHTML manifest entries.
6. `getImagesFromPages(pages, packageHref)` (`:73-90`):
   - `basePath = getParentDirectory(packageHref)`.
   - For each page: `entryPath = resolveZipPath(basePath, page)`; parse the page as HTML; `imageBasePath = getParentDirectory(entryPath)`.
   - For each element in `document.allElements` (document order): `img` contributes `resolveZipPath(imageBasePath, attr("src"))`; `image` contributes `resolveZipPath(imageBasePath, attr("xlink:href"))`.
   - No deduplication. Missing `src` gives `""`, which resolves to the base directory path (a defect: the result list can contain directory paths).
7. `resolveZipPath(basePath, relativePath)` (`:108-122`):
   - If `relativePath` starts with `pathSeparator`, return it unchanged (absolute in the archive).
   - `fixedBasePath = basePath.replace(pathSeparator, File.separator)`; prefix `File.separator` if it does not start with it.
   - `fixedRelativePath = relativePath.replace(pathSeparator, File.separator)`.
   - `resolvedPath = File(fixedBasePath, fixedRelativePath).canonicalPath`; then `resolvedPath.replace(File.separator, pathSeparator).substring(1)`.
   - Effect: a pure string normalisation of `base/relative` (`.` and `..` removed, duplicates collapsed) with the leading `/` removed. Symlinks do not exist in the zip, so `canonicalPath` is equivalent to lexical normalisation, except that `..` above the root is clamped at the root.
   - Note `relativePath` with a leading `/` is returned as is, and then the leading `/` is kept (not removed by `substring(1)`): an absolute path is returned with a leading slash, which will not match any archive entry name. Observed.
8. `getParentDirectory(path)` (`:127-134`): substring before the last `pathSeparator`, or `""` when none.
9. `getInputStream(entryName)` delegates to `ArchiveReader.getInputStream` (linear scan, see 4.2).

PROPOSAL
- Parse the container and package with an HTML5 parser (`scraper`, which is html5ever), because jsoup's behaviour with malformed XML (lower-casing tags, tolerant attribute parsing) is what the Kotlin depends on.
- Attribute `xlink:href`: html5ever stores the SVG `xlink:href` attribute with the namespace `xlink` and local name `href`. The port matches on prefix and local name together (`xlink` + `href`), not on the string `href`.
- Implement the separator detection, the path normalisation (pure string code, clamp `..` at root, keep the absolute-path rule), and the fallback to `OEBPS/content.opf`.
- Fixes (deliberate, recorded): a missing or empty `full-path` falls back to `OEBPS/content.opf`; an `img` without `src` or an `image` without `xlink:href` is skipped; the result is deduplicated but keeps first-seen order.
- Task T-13a-033.

### 4.6 CBZ writer (ZipWriter.kt, consumer MangaDownloader.kt:634-647)

OBSERVED
- `ZipWriter(context, file)`: `pfd = file.openFileDescriptor(context, "wt")` (truncating write). `Archive.writeNew()`; `ArchiveEntry.new2(archive)`; direct buffer 8192.
- Init: `setCharset(UTF-8)`, `writeSetFormatZip`, `writeZipSetCompressionStore` (stored, no compression), `writeOpenFd(archive, pfd.fd)`.
- `write(file: UniFile)`: opens `file` for reading; clears the entry; `setPathnameUtf8(entry, file.name)` (the **basename**, not a path); `Os.fstat(fd)` copied into the entry (mode, uid, gid, nlink, rdev, size, blksize, blocks, atime, mtime, ctime, inode, dev); `writeHeader`; loop `Os.read(fd, buffer)` until `position == 0`, flip, `writeData`; `writeFinishEntry`.
- `close()`: `ArchiveEntry.free`, `writeFree`, `pfd.close()`.
- Caller: `archiveChapter` (`MangaDownloader.kt:634-647`): creates `"$dirname.cbz$TMP_DIR_SUFFIX"`, writes every entry of `tmpDir.listFiles()` (in listing order, not sorted), renames to `"$dirname.cbz"`, then `tmpDir.delete()`.

PROPOSAL
- Crate `zip` with `SimpleFileOptions::default().compression_method(CompressionMethod::Stored)`, `last_modified_time` from the source file mtime (clamped to the DOS epoch 1980-01-01 00:00:00 minimum, since ZIP cannot store earlier times; libarchive currently writes the Unix time into the extended field, so this is a documented difference), `unix_permissions` from the source mode & 0o7777.
- Entries are the basename, not a path (keep: CBZ readers expect a flat list).
- Order: sorted by file name (natural order, see 2.3) for determinism. This changes the Kotlin's unspecified order; the reader sorts on read anyway, so there is no user-visible difference.
- Write to `<name>.cbz_tmp` (the source's `TMP_DIR_SUFFIX` is `"_tmp"`, `MangaDownloader.kt:766`) then rename to `<name>.cbz` (same as caller). Task T-13a-034.

---

## 5. Image handling (`core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt`)

### 5.1 Magic bytes and sniffing (findType)

OBSERVED: the decoder is the native `tachiyomi.decoder.ImageDecoder` (library `image-decoder`, pinned to commit `41c059e540` in `gradle/libs.versions.toml:52`). Its format detection (`nativeFindType` in `library/src/main/cpp/java_wrapper.cpp` of `tachiyomiorg/image-decoder`, and `decoder_headers.h`) is, in this order, on the first **32** bytes:

| Test (bytes, zero-based offsets) | Format code | Format | Animated flag |
|---|---|---|---|
| `[0]=0xFF [1]=0xD8 [2]=0xFF` | 0 | Jpeg | false |
| `[0]=0x89 [1]='P' [2]='N' [3]='G'` | 1 | Png | false |
| `[0..3]="RIFF"` (then WebP parse; on failure assume not animated) | 2 | Webp | from WebP header (`info.isAnimated`), false on parse failure |
| `[0..3]="GIF8"` | 3 | Gif | always true |
| JXL container `[0..11] = 00 00 00 0C 'J' 'X' 'L' ' ' 0D 0A 87 0A`, or codestream `[0]=0xFF [1]=0x0A` | 6 | Jxl | false |
| ISO-BMFF `ftyp` at `[4..7]`, brand search (below) finds `hei?` (`h`,`e`,`i`|`v`) | 4 | Heif | false |
| same, brand `avi` | 5 | Avif | false |
| none | null | | |

ftyp brand search (`get_ftyp_image_type`): `headerSize = (b0<<24)|(b1<<16)|(b2<<8)|b3` (unsigned); `maxOffset = min(headerSize, size) - 4`; `offset = 8`; while `offset <= maxOffset`: test the 4-byte brand at `offset`; `offset += 4`. Note: JXL container check is before ftyp.

The Kotlin reads 32 bytes into a 32-byte array; a file shorter than 32 bytes leaves zeros in the tail (so short files usually fail to match, which is the intended outcome).

PROPOSAL: the same table and order in `kuro-core::image::sniff::sniff(&[u8]) -> Option<ImageSniff { format, animated }>`. For WebP the animated flag comes from the RIFF `VP8X` chunk's animation bit (flag 0x02 in the VP8X flags byte) because `image` exposes it. Task T-13a-021.

### 5.2 ImageUtil.ImageType and mapping (ImageUtil.kt:46-70, 106-114)

OBSERVED
- `findImageType(stream)` (`:50-65`): `getImageType(stream)?.format` mapped `Jpeg->JPEG, Png->PNG, Webp->WEBP, Gif->GIF, Heif->HEIF, Avif->AVIF, Jxl->JXL`; `else -> null`. Exceptions give `null`.
- `findImageType(openStream: () -> InputStream)` opens the stream and calls the other overload inside `use`.
- `getImageType(stream)` (`:89-104`): `bytes = ByteArray(32)`; if `stream.markSupported()`: `mark(32)`, `read(bytes, 0, 32)`, `reset()`; else `read(bytes, 0, 32)` (**consumes** the bytes). `length == -1` gives `null`. Otherwise `ImageDecoder.findType(bytes)`.
- Enum `ImageType(mime, extension)`:
  - `AVIF("image/avif", "avif")`
  - `GIF("image/gif", "gif")`
  - `HEIF("image/heif", "heif")`
  - `JPEG("image/jpeg", "jpg")`
  - `JXL("image/jxl", "jxl")`
  - `PNG("image/png", "png")`
  - `WEBP("image/webp", "webp")`
  - Note: `JPEG` maps to extension `jpg`, not `jpeg`.

### 5.3 isImage (ImageUtil.kt:39-44)

OBSERVED: `name == null` gives false. `extension = name.substringAfterLast('.')` (whole name when no dot). True when `ImageType.entries.any { it.extension == extension }` (case-sensitive: `PAGE.JPG` is not matched) **or** `openStream != null && findImageType(openStream) != null`.

So `"x.jpeg"` is an image only by sniffing; `"x.JPG"` only by sniffing; `"x"` (no extension) only by sniffing.

PROPOSAL: identical logic; the sniff path reads at most 32 bytes. Task T-13a-021.

### 5.4 getExtensionFromMimeType (ImageUtil.kt:67-70)

OBSERVED: `type = mime?.let { ImageType.entries.find { it.mime == mime } } ?: findImageType(openStream)`; return `type?.extension ?: "jpg"`. Mime matching is exact (case-sensitive, no parameters). Caller: `MangaDownloader.getImageExtension` (`:579-582`) passes `"image/$subtype"` only when the response `Content-Type` type is `image`; otherwise `null` (sniff).

PROPOSAL: identical; `image/jpg` (common, non-standard) does not match `image/jpeg` and falls back to sniff. Task T-13a-021.

### 5.5 isAnimatedAndSupported (ImageUtil.kt:72-87)

OBSERVED: `type = getImageType(source.peek().inputStream()) ?: return false`; then:
- `Gif` gives `true` (any GIF, even one frame).
- `Webp` gives `type.isAnimated && SDK_INT >= 28` (`VERSION_CODES.P`).
- `Heif` gives `type.isAnimated && SDK_INT >= 30` (`VERSION_CODES.R`).
- else `false`. Exceptions give `false`.

Callers: `PagerPageHolder.kt:152` (decides whether to compute a background), `ImageUtil.splitTallImage` (`:216`, animated images are never split).

PROPOSAL (Linux): `GIF` true; `WEBP` true when animated (the `image` crate decodes animated WebP frames); HEIF and AVIF animated: false (no animated decoder in the chosen stack; if animation is detected, the image is shown as its first frame and `isAnimated` is false for the purpose of this function). Task T-13a-023. This is a documented capability change, not a parity claim.

### 5.6 Wide image / double-page detection (ImageUtil.kt:121-124)

OBSERVED: `extractImageOptions(imageSource).outWidth > outHeight` where options come from `BitmapFactory.decodeStream(..., inJustDecodeBounds = true)` on `peek()` (no pixel decode). Callers: `PagerPageHolder.kt:198,209` (split/rotate decisions).

PROPOSAL: read the image header dimensions with `image::ImageReader::into_dimensions()` (no full decode). `width > height`. Task T-13a-024.

### 5.7 splitInHalf (ImageUtil.kt:129-148)

OBSERVED (`W` = bitmap width, `H` = height)
- Decode the full bitmap (`BitmapFactory.decodeStream`).
- `singlePage = Rect(0, 0, W/2, H)` (destination, integer division).
- `half = createBitmap(W/2, H)`.
- `part`: `RIGHT -> Rect(W - W/2, 0, W, H)` (width `W/2`, so no scaling); `LEFT -> Rect(0, 0, W/2, H)`.
- `drawBitmap(bitmap, part, singlePage, null)`; output JPEG quality **100**.
- For odd `W` the RIGHT part starts at `ceil(W/2)` and the middle column is dropped (`W = 5`: RIGHT is columns 3,4; LEFT is columns 0,1).

Callers: `PagerPageHolder.kt:219-234`. Side selection (`PagerPageHolder.kt:219-234`):
- `viewer is L2RPagerViewer && page is InsertPage -> RIGHT`
- `viewer !is L2RPagerViewer && page is InsertPage -> LEFT`
- `viewer is L2RPagerViewer && page !is InsertPage -> LEFT`
- `viewer !is L2RPagerViewer && page !is InsertPage -> RIGHT`
- If `dualPageInvert`, the side is swapped.

PROPOSAL: `split_in_half(img: &RgbaImage, side) -> RgbaImage` using `image::imageops::crop_imm` (exact integer boundaries as above). Output encoded as JPEG quality 100 (`JpegEncoder::new_with_quality(w, 100)`) to keep the same byte pipeline; a PNG output would be lossless but changes the cache files. Task T-13a-025.

### 5.8 rotateImage (ImageUtil.kt:150-163)

OBSERVED: decode; `Matrix().postRotate(degrees)`; `Bitmap.createBitmap(bitmap, 0, 0, W, H, matrix, filter = true)` (bilinear); output JPEG quality 100. Output size for ±90 is `H x W`. Alpha is discarded by JPEG encoding (transparent pixels become whatever the codec produces, usually black).

Callers: `PagerPageHolder.kt:209-213`: `rotation = if (dualPageRotateToFitInvert) -90f else 90f`, applied to a wide page when `dualPageRotateToFit` is on.

PROPOSAL: `rotate(img, Rotation::Cw90 | Ccw90)` for exact 90-degree turns (no resampling, identical pixels to bilinear at those angles); JPEG 100 output. Document that arbitrary angles are not needed by any caller. Task T-13a-026.

### 5.9 splitAndMerge (ImageUtil.kt:168-194)

OBSERVED: result is `createBitmap(W/2, 2*H)`.
- Let `upperSide` be the side placed on top.
- `rightPart = if upperSide == RIGHT then Rect(W - W/2, 0, W, H) else Rect(0, 0, W/2, H)`. Drawn to `upperPart = Rect(0, 0, W/2, H)`. So the top half is the side named by `upperSide`.
- `leftPart = if upperSide == LEFT then Rect(W - W/2, 0, W, H) else Rect(0, 0, W/2, H)`. Drawn to `bottomPart = Rect(0, H, W/2, 2*H)`. So the bottom half is the other side.
- Comments in the source ("right -> upper", "left -> bottom") describe the default `RIGHT` case only; the rule is the one above.
- Output JPEG quality 100.

Caller: `WebtoonPageHolder.kt:210-225` (`process`): if `dualPageSplit` and the page is wide, `upperSide = if dualPageInvert then LEFT else RIGHT`.

PROPOSAL: same rule; output `W/2 x 2H`. Task T-13a-027.

### 5.10 Tall-image split (ImageUtil.kt:201-316)

OBSERVED: the function `splitTallImage(tmpDir: UniFile, imageFile: UniFile, filenamePrefix: String): Boolean`:
1. `imageSource = imageFile.openInputStream().use { Buffer().readFrom(it) }` (whole file into memory).
2. If `isAnimatedAndSupported(imageSource) || !isTallImage(imageSource)` return `true` (nothing to do counts as success).
3. `isTallImage`: `(outHeight / outWidth) > 3` with **integer division**. This is true when `H / W >= 4`, i.e. `H >= 4W`. `outWidth == 0` would throw `ArithmeticException` (not caught in this function; the caller catches all exceptions).
4. `bitmapRegionDecoder = getBitmapRegionDecoder(imageSource.peek().inputStream())`. If `null`, log "Failed to create new instance of BitmapRegionDecoder" and return `false`.
   - `getBitmapRegionDecoder`: API 31 and up: `BitmapRegionDecoder.newInstance(stream)`; else `newInstance(stream, false)`.
5. `options = extractImageOptions(imageSource).apply { inJustDecodeBounds = false }`.
6. `splitDataList = options.splitData` (below).
7. For each `splitData`:
   - `splitImageName = "${filenamePrefix}__${"%03d".format(Locale.ENGLISH, index + 1)}.jpg"`. Example: prefix `007` gives `007__001.jpg`.
   - `tmpDir.findFile(splitImageName)?.delete()` (remove leftovers).
   - `splitFile = tmpDir.createFile(splitImageName)!!`.
   - `region = Rect(0, topOffset, splitWidth, bottomOffset)`.
   - `splitFile.openOutputStream().use { decodeRegion(region, options) -> compress JPEG 100; recycle }`.
   - log `"Success: Split #${index+1} with topOffset=... height=... bottomOffset=..."`.
8. On success: `imageFile.delete()`, return `true`.
9. On any exception (`catch (e: Exception)`): delete every split name for this `filenamePrefix` (`findFile(...)?.delete()`), log ERROR, return `false`. The original file is kept.
10. `finally { bitmapRegionDecoder.recycle() }`.

`splitData` (`:276-307`), with `H = outHeight`, `W = outWidth`, `optimalImageHeight = getDisplayMaxHeightInPx * 2`:
- `partCount = (H - 1) / optimalImageHeight + 1` (integer; `-1` so an image exactly `optimalImageHeight` tall is one part).
- `optimalSplitHeight = H / partCount` (integer).
- `range = 0 until partCount`. For each `index` in range:
  - if the list is not empty and `H <= last.bottomOffset`, break (defensive).
  - `topOffset = index * optimalSplitHeight`.
  - `splitHeight = min(optimalSplitHeight, H - topOffset)`.
  - if `index == range.last`: `splitHeight += H - (topOffset + splitHeight)` (the last part takes the remainder).
  - add `SplitData(index, topOffset, splitHeight, W)`. `bottomOffset = topOffset + splitHeight`.
- Worked example: `H = 10000`, `optimalImageHeight = 4320`: `partCount = 9999/4320 + 1 = 3`; `optimal = 3333`; parts `[0,3333)`, `[3333,6666)`, `[6666,10000)`; the last part is 3334 tall.

`getDisplayMaxHeightInPx` (`:727-728`) = `max(displayMetrics.heightPixels, widthPixels)` of the **device** display (not the window). So the optimal part height depends on the device, not on the window.

Caller: `MangaDownloader.kt:584-599` (`splitTallImageIfNeeded`): skipped when `downloadPreferences.splitTallImages()` is false (`split_tall_images`, default `true`); the page file is found by prefix `%03d` of the page number; a file whose name already starts with `prefix__` is skipped; the return value of `splitTallImage` is **ignored**; exceptions are logged.

PROPOSAL
- `kuro-core::image::split_tall(src: &Path, dst_dir: &Path, prefix: &str, optimal_height: u32) -> Result<Vec<PathBuf>, ImageError>`.
- `optimal_height` = the largest of the monitor's width and height (from GDK, device pixels, `gdk::Monitor::geometry()` of the monitor under the window at startup) times 2. Document: this is the monitor, not the window (same as Android).
- Decode once with `image` (no region decoder needed); crop with `crop_imm`; encode each part JPEG 100. Write to temporary names then rename (atomic per part). On failure delete all parts for the prefix and keep the original.
- The "animated" early exit: GIF always skipped (same).
- Return `Err` instead of the Kotlin's boolean; the caller logs and continues (same observable result).
- Task T-13a-028.

### 5.11 Hardware bitmap helpers (Drop)

OBSERVED: `canUseHardwareBitmap` (`:318-333`), `hardwareBitmapThreshold` (`:328`), `HARDWARE_BITMAP_UNSUPPORTED` (`:621-724`), `getDisplayMaxHeightInPx` (see 5.10).
- `hardwareBitmapThreshold` default `GLUtil.SAFE_TEXTURE_LIMIT` (2048); set from `App.kt:145-146` (`ImageUtil.hardwareBitmapThreshold = it`).
- `ReaderPageImageView.kt:256` uses it as `setMaxTileSize(ImageUtil.hardwareBitmapThreshold)` (subsampling tile size).
- `TachiyomiImageDecoder.kt:48` uses `ImageUtil.canUseHardwareBitmap(bitmap)` to decide `Bitmap.Config.HARDWARE`.

PROPOSAL: Drop the hardware bitmap path (GPU textures are managed by GTK; no Android HARDWARE config). The tile size concept maps to the reader's tiled renderer, owned by the reader area; this area supplies no value. Task T-13a-030.

### 5.12 Reader page background: chooseBackground (ImageUtil.kt:338-580)

This is the most detailed algorithm in the area. It is used only when `!isAnimated && viewer.config.automaticBackground` (`PagerPageHolder.kt:152-156`). The result is a `Drawable` that is painted behind the page.

OBSERVED: exact algorithm (names as in the source; `W`, `H` = decoded bitmap size; `px(x, y)` returns ARGB, `red`, `green`, `blue`, `alpha` are its 8-bit channels):

```
decode:
  decoder = ImageDecoder.newInstance(imageStream)   // cropBorders = false, displayProfile = null (sRGB)
  image = decoder?.decode()                          // full bitmap, ARGB_8888
  decoder?.recycle()
white = 0xFFFFFFFF   (Color.WHITE)
if image == null               -> return solid(white)
if W < 50 || H < 50            -> return solid(white)

top    = 5
bot    = H - 5
left   = trunc(W * 0.0275)          // Double -> Int truncation
right  = W - left
midX   = W / 2                     // integer
midY   = H / 2
offsetX = trunc(W * 0.01)
leftOffsetX  = left - offsetX
rightOffsetX = right + offsetX

topLeftPixel      = px(left,  top)
topRightPixel     = px(right, top)
midLeftPixel      = px(left,  midY)
midRightPixel     = px(right, midY)
topCenterPixel    = px(midX,  top)
botLeftPixel      = px(left,  bot)
bottomCenterPixel = px(midX,  bot)
botRightPixel     = px(right, bot)

topLeftIsDark, topRightIsDark, midLeftIsDark, midRightIsDark,
topMidIsDark (= topCenterPixel), botLeftIsDark, botRightIsDark   = isDark(...)

darkBG = ( topLeftIsDark && (botLeftIsDark || botRightIsDark || topRightIsDark || midLeftIsDark || topMidIsDark) )
      || ( topRightIsDark && (botRightIsDark || botLeftIsDark || midRightIsDark || topMidIsDark) )

topAndBotPixels = [topLeftPixel, topCenterPixel, topRightPixel, botRightPixel, bottomCenterPixel, botLeftPixel]
isNotWhiteAndCloseTo[i] = !isWhite(p[i]) && isCloseTo(p[i], p[(i+1) % 6])
if all six are true            -> return solid(topLeftPixel)     // uniform border colour

cornerPixels = [topLeftPixel, topRightPixel, botLeftPixel, botRightPixel]
numberOfWhiteCorners = count(isWhite(corner))
if numberOfWhiteCorners > 2    -> darkBG = false

blackColor = topLeftIsDark ? topLeftPixel
           : topRightIsDark ? topRightPixel
           : botLeftIsDark ? botLeftPixel
           : botRightIsDark ? botRightPixel
           : white

overallWhitePixels = 0; overallBlackPixels = 0
topBlackStreak = 0; topWhiteStreak = 0; botBlackStreak = 0; botWhiteStreak = 0

outer: for x in [left, right, leftOffsetX, rightOffsetX]:
  whitePixelsStreak = 0; whitePixels = 0
  blackPixelsStreak = 0; blackPixels = 0
  blackStreak = false; whiteStreak = false
  notOffset = (x == left || x == right)
  step = H / 25                           // integer, >= 2 because H >= 50
  inner: for (index, y) in (0 until H step step).withIndex():
    pixel    = px(x, y)
    pixelOff = px(x + (if x < W / 2 then -offsetX else +offsetX), y)
    if isWhite(pixel):
      whitePixelsStreak += 1; whitePixels += 1
      if notOffset: overallWhitePixels += 1
      if whitePixelsStreak > 14: whiteStreak = true
      if whitePixelsStreak > 6 && whitePixelsStreak >= index - 1:
          topWhiteStreak = whitePixelsStreak
    else:
      whitePixelsStreak = 0
      if isDark(pixel) && isDark(pixelOff):
        blackPixels += 1
        if notOffset: overallBlackPixels += 1
        blackPixelsStreak += 1
        if blackPixelsStreak >= 14: blackStreak = true
        continue inner                  // SKIP the two statements below
    if blackPixelsStreak > 6 && blackPixelsStreak >= index - 1:
        topBlackStreak = blackPixelsStreak
    blackPixelsStreak = 0
  // end inner
  if blackPixelsStreak > 6:      botBlackStreak = blackPixelsStreak
  else if whitePixelsStreak > 6: botWhiteStreak = whitePixelsStreak

  when:
    blackPixels > 22:
        if x == right || x == rightOffsetX:
            blackColor = topRightIsDark ? topRightPixel
                       : botRightIsDark ? botRightPixel
                       : blackColor
        darkBG = true
        overallWhitePixels = 0
        break outer
    blackStreak:
        darkBG = true
        if x == right || x == rightOffsetX:
            blackColor = (same expression as above)
        if blackPixels > 18:
            overallWhitePixels = 0
            break outer
    whiteStreak || whitePixels > 22:
        darkBG = false

topIsBlackStreak    = topBlackStreak > topWhiteStreak
bottomIsBlackStreak = botBlackStreak > botWhiteStreak
if overallWhitePixels > 9 && overallWhitePixels > overallBlackPixels: darkBG = false
if topIsBlackStreak && bottomIsBlackStreak:                            darkBG = true

if device orientation is landscape:
    return solid(darkBG ? blackColor : white)

botCornersIsWhite   = isWhite(botLeftPixel)  && isWhite(botRightPixel)
topCornersIsWhite   = isWhite(topLeftPixel)  && isWhite(topRightPixel)
topCornersIsDark    = topLeftIsDark && topRightIsDark
botCornersIsDark    = botLeftIsDark && botRightIsDark
topOffsetCornersIsDark = isDark(px(leftOffsetX, top)) && isDark(px(rightOffsetX, top))
botOffsetCornersIsDark = isDark(px(leftOffsetX, bot)) && isDark(px(rightOffsetX, bot))

gradient:
  darkBG && botCornersIsWhite          -> [blackColor, blackColor, white, white]
  darkBG && topCornersIsWhite          -> [white, white, blackColor, blackColor]
  darkBG                               -> return solid(blackColor)
  topIsBlackStreak || (topCornersIsDark && topOffsetCornersIsDark && (topMidIsDark || overallBlackPixels > 9))
                                       -> [blackColor, blackColor, white, white]
  bottomIsBlackStreak || (botCornersIsDark && botOffsetCornersIsDark && (bottomCenterPixel.isDark() || overallBlackPixels > 9))
                                       -> [white, white, blackColor, blackColor]
  else                                 -> return solid(white)

return GradientDrawable(TOP_BOTTOM, gradient)   // 4-stop array: top, top, bottom, bottom (two colours)
```

Colour predicates (`ImageUtil.kt:582-589`):
- `isDark()`: `red < 40 && blue < 40 && green < 40 && alpha > 200`.
- `isCloseTo(other)`: `|r - r'| < 30 && |g - g'| < 30 && |b - b'| < 30` (alpha ignored).
- `isWhite()`: `red + blue + green > 740` (alpha ignored, so the threshold is 741 to 765).

Observations for the port:
- The gradient array has four entries but a `GradientDrawable` with `TOP_BOTTOM` and four colours is painted as four equal stops (0, 1/3, 2/3, 1). The visible effect is two colour bands with a hard transition at the middle of the array: the first two stops are one colour and the last two the other, so the transition is a gradient across the middle third. Document this exact painting in the GTK port: CSS `linear-gradient(to bottom, A 0%, A 33.33%, B 66.67%, B 100%)` or cairo with four stops at 0, 1/3, 2/3, 1.
- `landscape` detection: `context.resources.configuration.orientation == ORIENTATION_LANDSCAPE`. On the desktop there is no orientation. PROPOSAL: landscape means the reader viewport's width is greater than its height at the moment of the call (the same test the Android configuration would make for a phone held sideways, a documented approximation).
- Edge: the function returns early from the `darkBG` branch with `solid(blackColor)` (the only early return inside the gradient `when`).
- Edge (OBSERVED defect, verified by hand on the formulas): the sample column for the offset pixel is `x + offsetX` when `x >= W / 2`. For `x = rightOffsetX = W - left + offsetX` this is `W - left + 2 * offsetX`, which exceeds `W - 1` when `2 * offsetX > left - 1`. With `left = trunc(0.0275 W)` and `offsetX = trunc(0.01 W)`, that holds for `100 <= W <= 109` (`left = 2`, `offsetX = 1`). The Kotlin then calls `Bitmap.getPixel` with an out-of-range x, which throws; `PagerPageHolder.setImage` catches it and shows the page error state (`PagerPageHolder.kt`, `setImage` catch block). Observed, and worth a test (vector in p1-edge-cases-and-tests.md).
- PROPOSAL for this edge: when the offset column is outside `0..W-1`, treat the offset pixel as "not dark" (so the `isDark(pixel) && isDark(pixelOff)` test fails), and do not raise an error. This is a deliberate fix; the rest of the algorithm is unchanged. Keep every other index in range (checked: `leftOffsetX - offsetX >= 0` holds for `W >= 50`).

PROPOSAL: `kuro-core::image::background::choose(img: &RgbaImage, landscape: bool) -> Background`, with `enum Background { Solid(Rgba), VerticalBands { top: Rgba, bottom: Rgba } }`. The pure function takes the decoded image and a flag, so it is unit-testable without GTK. Task T-13a-029.

### 5.13 Logging inside ImageUtil

OBSERVED: `logcat { ... }` for the BitmapRegionDecoder failure and for each split success; `logcat(LogPriority.ERROR, e)` on split failure. PROPOSAL: `tracing::debug!` and `tracing::error!` with the same messages. Task T-13a-029 and T-13a-028.

---

## 6. Torrent helpers (`core/common/src/main/java/aniyomi/core/common/torrent/`)

### 6.1 Bencode grammar (BencodeParser.kt)

OBSERVED (`parse(input)` = `parseValue(readNextByte())` then `require(input.read() == -1) { "Unexpected extra data" }`):
- `parseValue(head)`:
  - `'i'` gives integer.
  - `'0'..'9'` gives byte string.
  - `'l'` gives list.
  - `'d'` gives dictionary.
  - else `IllegalArgumentException("Unexpected value type")`.
- `parseInteger()`: `parseNumberHelper(readNextByte(), 'e')`.
- `parseByteString(head)`: `length = parseStringLength(head)`; reads exactly `length` bytes (EOF gives `EOFException`).
- `parseStringLength(head)`: `parseNumberHelper(head, ':')`; `require(result in 0..Int.MAX_VALUE) { "Invalid string length" }`.
- `parseNumberHelper(head, terminator)`: the digit grammar:
  - First char: `-` or a digit.
  - If the accumulated text is `"0"`, no more characters are allowed (so `0` only as a single digit, `i0e` is valid, `i00e` and `i01e` are invalid).
  - If the accumulated text is `"-"`, the next char must be `1..9` (so `i-0e` is invalid).
  - Otherwise digits only.
  - Loop ends when the next byte equals the terminator. Parsed with `toLong()` (overflow throws `NumberFormatException`, not caught).
  - Errors: `IllegalArgumentException("Unexpected integer character")`.
- `parseList()`: loop reading heads until `'e'`; each element through `parseValue`.
- `parseDictionary()`: loop: read head; if `'e'` stop; `key = parseByteString(head)`; `require(result.isEmpty() || result.lastKey() < key) { "Dictionary keys out of order" }`; `result[key] = parseValue(readNextByte())`. A duplicate key fails the `<` check (strict).
- Key comparison is `ByteString.compareTo`:
  - `(this === other || this.value === other.value)` gives 0.
  - Else for each pair `(a, b)` from `zip` (shorter length): `comp = a - b` where `a` and `b` are **Kotlin `Byte` (signed)**; return the first non-zero.
  - Else `this.size - other.size`.
- Observed consequence: bytes `0x80..0xFF` are negative, so `0xFF` sorts before `0x00`. A dictionary with an ASCII key followed by a key starting with a byte `>= 0x80` (for example `"a"` then `"é"` encoded as UTF-8) is rejected with "Dictionary keys out of order", even though the BEP 3 byte order accepts it.

PROPOSAL
- Grammar identical (leading zeros, `-0`, empty numbers, trailing data, EOF, overflow all rejected).
- Key order is **unsigned** byte order (BEP 3). This accepts torrents the Kotlin rejects; it is a recorded deviation. Consequence for the info hash: none, because the info dictionary is re-encoded in unsigned sorted order, which equals raw bytes for canonical input.
- Integers are `i64`. Overflow is an error (`BencodeError::IntegerOverflow`).
- String length must fit `i32::MAX`, as in the Kotlin, so lengths are capped at 2 GiB (`0..=2147483647`).
- Task T-13a-036.

### 6.2 Bencode writer (BencodeWriter.kt)

OBSERVED: `i<decimal>e`, `<len>:<bytes>`, `l...e`, `d...e` with keys written in `SortedMap` order and checked strictly `>` against the previous key (`IllegalArgumentException("Dictionary keys not in order")`). Integers use `Long.toString(10)`, so `-5` is `i-5e`.

PROPOSAL: encode to `Vec<u8>`; the same check (strict order, unsigned) cannot fail for a map built by the parser, but keep the check for externally built maps. Task T-13a-037.

### 6.3 Torrent metadata (TorrentHelpers.kt)

OBSERVED: `parseTorrentDetailsFromTorrentFileContent(torrentFileContent: InputStream): Torrent`:
1. `parsed = BencodeParser.parse(stream) as BencodeValue.Dictionary`.
2. `infoDictionary = parsed.getByString("info") as BencodeValue.Dictionary`. If `info` is absent, `null as Dictionary` throws `NullPointerException` (not wrapped). Observed.
3. `title = (infoDictionary.getByString("name") as ByteString).toUTF8String()`. `toUTF8String` uses a strict UTF-8 decoder: invalid bytes throw `CharacterCodingException` (not wrapped). Missing `name` gives NPE.
4. `hash = calculateInfoHash(infoDictionary)`: `BencodeWriter.write(info, DigestOutputStream(NullOutputStream, SHA-1))` then lowercase hex of the 20-byte digest (`String.format("%02x", b)`, which gives the two's-complement hex of signed bytes, i.e. the correct hex).
5. trackers, in this order and without filtering or deduplication:
   - If `announce` is present: must be `ByteString` (else `ClassCastException`, wrapped as `RuntimeException("Invalid torrent file")`), add its UTF-8 string.
   - If `announce-list` is present: must be `List` (else wrapped); for each element must be `List` (else wrapped); for each inner element must be `ByteString` (else wrapped); add UTF-8 string.
   - Empty strings are added as they are.
6. `lengthEntry = infoDictionary.getByString("length") as Integer?`; `filesEntry = infoDictionary.getByString("files") as List?` (wrong types wrapped).
7. `when`:
   - `length != null && files == null`: `torrentSize = length.value`, `fileStats = [FileStats(id = null, path = title, length = length.value)]`. Note: `id` is null and `path` is the info name.
   - `files != null && length == null`: for `i, file` in `files` with `index = i`:
     - `fileAsDict = file as Dictionary` (wrapped if not);
     - `fileSize = fileAsDict["length"] as Integer` (wrapped; missing gives NPE);
     - `filePath = (fileAsDict["path"] as List).joinToString("/") { (it as ByteString).toUTF8String() }` (wrapped/NPE as above). An empty `path` list gives `""`.
     - `totalSizeAcc += fileSize`; `FileStats(id = i + 1, path = filePath, length = fileSize)`.
     - Result `torrentSize = totalSizeAcc`.
   - else: `RuntimeException("Invalid torrent file")`: both or neither present.
8. Returns `Torrent(title, hash, torrentSize, trackers, fileStats)`.
9. Exceptions: only `ClassCastException` is wrapped into `RuntimeException("Invalid torrent file", e)`. `NullPointerException`, `CharacterCodingException`, `IllegalArgumentException`, `EOFException` and `NumberFormatException` escape unchanged.

PROPOSAL
- One error type `TorrentError::InvalidTorrent(reason)` for every structural failure (including the NPE cases), and `TorrentError::Decode(...)` for UTF-8 failures. The caller only needs "invalid torrent" text; the reason is logged.
- Keep the file-id rule (1-based for multi-file, `None` for single file).
- Hash over the re-encoded info dictionary (identical to the Kotlin; the test vectors match, verified in this plan against the fixtures). Alternative (not chosen): hash the raw slice of the input. Equal for canonical input; chosen = re-encode for parity.
- Task T-13a-038.

### 6.4 Torrent DTOs (model/Torrent.kt, model/TorrentRequest.kt)

OBSERVED
- `Torrent`: `title` (required), `hash` (default null), `torrent_size` (`torrentSize`, default null), `trackers` (default null), `file_stats` (`fileStats`, default null). `FileStats`: `id` (default null), `path` (required), `length` (required).
- `TorrentRequest`: `action` (required), `hash` (default `""`), `link` (default `""`), `title` (default `""`), `poster` (default `""`), `data` (default `""`), `save_to_db` (`saveToDb`, default `false`).
- JSON settings from DI (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`): `ignoreUnknownKeys = true`, `explicitNulls = false`. `encodeDefaults` is left at its default `false`, so **fields equal to their default are not sent**. A `TorrentRequest` for add is therefore `{"action":"add","link":"...","title":"..."}`, plus `"poster"` or `"data"` only when non-empty, plus `"save_to_db":true` only when true.

PROPOSAL: serde with `#[serde(skip_serializing_if = ...)]` on each defaulted field, the same omission rule, and `#[serde(default)]` for decode. `Torrent` decodes unknown fields ignored. Task T-13a-039.

### 6.5 TorrServer HTTP client (TorrentServerApi.kt)

OBSERVED
- `hostUrl` = `"http://127.0.0.1:$port"`. `port` is `@Volatile`, initially `0`, set by `setPort`.
- `echo()`: `GET $hostUrl/echo`, `awaitSuccess`, body as string. Any exception: logged at DEBUG ("Error sending echo") and returns `""`. An empty return is the signal "not running" used everywhere (`TorrentServerService.kt:65,178`).
- `addTorrent(link, title, poster = "", data = "", save: Boolean)`: POST `$hostUrl/torrents` with `application/json` body `TorrentRequest(action = "add", link, title, poster, data, saveToDb = save)`. Response decoded as `Torrent`. Non-2xx throws (network helper `awaitSuccess`).
- `uploadTorrent(file, title, save = false)`: reads the entire stream (`file.use { readBytes() }`), builds `multipart/form-data` with parts: `file` (filename = `title`, content type `application/x-bittorrent`), `save` (`"true"`/`"false"`), `title`. POST `$hostUrl/torrent/upload`. Response decoded as `Torrent`.
- Errors are not translated here. Translation happens in `TorrentUtils.getTorrentInfo` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/TorrentUtils.kt:24-33`): magnet add gives `SocketTimeoutException` to `DeadTorrentException`, any other exception to `DisabledTorrServerException`.

PROPOSAL: `TorrServerClient { base: Url, http: reqwest::Client }` with `echo() -> Result<bool>` (never errors; false on any failure, logged at debug), `add(...)`, `upload(...)`. Endpoints and body shapes as above. Task T-13a-040.

### 6.6 Local stream URL (TorrentServerUtils.kt:21-27)

OBSERVED
- `file = torr.fileStats?.firstOrNull { it.id == index }`.
- `name = file?.let { File(it.path).name } ?: torr.title`. `File(path).name` is the part after the last `/` (on Android). For `"3/3.1"` that is `"3.1"`.
- `"http://127.0.0.1:${api.getPort()}/stream/${name.urlEncode()}?link=${torr.hash}&index=$index&play"`.
- `urlEncode` = `URLEncoder.encode(this, "utf8")` (form encoding: space to `+`, unreserved `A-Z a-z 0-9 . - * _` kept, other bytes as `%XX` uppercase).
- Observed: `torr.hash` is printed as `null` if missing (`hash` is optional in the DTO).
- Observed: `index` is the **requested** index, while `file.id` for multi-file torrents is 1-based. Playback code requests index `0` for uploads and for magnets without `index=` (`PlayerActivity.kt:1160,1178`). For a multi-file torrent, `index=0` matches no `id`, so the name falls back to `torr.title` and the URL carries `index=0`. Whether TorrServer treats `0` as "first file" is not verifiable from this repository (open question OQ-13a-2).

PROPOSAL: same string; `hash` required (error if absent); `name` split on `/` only (not `std::path`, so Windows separators in torrents do not change the result). Task T-13a-044.

### 6.7 Tracker list (TorrentServerUtils.kt:13-19)

OBSERVED
- `animeTrackers = preferences.torrServerTrackers().get().split("\n").joinToString(",\n")`. Computed **once**, when the singleton `TorrentServerUtils` is constructed (`AppModule.kt:239`, `addSingletonFactory`). Changes to the preference afterwards are not seen until the next app start. Observed defect, and the preference UI stops the server on change (`PlayerSettingsTorrentScreen.kt:120-123`), so the user sees a restart that does not apply the new list until the app restarts.
- `split("\n")` keeps `\r` and empty lines; an empty line produces `,\n\n` (an empty entry). The default has no empty lines.
- `setTrackersList()` = `TorrServer.addTrackers(animeTrackers)` (native, return value unused).

PROPOSAL: read the preference at each start (`setTrackersList` reads the current value), trim each line, drop empty lines, and send. How the tracker list is delivered is the replacement question in p1-topic-torrserver-lifecycle.md. Task T-13a-041.

### 6.8 Torrent preferences (TorrentPreferences.kt)

OBSERVED keys, types and defaults (full table in p1-data-and-config.md):
- `pref_torrserver_enable` (Boolean, `false`).
- `pref_torrserver_shownotice` (Boolean, `false`).
- `pref_torrserver_port` (String, `"8090"`). The settings summary says "Enter 0 to let system choose a port" (`pref_player_torrents_port_summary`), so `0` means the server picks a free port; the port returned by `startServer` is what the app uses.
- `pref_torrserver_tackers` (String, default is the 26-line list, see data file; note the misspelled key `tackers`, kept for compatibility).
- `pref_torrserver_proxymode` (enum `ProxyMode`, default `ProxyMode.None`, stored as the constant name).
- `pref_torrserver_proxyurl` (String, `""`).

`ProxyMode` (TorrentPreferences.kt:46-51): `None("tracker")`, `Tracker("tracker")`, `Peers("peers")`, `Full("full")`. The `value` is what is passed to TorrServer as `proxyMode` (TorrentServerService.kt:74). So the **None** choice is sent to the server as `tracker`, with an empty proxy URL. Observed defect: the server is told to run tracker-proxy mode with no proxy. Either the server ignores proxy mode when the URL is empty, or the mode takes effect. Not verifiable from this repository. PROPOSAL: map `None` to the server's documented "no proxy" value if one exists, otherwise do not send a proxy mode at all when None. Recorded as OQ-13a-3.

Settings validation (`PlayerSettingsTorrentScreen.kt:94-106, 149-185`):
- Port: `pref.toIntOrNull()` must be non-null and in `0..65535`; otherwise error `pref_player_torrents_port_error`. The field is numeric-keyboard (`KeyboardType.Number`) and disabled when torrents are off.
- Proxy URL: `uri = pref.toUri()`; `scheme == null || host == null` gives error `pref_player_torrents_proxy_url_invalid_uri`; scheme not in `{http, https, socks4, socks4a, socks5, socks5h}` gives error `pref_player_torrents_proxy_url_invalid_protocol` with the scheme as argument. Disabled when torrents are off or proxy mode is `None`.
  - Observed: an empty value fails the scheme check first, so the dialog refuses an empty URL (the shared `EditTextInfoPreference` is outside this area; behaviour noted, not verified in the UI).
- Trackers: multi-line edit, subtitle = first two lines joined by `\n`, with `\n...` appended when there are more than two lines (`PlayerSettingsTorrentScreen.kt:113-119`). Change stops the server (`:120-123`).
- Reset trackers: text action enabled when torrents are on and the value differs from the default; deletes the key (so the default applies, `:126-134`).
- Enable switch: turning on for the first time (`shownNotice == false`) shows the notice dialog (title `pref_player_torrents_notice`, text `pref_player_torrents_notice_text` and `pref_player_torrents_notice_footer`, OK button `MR.strings.action_ok`) and sets `shownNotice = true` at once. Turning off stops the server (`TorrentServerService.stop()`).

PROPOSAL: the same validators in `kuro-core::torrent::prefs::validate_port(&str)` and `validate_proxy_url(&str)` returning `Result<(), ValidationError>`, used by the GTK settings row. Task T-13a-042 (keys, defaults, reset) and T-13a-043 (ProxyMode quirk and validators).

### 6.9 Torrent server process (TorrentServerService.kt and TorrentServerUtils.kt; OBSERVED in the Android service)

OBSERVED
- `start()` (companion): `startService(Intent(ACTION_START))` then `wait(10)`. Exceptions: DEBUG log and `printStackTrace()`.
- `stop()`: `startService(Intent(ACTION_STOP))`. Exceptions: DEBUG log.
- `onStartCommand`: `ACTION_START` runs `startServer()`, posts the notification, returns `START_STICKY`. `ACTION_STOP` runs `stopServer()` and returns `START_NOT_STICKY`. Anything else returns `START_NOT_STICKY`.
- `startServer()` (runs in `CoroutineScope(EmptyCoroutineContext)`, i.e. `Dispatchers.Default`):
  1. If `api.echo() == ""` (not running):
     - if `networkPreferences.verboseLogging()`: `TorrServer.registerLogCallback()`;
     - `proxyMode = torrServerProxyMode().get()`;
     - `port = TorrServer.startServer(port = torrServerPort().get(), path = filesDir.absolutePath, proxyMode = proxyMode.value, proxyUrl = if proxyMode == None then "" else torrServerProxyUrl().get())`;
     - if `port != -1`: `api.setPort(port)`; `wait(10)`; `torrentServerUtils.setTrackersList()`.
  2. If echo is non-empty (already running), nothing happens: the port is not updated and trackers are not pushed.
  3. If `startServer` returns `-1`, nothing is set and the foreground notification still says "running" (observed inconsistency).
- `stopServer()`: `TorrServer.stopServer()`, cancel notification `Notifications.ID_TORRENT_SERVER`, `stopSelf()`.
- `wait(timeout: Int = -1)` (companion): `count = 0`; if `timeout < 0` then `count = -20`; `while (api.echo() == "") { delay(1s); count++; if (count > timeout) return false }`; `return true`.
  - `wait(10)`: at most 11 one-second polls (count 0..11, returns false when it exceeds 10).
  - `wait()` (timeout -1): starts at -20, so about 20 polls before `false`.
- Notification (`notification(context)`): channel `Notifications.CHANNEL_TORRENT_SERVER`; small icon `R.drawable.ic_ani`; content text `AYMR.strings.torrentserver_is_running` ("Torrent Server is running"); title `MR.strings.app_name` ("Aniyomi"); `setAutoCancel(false)`; `setOngoing(true)`; delete intent restarts the service with `ACTION_START` (Android 14 workaround, comment "fuck android 14"); `setUsesChronometer(true)`; action button label **`"Stop"`** (hardcoded English, not a string resource) with `ACTION_STOP`. Foreground type `DATA_SYNC` on API 29 and up.

PROPOSAL: see p1-topic-torrserver-lifecycle.md for the supervised child-process design. Behaviour kept: idempotent start (already running means no-op), stop on disable, the 10-poll readiness wait, the tracker push after start, `-1` treated as failure (and the status shown as "not running", a deliberate fix). Task T-13a-041.

### 6.10 Torrent playback routing (consumer: PlayerActivity.kt:1093-1197)

OBSERVED
- Route to torrents when `torrServerEnable()` and (`videoUrl.startsWith(torrentServerApi.hostUrl)` or `videoUrl.startsWith("magnet")` or `videoUrl.endsWith("torrent")`).
  - `hostUrl` is the current `http://127.0.0.1:<port>`, so a URL produced by this server is detected only when the port matches.
- Then `launchIO { TorrentServerService.start(); torrentLinkHandler(videoUrl, videoTitle, videoOptions) }`.
- `videoOptions` = `mpvArgs.joinToString(",") { (option, value) -> "$option=\"$value\"" }` (double-quoted values, passed to mpv `loadfile` as the fourth argument, `replace`, index `0`).
- `torrentLinkHandler`:
  1. `content://` URL: `openInputStream(videoUrl.toUri())!!` (NPE if null); `uploadTorrent(stream, title, save = false)`; play link index `0`; `loadfile`; return.
  2. magnet: `index` from `substringAfter("index=").substringBefore("&").toInt()` when the text contains `index=`; `NumberFormatException` gives `0`; default `0`.
  3. `addTorrent(videoUrl, title, "", "", false)`; `getTorrentPlayLink(torrent, index)`; `loadfile`.
- Errors in this path are not caught here; they propagate out of `launchIO`.

PROPOSAL: the same routing rules in `kuro-core::torrent::route::classify(url) -> Route::{Direct, TorrentServer}` and `resolve(route) -> PlayLink`. The `content://` branch becomes "file path": read the file and upload. Errors are returned, and the player shows a toast with the mapped message. Task T-13a-044.

### 6.11 Torrent metadata for the source API (TorrentUtils.kt:20-73, consumer of 6.3 and 6.5)

OBSERVED
- `getTorrentInfo(url, title)`:
  - magnet (`url.startsWith("magnet")`): `addTorrent(url, title, "", "", false)`; `SocketTimeoutException` gives `DeadTorrentException()`; any other exception gives `DisabledTorrServerException()`.
  - else: GET `url` through the app's HTTP client, parse the body as a torrent file (no server needed).
  - `torrentToTorrentInfo(torrent, title)`: `TorrentInfo(title = overrideTitle ?: torrent.title, files = fileStats.map { TorrentFile(path, id ?: 0, length, hash!!, trackers ?: []) } ?: [], hash = hash!!, size = torrentSize ?: -1, trackers = trackers ?: [])`. The `title` argument is non-null, so it always wins over the torrent's own name.
- A blocking shim for binary compatibility with extensions exists (`blockingShimForGetTorrentInfo`, hidden deprecation, `runBlocking`).

PROPOSAL: `torrent::info::get_torrent_info(url, title) -> Result<TorrentInfo, TorrentError>` with the same branches. Task T-13a-038 and T-13a-040. The blocking shim is not needed (no binary extension ABI in Rust) and is recorded as dropped.

---

## 7. Android system helpers (`util/system/`, `util/lang/`, `mihon/core/common/extensions/`)

### 7.1 DeviceUtil (DeviceUtil.kt)

OBSERVED (summary, all Android-only):
- `isMiui` = `getSystemProperty("ro.miui.ui.version.name")?.isNotEmpty() ?: false`.
- `miuiMajorVersion` = `Build.VERSION.INCREMENTAL.substringBefore('.').trimStart('V').toIntOrNull()` when MIUI.
- `isMiuiOptimizationDisabled()`: property `persist.sys.miui_optimization` equal to `"0"` or `"false"`, else reflection on `android.miui.AppOpsUtils.isXOptMode`.
- `isSamsung` = `Build.MANUFACTURER` equals `samsung` ignoring case.
- `oneUiVersion`: reads `Build.VERSION.SEM_PLATFORM_INT - 90000`; `< 0` gives `1.0`; else `(version / 10000) + "." + (version % 10000 / 100)` as Double.
- `invalidDefaultBrowsers` = `["android", "com.hihonor.android.internal.app", "com.huawei.android.internal.app", "com.zui.resolver", "com.transsion.resolver", "com.android.intentresolver"]`.
- `isLowRamDevice(context)`: `ActivityManager.MemoryInfo.totalMem < 3 * 1024^3` (3 GiB).
- `getSystemProperty(key)`: reflection on `android.os.SystemProperties.get`; on failure WARN log and `null`.

Consumers: `App.kt`, `UiPreferences.kt`, `MigrateSecureScreenMigration.kt`, `ContextExtensions.kt`, settings and debug screens. All of them are Android UI or migration concerns.

PROPOSAL: Drop all vendor, One UI and browser-list items. Keep one platform query for the memory check if a consumer needs it (`kuro-platform::sys::total_memory_bytes()` reading `/proc/meminfo` `MemTotal` in kB, times 1024). Task T-13a-045.

### 7.2 GLUtil (GLUtil.kt) and texture-limit option list

OBSERVED
- `DEVICE_TEXTURE_LIMIT` (lazy): EGL 1.0 query. `eglGetDisplay(DEFAULT)`, `eglInitialize`, count configs, list configs, max over `EGL_MAX_PBUFFER_WIDTH`, `eglTerminate`; result `max(maximumTextureSize, SAFE_TEXTURE_LIMIT)`.
- `SAFE_TEXTURE_LIMIT = 2048`.
- `CUSTOM_TEXTURE_LIMIT_OPTIONS` (lazy): `steps = DEVICE_TEXTURE_LIMIT / 1024`; `buildList(steps) { add(DEVICE); for step in steps downTo 2 { v = step * 1024; if v >= DEVICE continue; add(v) } }`.
  - Examples: device `8192` gives `[8192, 7168, 6144, 5120, 4096, 3072, 2048]`; device `4096` gives `[4096, 3072, 2048]`; device `3000` gives `[3000, 2048]`; device `2048` gives `[2048]`.
- Preference `pref_hardware_bitmap_threshold` (Int, `GLUtil.SAFE_TEXTURE_LIMIT` in `BasePreferences.kt:40`). `App.kt:141-143` sets it to `DEVICE_TEXTURE_LIMIT` when unset on first run.
- Settings row (`SettingsAdvancedScreen.kt`, the reader group, around lines 363-384): title `pref_hardware_bitmap_threshold`, entries from `CUSTOM_TEXTURE_LIMIT_OPTIONS` (index 0 shows `pref_hardware_bitmap_threshold_default` with the value), summary `pref_hardware_bitmap_threshold_summary`; enabled only when `!HARDWARE_BITMAP_UNSUPPORTED && DEVICE_TEXTURE_LIMIT > SAFE_TEXTURE_LIMIT`.

PROPOSAL: Drop the EGL query and the preference (T-13a-030). The option list is kept as a pure function `texture_limit_options(device: u32) -> Vec<u32>` only if the desktop reader adds a maximum-decoded-size setting, which is not part of this plan. The stored preference key is ignored and not migrated (no value of it affects the desktop reader).

### 7.3 WebView helpers (WebViewUtil.kt)

OBSERVED
- `getInferredUserAgent(context)`: default UA of a `WebView` (obtained by setting `userAgentString = null`, reading, then restoring) with two regex replacements: `"; Android .*?\\)"` gives `"; Android 10; K)"` (non-greedy, up to the first `)`); `"Version/.* Chrome/"` gives `"Chrome/"` (greedy). Example input and output in the source comment (`WebViewUtil.kt:119-127`).
- `getVersion(context)`: `WebView.getCurrentWebViewPackage()` null gives `"how did you get here?"`, else `"$label $version"` (application label plus `versionName`).
- `supportsWebView(context)`: `CookieManager.getInstance()` must not throw (any `Throwable`: ERROR log, `false`); and `hasSystemFeature(FEATURE_WEBVIEW)`.
- `spoofedPackageName(context)`: `"com.android.chrome"` if installed, else `"com.android.settings"`.
- `MINIMUM_WEBVIEW_VERSION = 118`. `isOutdated()`: `getWebViewMajorVersion() < 118`; the major version is the first capture of `.*Chrome/(\d+)\..*` on the default UA, `0` when no match (so an unparseable UA counts as outdated).
- `getHtml()`: `evaluateJavascript("document.documentElement.outerHTML") { html -> resume(html) }`. Android returns the JavaScript result as JSON text, so a string arrives quoted and escaped. The only caller is `app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt:80`; whether it decodes the JSON is outside this area (verify in that file before porting).
- `setDefaultSettings()`: `javaScriptEnabled = true`, `domStorageEnabled = true`, `useWideViewPort = true`, `loadWithOverviewMode = true`, `cacheMode = LOAD_DEFAULT`, `setSupportZoom(true)`, `builtInZoomControls = true`, `displayZoomControls = false`; `CookieManager.getInstance().acceptThirdPartyCookies(this)`.

PROPOSAL (WebKitGTK via `webkit6`, owned by the `WebViewHost` trait in kuro-platform; see area:12-network-stack for the Cloudflare flow)
- UA: keep the same two regex transforms on the WebKitGTK default UA, so sites see the same string shape. Open decision OQ-13a-4: whether a Linux UA should claim Android at all (the transform is kept for parity; the alternative is to keep the native Linux UA).
- Version gate: `webkit6::functions::major_version()/minor_version()`; minimum to be fixed by the networking area after testing sources (proposal `2.44`, unverified; open).
- Settings: `WebKitSettings`: `enable-javascript = true`, `enable-html5-local-storage = true`, `enable-html5-database = true`; zoom and viewport flags dropped (no Android meaning). Third-party cookies: `WebKitCookieManager` accept policy `always` (the Android `acceptThirdPartyCookies` equivalent; verify in the webkit6 API at implementation).
- `getHtml()` returns decoded text (no JSON wrapper): `webview.evaluate_javascript(...)` then `to_str()`.
- Task T-13a-052.

### 7.4 Toast (ToastExtensions.kt)

OBSERVED: `Context.toast(text: String?, duration, block)` = `Toast.makeText(applicationContext, text.orEmpty(), duration).also { block(it); it.show() }`. `toast(resource)` localises first. Durations: `Toast.LENGTH_SHORT` (value 0) and `Toast.LENGTH_LONG` (value 1) are flags; the displayed durations are about 2000 ms and 3500 ms (Android platform values). The `block` runs before `show()`.

PROPOSAL: `kuro-ui::toast::show(message, Duration)` with durations `Short = 2 s`, `Long = 3.5 s`, an in-app overlay (not an OS notification), one toast at a time (a new message replaces the visible one), dismissed on click, text as given (empty text shows nothing). Task T-13a-053.

### 7.5 dpToPx (DensityExtensions.kt)

OBSERVED: `(this * Resources.getSystem().displayMetrics.density).toInt()` (truncation toward zero).

PROPOSAL: Drop; GTK sizes are logical pixels with the scale applied by the toolkit. Task T-13a-046.

### 7.6 Coroutine helpers (CoroutinesExtensions.kt)

OBSERVED
- `launchUI(block)` = `GlobalScope.launch(Dispatchers.Main, DEFAULT, block)` (delicate API).
- `launchIO(block)` = `GlobalScope.launch(Dispatchers.IO, DEFAULT, block)`.
- `launchNow(block)` = `GlobalScope.launch(Dispatchers.Main, UNDISPATCHED, block)`: runs on the caller until the first suspension point (no dispatch).
- `CoroutineScope.launchUI` = `launch(Dispatchers.Main)`; `CoroutineScope.launchIO` = `launch(Dispatchers.IO)`.
- `CoroutineScope.launchNonCancellable(block)` = `launchIO { withContext(NonCancellable, block) }`: the work runs to completion even if the scope is cancelled.
- `withUIContext` = `withContext(Dispatchers.Main)`; `withIOContext` = `withContext(Dispatchers.IO)`; `withNonCancellableContext` = `withContext(NonCancellable)`.
- Callers counted in p1-inventory.md: `withIOContext` (62 raw matches), `withUIContext` (29), `launchUI` (12), `launchNonCancellable` (19). These counts come from a text search and include non-scope files.

PROPOSAL
- IO work: `tokio::task::spawn_blocking` for CPU and disk work (images, archives, hashing), `tokio::spawn` for network and timers.
- UI work: a `glib::MainContext` handle; `ui_dispatch(|| ...)` posts a closure to the default main context (`glib::MainContext::default().invoke(...)`) or `spawn_local` from the main thread.
- `launchNow`: no direct equivalent. Use a direct call on the main thread for work that must start before the caller continues (rare; list in the callers when porting).
- `launchNonCancellable`: a detached `tokio::spawn` with the handle stored in the app context so shutdown can wait for it (`JoinSet` drain on exit).
- Task T-13a-048.

### 7.7 RxJava bridge (RxCoroutineBridge.kt)

OBSERVED: `awaitSingle()` subscribes with `request(1)`, resumes on `onNext`, throws `IllegalStateException("Should have invoked onNext")` on completion without a value, and resumes with the error on `onError` (the `tryResumeWithException` pattern avoids a double resume after cancellation). Cancellation unsubscribes.

PROPOSAL: Drop. The legacy Observable extension API is the source area's concern (area 9 and 12 for the source ABI). Rust sources are `async` traits. Task T-13a-047.

### 7.8 Logging (LogcatExtensions.kt)

OBSERVED: `Any.logcat(priority = DEBUG, throwable = null, message = { "" })` builds `msg = message()`; when `throwable != null`, adds `"\n"` if the message is not blank, then `throwable.asLog()` (stack trace text); logs through the `logcat` library with the receiver's class as the tag. Priorities used in this area: `DEBUG`, `ERROR`, `WARN` (the `LogPriority` enum of the logcat library).

PROPOSAL: `tracing` with fields. `logcat(priority)` maps `VERBOSE->trace, DEBUG->debug, INFO->info, WARN->warn, ERROR->error, ASSERT->error`. The throwable goes into a structured `error = %e` field plus the chained sources (`anyhow`/`thiserror` chain walk). Task T-13a-049.

### 7.9 Localisation (Localize.kt)

OBSERVED: `stringResource(res)` = `StringDesc.Resource(res).toString(context).fixed()`; `stringResource(res, args...)` = `StringDesc.ResourceFormatted(...)`; plurals via `StringDesc.Plural` / `PluralFormatted` (Android quantity rules). `fixed()` = `this.replace("\\", "")` (the source writes it as the raw string `"""\"""` replaced by `""""""`, i.e. backslash to nothing). Comment: "janky workaround for moko-resources issue 337".

Observed consequence: every backslash in a localised string, **including backslashes inside substituted arguments** (for example a Windows path shown in a message), is removed.

PROPOSAL: keep formatting; do not strip backslashes (the i18n layer of area i18n owns format strings; no caller here needs the workaround). Plural rules come from the i18n crate (`fluent` plural categories). Task T-13a-050.

### 7.10 JSON empty object (JsonObject.kt)

OBSERVED: `JsonObjectEmpty = JsonObject(emptyMap())`; `JsonObjectEmptyBytes = byteArrayOf(0x7B, 0x7D)` (the two ASCII bytes of `{}`); `JsonObject.Companion.EMPTY` returns the empty object.

PROPOSAL: `serde_json::Value::Object(serde_json::Map::new())` and `b"{}"`. Task T-13a-051.

### 7.11 Boolean to long (BooleanExtensions.kt)

OBSERVED: `Boolean.toLong() = if (this) 1L else 0L`. Used with SQL bind parameters (no caller in this area).

PROPOSAL: `i64::from(b)`; remove the helper when no caller remains. Task T-13a-057.

---

## 8. Archive and image caller map (summary of where each function is used)

| Function | Consumer (repo-relative) |
|---|---|
| `ArchiveReader.useEntries` / `getInputStream` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ArchivePageLoader.kt`, `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt`, `source-local/src/androidMain/kotlin/tachiyomi/source/local/metadata/EpubReaderExtensions.kt` |
| `EpubReader` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/EpubPageLoader.kt`, `source-local/.../metadata/EpubReaderExtensions.kt`, `ChapterLoader.kt` |
| `ZipWriter` | `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt` (`archiveChapter`) |
| `ImageUtil.isImage`, `findImageType`, `getExtensionFromMimeType` | `MangaDownloader.kt`, `MangaDownloadManager.kt:175`, `ImageSaver.kt:38`, `TachiyomiImageDecoder.kt:74`, local source image managers |
| `ImageUtil.chooseBackground`, `isAnimatedAndSupported`, `isWideImage`, `splitInHalf`, `rotateImage` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt` |
| `ImageUtil.splitAndMerge` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt:224` |
| `ImageUtil.splitTallImage` | `MangaDownloader.kt:595` |
| `DiskUtil.buildValidFilename` | `ImageSaver.kt:39`, `ReaderViewModel.kt`, `PlayerViewModel.kt`, `MangaDownloader.kt` |
| `DiskUtil.getAvailableStorageSpace` | `MangaDownloader.kt:337` |
| `DiskUtil.createNoMediaFile` | `MangaDownloader.kt:432` |
| `DiskUtil.getDirectorySize` | `ChapterCache.kt:51` |
| `DiskUtil.hashKeyForDisk` | `ChapterCache.kt`, `MangaCoverCache.kt` |
| `copyAndSetReadOnlyTo` | `AnimeExtensionLoader.kt`, `MangaExtensionLoader.kt` |
| `TorrentServerUtils`, `TorrentServerApi`, `TorrentPreferences` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt`, `data/torrent/service/TorrentServerService.kt`, `data/download/anime/AnimeDownloader.kt`, `ui/entries/anime/AnimeScreenModel.kt`, `di/AppModule.kt`, `di/PreferenceModule.kt`, `presentation/.../PlayerSettingsTorrentScreen.kt` |
| `TorrentHelpers` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/TorrentUtils.kt` |
| `WebViewUtil` | `app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt`, `app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt`, `App.kt`, `DebugInfoScreen.kt` |
| `StringExtensions.takeBytes` | `ReaderViewModel.kt`, `PlayerViewModel.kt` |
| `StringExtensions.htmlDecode` | `data/track/anilist/dto/ALAnime.kt`, `ALManga.kt`, `mangaupdates/dto/MURecord.kt` |
| `compareToWithCollator` | `ChapterSorter.kt`, `EpisodeSorter.kt`, `SeasonSorter.kt`, library screen models |

Caller counts in p1-inventory.md come from a text search and are approximate. The table above lists the files verified by reading the call lines in this plan.

---

## 9. Concurrency, timings and limits (collected)

| Item | Value | Where |
|---|---|---|
| Torrent echo poll interval | 1 second (`delay(1.seconds)`) | `TorrentServerService.kt:179` |
| Torrent readiness wait after start | `wait(10)`: at most 11 polls | `TorrentServerService.kt:79,153` |
| Torrent default wait (`wait()`) | about 20 polls | `TorrentServerService.kt:173-186` |
| Toast short / long | about 2000 ms / 3500 ms (platform values) | Android `Toast` |
| Image split: parts per page | `ceil(H / (2 * maxDisplayDim))` approximately; exact formula in 5.10 | `ImageUtil.kt:282` |
| Image split trigger | `H >= 4 * W` (integer division `H / W > 3`) | `ImageUtil.kt:208` |
| Background sample rows | 25 (step `H / 25`) | `ImageUtil.kt:443` |
| Background minimum size | 50 x 50 px | `ImageUtil.kt:345` |
| Background streak thresholds | white streak `> 14`, black streak `>= 14`, streak `> 6`, black pixels `> 22` / `> 18`, white pixels `> 22`, overall `> 9` | `ImageUtil.kt:452-520` |
| Colour thresholds | dark: each of r, g, b `< 40` and alpha `> 200`; white: `r + g + b > 740`; close: each channel diff `< 30` | `ImageUtil.kt:582-589` |
| Filename budget | 250 bytes for the base name (before extension) | `DiskUtil.kt:254` |
| Filename component max | 240 UTF-16 units | `DiskUtil.kt:235` |
| Image sniff length | 32 bytes | `ImageUtil.kt:90`, `java_wrapper.cpp` |
| Image copy buffer | 8192 bytes | `FileExtensions.kt:52`, `UniFileTempFileManager.kt:78` |
| Low-RAM threshold | 3 GiB total memory | `DeviceUtil.kt:199` |
| Texture size safe default | 2048 | `GLUtil.kt:50` |
| Texture option step | 1024 | `GLUtil.kt:65` |
| Bencode string length max | `Int.MAX_VALUE` | `BencodeParser.kt:53` |
| Port range | `0..65535` | `PlayerSettingsTorrentScreen.kt:98` |
| WebView minimum major | 118 | `WebViewUtil.kt:116` |
| JPEG quality for all re-encodes | 100 | `ImageUtil.kt:145,155,192,249` |

Threads: all of this area's Kotlin is called from `Dispatchers.IO` (archive, image, file work) or `Dispatchers.Main` (toast, WebView). The torrent server supervisor runs on `Dispatchers.Default`. No shared mutable state except `TorrentServerApi.port` (`@Volatile`) and the `ArchiveInputStream.isClosed` flag (guarded by `lock`).

---

## 10. Interactions with other areas

- area:02-preferences-system owns the storage of the settings table and the settings screen; this area owns the typed handle and the private/app-state prefixes.
- area:12-network-stack owns the HTTP client (`NetworkHelper`, `awaitSuccess`, `GET`, `POST`), the Cloudflare and WebView flow, and the user-agent setting; this area owns `WebViewUtil` helpers and the TorrServer client that uses that client.
- area:01-foundation-app-bootstrap owns the workspace, `Paths`, the app context and startup order; this area registers its services there (`TorrentServerUtils`, `TorrentServerApi`, `TorrentPreferences` are singletons).
- The reader and download areas (other planners) own the call sites listed in section 8.
- Platform traits (cross-platform rule, `plans/addenda/requirements-cross-platform.md`): `Paths` (folders), `WebViewHost` (WebKitGTK), `Opener` (portal open and share), `Notifier` (torrent server status), `SchedulerBackend` (not used here). No other OS call is allowed in this area's Rust code.
