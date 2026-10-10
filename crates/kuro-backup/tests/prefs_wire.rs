//! Wire fixtures for the sealed `PreferenceValue` (T-22a-051, T-21a-034): one record per variant, cut from a real
//! Aniyomi backup (see `fixtures/MANIFEST.md`), and the golden decode of the whole real file when it is present.

use kuro_backup::format::decode_file;
use kuro_backup::schema::{BackupPreference, PrefValue};
use kuro_backup::wire::{Message, Writer};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// `(key, value, bytes of the Backup.preferences element)`, in `fixtures/restore/prefs_wire.md` order.
fn records() -> Vec<(&'static str, PrefValue, Vec<u8>)> {
    let hex = |s: &str| unhex(s);
    vec![
        ("migrate_flags", PrefValue::Int(3), hex("0a0d6d6967726174655f666c616773123f0a3965752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e496e74507265666572656e636556616c756512020803")),
        ("last_ext_check", PrefValue::Long(1779815489026), hex("0a0e6c6173745f6578745f636865636b12450a3a65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e4c6f6e67507265666572656e636556616c756512070882bc94aae633")),
        ("analytics", PrefValue::Boolean(false), hex("0a09616e616c797469637312430a3d65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e426f6f6c65616e507265666572656e636556616c756512020800")),
        ("auto_update_metadata", PrefValue::Boolean(true), hex("0a146175746f5f7570646174655f6d6574616461746112430a3d65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e426f6f6c65616e507265666572656e636556616c756512020801")),
        ("pref_app_theme", PrefValue::String("LAVENDER".into()), hex("0a0e707265665f6170705f7468656d65124a0a3c65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e537472696e67507265666572656e636556616c7565120a0a084c4156454e444552")),
        ("pref_player_speed", PrefValue::Float(1.0), hex("0a11707265665f706c617965725f737065656412440a3b65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e466c6f6174507265666572656e636556616c756512050d0000803f")),
        ("source_languages", PrefValue::StringSet(["all".to_string(), "en".to_string()].into_iter().collect()), hex("0a10736f757263655f6c616e677561676573124c0a3f65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e537472696e67536574507265666572656e636556616c756512090a03616c6c0a02656e")),
    ]
}

#[test]
fn each_variant_decodes_from_real_bytes_and_encodes_back_byte_equal() {
    for (key, value, bytes) in records() {
        let decoded = BackupPreference::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, BackupPreference { key: key.into(), value }, "{key}");
        assert_eq!(decoded.to_bytes().unwrap(), bytes, "{key}");
    }
}

/// X-03: an unknown field next to the known ones is skipped, in the record and in its envelope.
#[test]
fn unknown_fields_are_skipped() {
    let (key, value, _) = records().remove(0);
    let mut w = Writer::new();
    w.string(1, key);
    w.string(15, "extra");
    let mut inner = Writer::new();
    inner.string(1, "eu.kanade.tachiyomi.data.backup.models.IntPreferenceValue");
    let mut v = Writer::new();
    v.int32(1, 3);
    v.int32(9, 7);
    inner.bytes(2, &v.finish());
    inner.int32(9, 1);
    w.bytes(2, &inner.finish());
    let decoded = BackupPreference::from_bytes(&w.finish()).unwrap();
    assert_eq!(decoded, BackupPreference { key: key.into(), value });
}

/// Where the user-supplied Aniyomi backup lives. The file holds a personal library, so it is not committed; set
/// `KURO_ANIYOMI_BACKUP` to point at another copy.
fn real_backup() -> Option<std::path::PathBuf> {
    let from_env = std::env::var_os("KURO_ANIYOMI_BACKUP").map(std::path::PathBuf::from);
    let default = std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Downloads/xyz.jmir.tachiyomi.mi_2026-05-26_18-11.tachibk"));
    from_env.or(default).filter(|p| p.is_file())
}

/// T-21a-034: the counts of `fixtures/MANIFEST.md`. Skipped (it passes) when the real file is not on this machine.
#[test]
fn golden_decode_current_all() {
    let Some(path) = real_backup() else { return };
    let b = decode_file(&path).unwrap();
    assert!(!b.is_legacy);
    assert_eq!(
        (b.manga.len(), b.categories.len(), b.sources.len(), b.preferences.len(), b.source_preferences.len(), b.manga_extension_repos.len()),
        (117, 2, 6, 34, 7, 1)
    );
    assert_eq!(
        (b.anime.len(), b.anime_categories.len(), b.anime_sources.len(), b.extensions.len(), b.anime_extension_stores.len(), b.custom_buttons.len()),
        (89, 4, 7, 12, 2, 1)
    );
    let kinds = |f: fn(&PrefValue) -> bool| b.preferences.iter().filter(|p| f(&p.value)).count();
    assert_eq!(kinds(|v| matches!(v, PrefValue::Int(_))), 12);
    assert_eq!(kinds(|v| matches!(v, PrefValue::Long(_))), 1);
    assert_eq!(kinds(|v| matches!(v, PrefValue::Float(_))), 1);
    assert_eq!(kinds(|v| matches!(v, PrefValue::String(_))), 9);
    assert_eq!(kinds(|v| matches!(v, PrefValue::Boolean(_))), 9);
    assert_eq!(kinds(|v| matches!(v, PrefValue::StringSet(_))), 2);
    // UpdateStrategy ordinal 1 is OnlyFetchOnce; 2 manga carry it, every other entry omits the default (ordinal 0).
    let once = b.manga.iter().filter(|m| m.update_strategy as u8 == 1).count();
    assert_eq!(once, 2);
}
