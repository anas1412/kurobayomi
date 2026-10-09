# Topic: the .tachibk wire format and kotlinx ProtoBuf behaviour

Scope: how `.tachibk` payloads are laid out and what the Kotlin serializer does, so that the Rust codec (`src/backup/wire.rs`, T-21a-001) produces files Aniyomi reads and reads files Aniyomi writes. Field tables are in p1-data-and-config.md; this file explains the rules and how each rule is verified.

## 1. File container

`.tachibk` = gzip(protobuf(`Backup`)). The reader also accepts raw protobuf (no gzip) because `BackupDecoder` treats any non-gzip, non-JSON prefix as raw (B-11). The writer always gzips (level 6, `okio.gzip()` default).

Protobuf wire primitives used (all standard protobuf wire format):

| Wire type | Number | Used for |
|---|---|---|
| Varint | 0 | Int, Long, Boolean, enum ordinal |
| Fixed64 | 1 | Double (`seasonNumber`) |
| Length-delimited | 2 | String, ByteArray, nested message, unpacked repeated element |
| Fixed32 | 5 | Float (`chapterNumber`, `episodeNumber`, `lastChapterRead`, `lastEpisodeSeen`, `score`, `value` of FloatPreferenceValue) |

Tag = `(field_number << 3) | wire_type`, encoded as a varint.

## 2. kotlinx ProtoBuf behaviours that matter here

Status column: "source" = visible in the Kotlin source in this repository; "verify" = standard kotlinx.serialization behaviour that must be confirmed with a fixture (T-21a-034) before the writer ships.

| # | Behaviour | Why it matters | Status |
|---|---|---|---|
| K-1 | Int, Long, Boolean as varint; negative values as 10-byte two's complement | Negative `sortIndex`, `seasonNumber` is not affected (Double) | verify |
| K-2 | Float is fixed32, Double fixed64 | Float fields are the reason for the float32 precision edge (E-07) | verify |
| K-3 | Lists of scalars are NOT packed by default; `@ProtoPacked` would pack | `categories` (field 17 of `BackupManga` and `BackupAnime`) is unpacked | source (no `@ProtoPacked` annotation in the repo) + verify |
| K-4 | Enums are encoded as a varint of the ordinal | `updateStrategy` (105, `ALWAYS_UPDATE=0`, `ONLY_FETCH_ONCE=1`), `fetchType` (507, `Seasons=0`, `Episodes=1`) | verify |
| K-5 | Null nullable fields are omitted | `artist`, `thumbnailUrl`, `favoriteModifiedAt`, `parentId`, `id`, `isLegacy` (Boolean?) | verify |
| K-6 | Default values are encoded (encodeDefaults) | A file from Aniyomi always contains `is_legacy` (500) = false and `favorite` (100). Kotlin's ProtoBuf default instance is used (`AppModule.kt:190-191`) | verify (the decoder does not depend on it; the encoder output size and the detector do) |
| K-7 | Unknown field numbers are skipped | Forward compatibility with newer Aniyomi files | verify |
| K-8 | Missing required field (no default) raises an error | `url`, `source`, `name`, `sourceId`, `pkgName`, `apk`, `key`, `value`, `sourceKey`, `isFavorite`, `sortIndex`, `content`, `longPressContent`, `onStartup`, `syncId`, `libraryId`, `baseUrl`, `indexUrl`, `signingKey`, `contactWebsite`, `signingKeyFingerprint`, `website`, `lastRead` (required fields without defaults) | verify |
| K-9 | Sealed classes (`PreferenceValue`) are encoded as a nested message whose tag identifies the subclass | Needed for `BackupPreference.value` (field 2) | verify (exact tags unknown from source text) |
| K-10 | Fields are written in declaration order (= ascending number in this code base) | Makes the golden byte examples in section 4 deterministic | verify |
| K-11 | Strings are UTF-8 | Nothing special | source (Kotlin String) |
| K-12 | Repeated message fields: one tag per element | `manga`, `chapters`, `episodes`, `tracking`, `history`, `categories` (message lists) | verify |

## 3. Decode tolerance policy (Kurobayomi)

- Accept packed and unpacked repeated scalars (E-36).
- Accept absent fields as defaults (K-6 means Aniyomi always writes them, but Mihon files may omit some).
- Reject wrong wire types on known fields (protects against misreading).
- Reject enum ordinals outside the declared range (kotlinx does the same).
- Reject missing required fields.
- Ignore unknown field numbers.
- Do not reject based on `is_legacy` value; choose the message type with the detector only.

## 4. Golden bytes (expected under K-1..K-6, K-10)

`BackupManga { source = 1, url = "a", all other fields default, viewer = 0, viewer_flags = null, favoriteModifiedAt = null, excludedScanlators = [] }`:

```
08 01                  field 1  source = 1 (varint)
12 01 61               field 2  url = "a" (len 1)
1A 00                  field 3  title = "" (len 0)
40 00                  field 8  status = 0
68 00                  field 13 dateAdded = 0
70 00                  field 14 viewer = 0
A0 06 01               field 100 favorite = true   (tag 800 = 0xA0 0x06)
A8 06 00               field 101 chapterFlags = 0  (tag 808)
C8 06 00               field 105 updateStrategy = ALWAYS_UPDATE (0)  (tag 840)
D0 06 00               field 106 lastModifiedAt = 0 (tag 848)
E8 06 00               field 109 version = 0        (tag 872)
```
Omitted: genre (no elements), artist, author, description, thumbnailUrl (null), chapters, categories, tracking, viewer_flags (null), history, favoriteModifiedAt (null), excludedScanlators (empty).

Use this as the first unit test of `BackupManga::encode` after the defaults assumption (K-6) is confirmed. If Aniyomi omits defaults, the expected output shrinks to `08 01 12 01 61` plus the non-default fields; keep both variants as fixtures.

## 5. Verifying the open items (T-21a-034)

Required artefacts from the user's Aniyomi app (the Kurobayomi repository cannot produce them):
1. `current-all.tachibk` and `current-default.tachibk` (created in the app; see p1-edge-cases-and-tests.md 4.10).
2. The text schema from Aniyomi's debug screen (`app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/BackupSchemaScreen.kt`, reachable from the debug settings), which prints `ProtoBufSchemaGenerator.generateSchemaText(Backup.serializer().descriptor)`. This gives the exact field numbers, types and the generated message for `PreferenceValue` (including the subclass tags in K-9). Copy it into `fixtures/generated/schema-dump.txt`.

Procedure:
1. Gunzip `current-all.tachibk` (`gzip -dc`), keep the raw protobuf.
2. Dump the top-level field tags with a 40-line script (tag, wire type, length) to confirm K-4, K-5, K-10.
3. For `BackupPreference` values, find the nested message tags used for `IntPreferenceValue` and the other five subclasses. Write them into `schema.rs` as the constants `PREF_VALUE_TAG_*`.
4. Compare the Rust-encoded `current-all` equivalent against the file field by field (not byte by byte; gzip and default encoding may differ).
5. Record the result as `fixtures/aniyomi/MANIFEST.md` (file, date, Aniyomi version, option set).

Until step 3 is done, `PrefValue` encoding must not ship (decode of an unknown preference tag returns a `WireError`; encoding preferences returns an explicit "preference encoding not verified" error). Backups without preferences can ship.

## 6. Legacy and Mihon files

- Legacy (pre-1.0 Aniyomi): `LegacyBackup` numbering (p1-data-and-config.md 1.2). Field 3 anime, field 106 extensions, 107 anime stores, 108 manga repos, 109 custom buttons. Must not be decoded with the current `Backup` message.
- Mihon: `Backup` fields 1, 2, 101, 104 (manga-only). Current Aniyomi reads them as current. Kurobayomi must read Mihon-shaped manga with the same message.

## 7. Size and performance notes

- The whole payload is built in memory (B-06.6). For a large library (10,000 manga with 500 chapters each) the uncompressed payload can exceed 200 MB. Mitigation (v2): stream the top-level list fields by writing each element directly into a gzip encoder; the Kotlin reader needs the whole file in memory anyway (`readByteArray`), so the reader side is the true limit on the Aniyomi side.
- Nested `Writer` allocation: one allocation per nested message per level. Acceptable for P1; measure before optimising.

## 8. Things deliberately not reproduced

- Protobuf schema evolution rules beyond the ones above (no `reserved` handling needed; Kurobayomi writes only the fields listed).
- The broken `BrokenBackupSource`/`BrokenBackupAnimeSource` field 0 layout (unused, dropped).
- The `full/models` Mihon container variant (dead code).
