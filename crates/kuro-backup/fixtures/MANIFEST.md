# Aniyomi-produced fixtures (T-21a-034)

| File | App | Date | Option set | Committed |
|---|---|---|---|---|
| `xyz.jmir.tachiyomi.mi_2026-05-26_18-11.tachibk` | Aniyomi (`xyz.jmir.tachiyomi.mi`) | 2026-05-26 18:11 | all options on (library, categories, preferences, source preferences, extensions, extension stores, custom buttons) | no: personal library, 2.1 MB. It stays in the user's Downloads folder; `KURO_ANIYOMI_BACKUP` points the golden test at another copy. |

Expected decode (`golden_decode_current_all` in `tests/prefs_wire.rs`):

| Part | Count |
|---|---|
| manga (1) / categories (2) / sources (101) | 117 / 2 / 6 |
| preferences (104) / source preferences (105) / manga extension repos (106) | 34 / 7 / 1 |
| anime (501) / anime categories (502) / anime sources (503) | 89 / 4 / 7 |
| extensions (504) / extension stores (505) / custom buttons (506) | 12 / 2 / 1 |
| preference variants Int / Long / Float / String / Boolean / StringSet | 12 / 1 / 1 / 9 / 9 / 2 |
| manga with `UpdateStrategy` ordinal 1 | 2 |

What the file confirms: the six `PreferenceValue` serial names and the envelope (field 1 name, field 2 body), recorded in
`restore/prefs_wire.md` and as `schema::pref_tag`; `UpdateStrategy` ordinals 0 and 1; defaults omitted on entry fields.
What it does not confirm: `FetchType` ordinals (field 507 absent in this file).

Committed evidence: the seven records in `tests/prefs_wire.rs` are byte-exact slices of this file.
