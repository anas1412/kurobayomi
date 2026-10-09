# 22-backup-restore — topic: decoding the backup file (restore side)

Owner of the wire models: area:21-backup-format-and-create. This topic fixes the exact decode pipeline the restore path depends on. Field tables are in p1-data-and-config.md §1-§3.

## 1. Pipeline (as implemented in Kotlin, `BackupDecoder.decode` + `BackupDetector` + `LegacyBackup.toBackup`)

```
open(uri) -> stream
peek 2 bytes                          EOF -> EOFException (unwrapped)
  0x1f 0x8b  -> gunzip stream
  0x7b 0x7d | 0x7b 0x22 | 0x7b 0x0a -> IOException(invalid_backup_file_json)
  otherwise  -> raw stream
bytes = read all
if BackupDetector.isLegacy(bytes):    // fields 103 and 500 only
     Backup = LegacyBackup.decode(bytes).toBackup()
else:
     Backup = Backup.decode(bytes)
SerializationException (any) -> IOException(invalid_backup_file_unknown)
```

Notes:
- The peek reads the first two bytes of the raw stream before gzip is applied. For a gzip file, the peek sees `1f 8b`.
- `source.gzip()` consumes the stream; the raw branch uses the same buffered source.
- The detector decodes the full bytes once with a reduced message (fields 103 and 500). Protobuf ignores unknown fields, so the reduced decode is cheap and safe.

## 2. Rust equivalent (restore calls this, area:21 implements it)

```rust
// src/backup/format.rs (area:21)
pub fn read_bytes(path: &std::path::Path) -> Result<Vec<u8>, RestoreError> {
    // 1. open; read at most 2 bytes; if fewer than 2 bytes: Err(Decode("Backup file is empty")) for 0 bytes,
    //    Err(Decode("Backup file is corrupted")) for 1 byte (PROPOSAL; Kotlin text differs, see E-01/E-02).
    // 2. magic check as in §1 (same three JSON signatures; gzip via flate2::read::GzDecoder).
    // 3. read all bytes (Vec<u8>); gzip errors -> Err(Decode("Backup file is corrupted")) with the detail kept.
}

pub fn decode(bytes: &[u8]) -> Result<Backup, RestoreError> {
    if detect_legacy(bytes)? { LegacyBackup::decode(bytes)?.into_backup() } else { Backup::decode(bytes)? }
}

fn detect_legacy(bytes: &[u8]) -> Result<bool, RestoreError> {
    // Decode only tags 103 (repeated message DetectAnimeSource) and 500 (bool, default true).
    // legacy = is_legacy_500_or_default && !anime_sources_103.is_empty()
}
```

Decode error mapping (must match behaviour §3.1):
- any `prost::DecodeError` or missing required field -> `Decode("Backup file is corrupted")`.
- gzip read error -> `Decode("Backup file is corrupted")`, detail kept for "Full error".
- empty input -> `Decode("Backup file is empty")` (PROPOSAL).
- JSON signature -> `Decode("JSON backup not supported")`.
- Zero library entries and zero other content (PROPOSAL, E-04): `Decode("Backup does not contain any library entries.")`.

## 3. Decoding rules that prost must reproduce

1. Repeated message fields: append in order of appearance (repeated message = list).
2. Repeated scalar (`List<Long>`, `List<String>`): accept packed and unpacked forms.
3. Last occurrence wins for singular scalar fields (protobuf rule). Affects `BackupCategory.name` and similar.
4. Unknown fields: skipped. Verify with a fixture that contains a field number outside the schema (T-22a-051).
5. Missing optional fields: `None`; the accessor applies the Kotlin default (table in p1-data-and-config.md §3).
6. Missing required fields (no default in Kotlin): decode error (`MissingFieldException` is a `SerializationException`).
7. Enum: varint ordinal; an out-of-range ordinal is expected to fail the decode (verify with fixture T-22a-051). Use `TryFrom<i32>` and map the error to corrupted.
8. Float (`fixed32`) and Double (`fixed64`): IEEE 754, little endian.
9. `Int` fields are varint int32; negative values are encoded as 10-byte sign-extended varints. Decode with `as i32` from `u64`.
10. Strings: UTF-8; invalid UTF-8 is a decode error in kotlinx (`SerializationException`). Rust: `String::from_utf8` error -> corrupted.

## 4. The sealed PreferenceValue wire form

Status: not derivable from the Kotlin source alone. kotlinx encodes sealed classes polymorphically. The restore decoder needs the exact encoding.

Plan (T-22a-051):
1. Create a backup with one `IntPreferenceValue`, one `StringSetPreferenceValue` and one `BooleanPreferenceValue` in the Kotlin app (or from an existing Mihon/Aniyomi backup file on the test machine).
2. Dump the bytes of field 2 of each `BackupPreference` with a protobuf inspector (`protoc --decode_raw` style).
3. Record the tag numbers and wire types of the polymorphic envelope and the value field in `fixtures/restore/prefs_wire.md`.
4. Implement the decoder for the six variants from the recorded layout. Accept unknown variant tags as "skipped preference" (count, do not fail the whole file).

Until T-22a-051 is done, prefs decoding is blocked; restore of app and source preferences is not shippable.

## 5. Legacy conversion details

`LegacyBackup.toBackup()` sets `isLegacy = false`, copies manga/categories/sources/preferences/source preferences/manga extension repos from the same numbers; anime from 3/4/103 to the new fields; extensions (106), anime extension stores (107), custom buttons (109) directly. Fields 100 and 102 are reserved and not decoded (the legacy source model with a non-compliant number, commented in source).

Test: X-06 with Q-02.

## 6. Size and performance

- Entire file is read into memory (`BackupDecoder.kt:116`). A 100 MB backup needs ~100 MB plus the decoded graph (Rust structs are typically larger than the wire form). Document the limit; no streaming.
- Decode once per run. Validation (on the restore page) and the job should share the decoded value (E-13).
