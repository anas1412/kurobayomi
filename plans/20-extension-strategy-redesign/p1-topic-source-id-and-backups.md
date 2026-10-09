# Topic: source ids, stubs and Aniyomi backups

## Why ids must be preserved

Every library entry, chapter, history row and backup entry stores the source id as a 64-bit integer. Changing the derivation breaks every existing user's library. The derivation is therefore frozen and reproduced exactly.

## Derivation (observed)

`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt` and `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt`:

```
key   = "${name.lowercase()}/$lang/$versionId"
md5   = MD5(UTF-8(key))
id    = big-endian 64-bit value of md5[0..8]  AND  Long.MAX_VALUE
```

Defaults: `versionId = 1`. The id is therefore always non-negative.

Candidate test vector from the observed index (`aniyomiorg/aniyomi-extensions`, branch `repo`): the source named `Google Drive` with `lang` `all` appears with id `4222017068256633289`. This is NOT yet verified as the output of `generateId("Google Drive","all",1)`. The implementer must run the Kotlin derivation once (a one-line JVM snippet is enough) and record the golden value before using it. This is required step E12 item 1.

## Stub sources

A stub row stores `(id, lang, name)`. When an entry refers to an id that is not loaded:
- the entry, its chapters and history remain;
- the source name and language come from the stub row, so the UI can show "Name (LANG) - not installed";
- reader, search and detail refresh are disabled with message key `source_not_installed`.

When the extension is installed again, the same id resolves to the live source. No migration runs.

## Backup import

Aniyomi backups (protobuf or JSON, format owned by area:21-backup-format-and-create if present in the plan; this topic only covers ids) carry `source` as an integer. Import rule:
1. Look up `source` in the live table. Found -> link.
2. Not found -> create stub row from the backup's `source` name if the backup stores it, otherwise `name = ""`, `lang = ""`.
3. Never rewrite the id.

## Renamed or versioned sources

- Name change upstream changes the id. The old id becomes a stub and the new id is a separate source. The UI offers "merge" (planned in area:09-domain-source-track-download-release-misc), which rewrites the library rows to the new id. Not implemented in this area.
- Version bump (`versionId` 1 -> 2) changes the id on purpose so a broken source does not overwrite data. Same as Aniyomi.

## Open question (not decided here)

Whether the Rust side should keep the `name.lowercase()` Unicode behaviour or ASCII lowercase for future sources. The existing ids require Unicode lowercase for any name that has non-ASCII letters. Decision: keep Unicode lowercase (`str::to_lowercase`), because that is what upstream does for non-ASCII names.
