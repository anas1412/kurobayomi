//! Container reading: magic-byte sniffing, gzip, legacy detection and the decode entry point (area 21,
//! T-21a-007, T-21a-008, T-21a-009). Never parses JSON.

use std::io::Read;
use std::path::Path;

use crate::error::BackupError;
use crate::schema::{Backup, BackupAnimeSource, LegacyBackup};
use crate::wire::{Message, Reader, WireError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sniffed {
    Gzip,
    JsonForbidden,
    Raw,
    /// Fewer than two bytes.
    Truncated,
}

/// Classifies a file by its first two bytes (B-11, E-01, E-02).
pub fn sniff(first_two: &[u8]) -> Sniffed {
    match first_two {
        [a, b, ..] if [*a, *b] == [0x1F, 0x8B] => Sniffed::Gzip,
        [0x7B, 0x7D] | [0x7B, 0x22] | [0x7B, 0x0A] => Sniffed::JsonForbidden,
        [_, _, ..] => Sniffed::Raw,
        _ => Sniffed::Truncated,
    }
}

/// Reads the file and returns the protobuf payload, gunzipped when needed.
pub fn read_backup_bytes(path: &Path) -> Result<Vec<u8>, BackupError> {
    let raw = std::fs::read(path)?;
    payload_of(&raw)
}

/// The protobuf payload of an in-memory file image.
pub fn payload_of(raw: &[u8]) -> Result<Vec<u8>, BackupError> {
    match sniff(raw.get(..2).unwrap_or(&[])) {
        Sniffed::Gzip => {
            let mut out = Vec::new();
            flate2::read::MultiGzDecoder::new(raw)
                .read_to_end(&mut out)
                .map_err(|e| BackupError::Corrupt(format!("gzip: {e}")))?;
            Ok(out)
        }
        Sniffed::JsonForbidden => Err(BackupError::JsonNotSupported),
        Sniffed::Raw => Ok(raw.to_vec()),
        Sniffed::Truncated => Err(BackupError::Corrupt("file shorter than two bytes".into())),
    }
}

/// Partial decode of fields 500 and 103 (B-12). Any decode error counts as "not legacy" (matches Kotlin's
/// `SerializationException -> false`).
pub fn is_legacy(payload: &[u8]) -> bool {
    detect(payload).unwrap_or(false)
}

fn detect(payload: &[u8]) -> Result<bool, WireError> {
    let mut r = Reader::new(payload);
    let mut legacy_flag: Option<bool> = None;
    let mut anime_sources = 0usize;
    while let Some((f, wt)) = r.next_tag()? {
        match f {
            500 => legacy_flag = Some(r.boolean(f, wt)?),
            103 => {
                let _: BackupAnimeSource = r.message(f, wt)?;
                anime_sources += 1;
            }
            _ => r.skip(wt)?,
        }
    }
    Ok(legacy_flag.unwrap_or(true) && anime_sources > 0)
}

/// Decodes a payload as the current container, or as the legacy one when detected, converted to current.
pub fn decode_payload(payload: &[u8]) -> Result<Backup, BackupError> {
    if is_legacy(payload) {
        Ok(LegacyBackup::from_bytes(payload)?.into_current())
    } else {
        Ok(Backup::from_bytes(payload)?)
    }
}

/// Reads and decodes a backup file. Used by restore and by the post-write self-check.
pub fn decode_file(path: &Path) -> Result<Backup, BackupError> {
    decode_payload(&read_backup_bytes(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{BackupAnime, BackupManga};
    use crate::wire::Writer;
    use std::io::Write;

    fn gz(payload: &[u8]) -> Vec<u8> {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(payload).unwrap();
        enc.finish().unwrap()
    }

    fn fields(payload: &[u8]) -> Vec<u32> {
        let mut out = Vec::new();
        let mut r = Reader::new(payload);
        while let Some((f, wt)) = r.next_tag().unwrap() {
            out.push(f);
            r.skip(wt).unwrap();
        }
        out
    }

    #[test]
    fn sniff_table() {
        assert_eq!(sniff(&[0x1F, 0x8B]), Sniffed::Gzip);
        assert_eq!(sniff(&[0x7B, 0x7D]), Sniffed::JsonForbidden);
        assert_eq!(sniff(&[0x7B, 0x22]), Sniffed::JsonForbidden);
        assert_eq!(sniff(&[0x7B, 0x0A]), Sniffed::JsonForbidden);
        assert_eq!(sniff(&[0x20, 0x7B, 0x22]), Sniffed::Raw, "leading whitespace is raw protobuf (E-02)");
        assert_eq!(sniff(&[0x1F]), Sniffed::Truncated);
        assert_eq!(sniff(&[]), Sniffed::Truncated);
    }

    #[test]
    fn gzip_payload_is_read_completely() {
        let payload = Backup::default().to_bytes().unwrap();
        let got = payload_of(&gz(&payload)).unwrap();
        assert_eq!(got, payload);
    }

    #[test]
    fn truncated_gzip_is_corrupt_not_panic() {
        let full = gz(&vec![0x08u8; 4096]);
        let cut = &full[..full.len() / 2];
        assert!(matches!(payload_of(cut), Err(BackupError::Corrupt(_))));
    }

    #[test]
    fn json_is_rejected_with_key_text_and_never_parsed() {
        let err = payload_of(b"{\"manga\": []}").unwrap_err();
        assert!(matches!(err, BackupError::JsonNotSupported));
        assert_eq!(err.user_text(), "JSON backup not supported");
    }

    #[test]
    fn raw_protobuf_reads_without_gzip() {
        let payload = Backup::default().to_bytes().unwrap();
        assert_eq!(payload_of(&payload).unwrap(), payload);
    }

    #[test]
    fn one_byte_file_is_corrupt() {
        let err = payload_of(&[0x1F]).unwrap_err();
        assert_eq!(err.user_text(), "Backup file is corrupted");
    }

    #[test]
    fn legacy_detect_true() {
        let mut w = Writer::new();
        w.message(3, &BackupAnime { source: 1, url: "a".into(), ..Default::default() }).unwrap();
        w.message(103, &BackupAnimeSource { name: "s".into(), source_id: 1 }).unwrap();
        assert!(is_legacy(&w.finish()));
    }

    #[test]
    fn legacy_detect_false_current() {
        let mut w = Writer::new();
        w.boolean(500, false);
        w.message(501, &BackupAnime { source: 1, url: "a".into(), ..Default::default() }).unwrap();
        assert!(!is_legacy(&w.finish()));
        let cur = Backup {
            is_legacy: false,
            anime: vec![BackupAnime { source: 1, url: "a".into(), ..Default::default() }],
            ..Default::default()
        };
        assert!(!is_legacy(&cur.to_bytes().unwrap()));
    }

    #[test]
    fn legacy_detect_false_mihon() {
        let mut w = Writer::new();
        w.message(1, &BackupManga { source: 1, url: "m".into(), ..Default::default() }).unwrap();
        w.message(2, &crate::schema::BackupCategory { name: "c".into(), ..Default::default() }).unwrap();
        w.message(101, &crate::schema::BackupSource { name: "s".into(), source_id: 1 }).unwrap();
        assert!(!is_legacy(&w.finish()));
    }

    #[test]
    fn legacy_detect_false_no_sources() {
        // Documented weakness (B-12): legacy file with no anime sources is read as current.
        let mut w = Writer::new();
        w.message(3, &BackupAnime { source: 1, url: "a".into(), ..Default::default() }).unwrap();
        assert!(!is_legacy(&w.finish()));
    }

    #[test]
    fn malformed_input_is_not_legacy() {
        assert!(!is_legacy(&[0x0A, 0x20, 0x01]));
        assert!(!is_legacy(&[0x00]));
    }

    #[test]
    fn legacy_file_decodes_to_current_lists() {
        let mut w = Writer::new();
        w.message(3, &BackupAnime { source: 1, url: "a".into(), ..Default::default() }).unwrap();
        w.message(103, &BackupAnimeSource { name: "s".into(), source_id: 1 }).unwrap();
        w.message(106, &crate::schema::BackupExtension { pkg_name: "e".into(), apk: vec![1] }).unwrap();
        let cur = decode_payload(&w.finish()).unwrap();
        assert!(!cur.is_legacy);
        assert_eq!(cur.anime.len(), 1);
        assert_eq!(cur.anime_sources.len(), 1);
        assert_eq!(cur.extensions.len(), 1);
        assert_eq!(fields(&Backup::default().to_bytes().unwrap()), vec![500]);
    }

    /// T-21a-043: a Mihon-shaped file (manga, categories, sources; no 103, no 500) reads as the current container, and
    /// its preflight reports no anime sources and no trackers.
    #[test]
    fn mihon_shaped_manga_file_decodes_as_current() {
        use crate::creator::SourceRegistry;
        use crate::validate::{TrackerRegistry, preflight};

        let mut manga = Writer::new();
        manga.int64(1, 7);
        manga.string(2, "/m/1");
        manga.int32(14, 2);
        let mut cat = Writer::new();
        cat.string(1, "Reading");
        cat.int64(2, 1);
        cat.int64(3, 4);
        let mut src = Writer::new();
        src.string(1, "Mock");
        src.int64(2, 7);
        let mut w = Writer::new();
        w.bytes(1, &manga.finish());
        w.bytes(2, &cat.finish());
        w.bytes(101, &src.finish());
        // Field 104: every real Mihon or Aniyomi file carries app preferences.
        w.message(104, &crate::schema::BackupPreference { key: "theme".into(), value: crate::schema::PrefValue::String("SYSTEM".into()) }).unwrap();
        w.message(104, &crate::schema::BackupPreference { key: "n".into(), value: crate::schema::PrefValue::Int(4) }).unwrap();
        let payload = w.finish();

        assert!(!is_legacy(&payload));
        let backup = decode_payload(&payload).unwrap();
        assert_eq!(backup.manga.len(), 1);
        assert_eq!(backup.manga[0].source, 7);
        assert_eq!(backup.manga[0].effective_viewer_flags(), 2, "viewer (14) is used without 103");
        assert_eq!(backup.categories[0].name, "Reading");
        assert_eq!(backup.sources[0].source_id, 7);
        assert!(backup.anime.is_empty() && backup.anime_sources.is_empty());
        assert_eq!(backup.preferences.len(), 2);
        assert_eq!(backup.preferences[1].value, crate::schema::PrefValue::Int(4));

        struct Installed;
        impl SourceRegistry for Installed {
            fn is_installed(&self, id: i64) -> bool {
                id == 7
            }
            fn name_of(&self, _: i64) -> String {
                "Mock".into()
            }
            fn stub_label(&self, id: i64) -> String {
                id.to_string()
            }
        }
        struct NoTrackers;
        impl TrackerRegistry for NoTrackers {
            fn name(&self, _: i32) -> Option<String> {
                None
            }
            fn logged_in(&self, _: i32) -> bool {
                false
            }
        }
        let report = preflight(&backup, &Installed, &NoTrackers);
        assert!(report.missing_sources.is_empty());
        assert!(report.missing_trackers.is_empty());
    }
}
