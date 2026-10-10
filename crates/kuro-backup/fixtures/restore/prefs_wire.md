# Wire layout of the sealed `PreferenceValue` (T-22a-051, T-21a-034)

Source: one real Aniyomi backup, `xyz.jmir.tachiyomi.mi_2026-05-26_18-11.tachibk` (supplied by the user, 2,109,067 bytes
gzip, 3,956,807 bytes of protobuf). It carries 34 app preferences (field 104) and 7 source-preference groups (field 105).
The file holds a personal library, so it is not committed; the records below are cut from it and pinned by
`tests/prefs_wire.rs`.

## Envelope

`Backup.preferences` (field 104) is a `BackupPreference`:

| Field | Wire type | Content |
|---|---|---|
| 1 `key` | length-delimited | the preference key, UTF-8 |
| 2 `value` | length-delimited | the polymorphic envelope below |

The envelope (kotlinx.serialization polymorphic encoding of the sealed class) is a message of two fields:

| Field | Wire type | Content |
|---|---|---|
| 1 | length-delimited | the serial name of the subclass (fully qualified class name) |
| 2 | length-delimited | the subclass message; its single field 1 is `value` |

## The six variants

| Serial name (envelope field 1) | `value` (field 1) wire type | Seen in the dump |
|---|---|---|
| `eu.kanade.tachiyomi.data.backup.models.IntPreferenceValue` | varint (0) | 12 |
| `eu.kanade.tachiyomi.data.backup.models.LongPreferenceValue` | varint (0) | 1 |
| `eu.kanade.tachiyomi.data.backup.models.FloatPreferenceValue` | 32-bit (5) | 1 |
| `eu.kanade.tachiyomi.data.backup.models.StringPreferenceValue` | length-delimited (2) | 9 |
| `eu.kanade.tachiyomi.data.backup.models.BooleanPreferenceValue` | varint (0) | 9 |
| `eu.kanade.tachiyomi.data.backup.models.StringSetPreferenceValue` | length-delimited (2), repeated | 2 |

A value equal to the Kotlin default is still written: `analytics` (false) is `12 02 08 00`, and Int `ext_updates_count`
(0) is `12 02 08 00`. Booleans are a varint of 0 or 1. The set `source_languages` is two repeated field-1 strings.

## Records (hex of one `Backup.preferences` element)

| Key | Value | Hex |
|---|---|---|
| `migrate_flags` | Int 3 | `0a0d6d6967726174655f666c616773123f0a3965752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e496e74507265666572656e636556616c756512020803` |
| `last_ext_check` | Long 1779815489026 | `0a0e6c6173745f6578745f636865636b12450a3a65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e4c6f6e67507265666572656e636556616c756512070882bc94aae633` |
| `analytics` | Boolean false | `0a09616e616c797469637312430a3d65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e426f6f6c65616e507265666572656e636556616c756512020800` |
| `auto_update_metadata` | Boolean true | `0a146175746f5f7570646174655f6d6574616461746112430a3d65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e426f6f6c65616e507265666572656e636556616c756512020801` |
| `pref_app_theme` | String LAVENDER | `0a0e707265665f6170705f7468656d65124a0a3c65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e537472696e67507265666572656e636556616c7565120a0a084c4156454e444552` |
| `pref_player_speed` | Float 1.0 | `0a11707265665f706c617965725f737065656412440a3b65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e466c6f6174507265666572656e636556616c756512050d0000803f` |
| `source_languages` | StringSet {all, en} | `0a10736f757263655f6c616e677561676573124c0a3f65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e537472696e67536574507265666572656e636556616c756512090a03616c6c0a02656e` |

## Enum ordinals

- `UpdateStrategy`: field 105 of `BackupManga` is a varint. Ordinal 1 (`OnlyFetchOnce`) is present on 2 manga in the
  dump; the 115 others omit the field (ordinal 0, `AlwaysUpdate`, the default). Confirmed: 0 and 1.
- `FetchType`: field 507 of `BackupAnime`. None of the 89 anime in this dump carries it (the backup predates the field or
  every value was the default `Episodes`), so the ordinals `Seasons` 0 and `Episodes` 1 come from the Kotlin declaration
  order (`FetchType.kt`), not from the dump. Not confirmed from bytes.

## Unknown fields

Unknown field numbers are skipped in the record and in the envelope (`unknown_fields_are_skipped`, X-03).
