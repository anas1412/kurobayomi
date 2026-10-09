//! Per-video mpv options from file metadata (area 45 `PlayerActivity.setMpvOptions`, T-45a-013).
//! A downloaded file may carry `ANIYOMI_MPV_ARGS` in its container metadata: `option=value` pairs
//! separated by `;`. Values may contain `=`. The caller reads the mpv `metadata` property after
//! `FILE_LOADED` and passes its JSON text here.

use std::collections::HashMap;

use crate::mpv::PropertySink;

/// The metadata key that carries the per-video options (`Video.MPV_ARGS_TAG`).
pub const MPV_ARGS_TAG: &str = "ANIYOMI_MPV_ARGS";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MetadataError {
    #[error("metadata is not a string map: {0}")]
    NotAMap(String),
    #[error("option has no value: {0}")]
    NoValue(String),
}

/// Only local videos (`file`, `content`, `data`) are read for metadata (`setMpvOptions`).
pub fn reads_metadata(url: &str) -> bool {
    ["file", "content", "data"]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

/// The `option=value` pairs of `ANIYOMI_MPV_ARGS`, in order. No entry gives an empty list. One
/// malformed entry fails the whole list, so nothing is applied from a broken tag.
pub fn mpv_args(metadata_json: &str) -> Result<Vec<(String, String)>, MetadataError> {
    let map: HashMap<String, String> =
        serde_json::from_str(metadata_json).map_err(|e| MetadataError::NotAMap(e.to_string()))?;
    let Some(raw) = map.get(MPV_ARGS_TAG) else {
        return Ok(Vec::new());
    };
    raw.split(';')
        .map(|entry| {
            entry
                .split_once('=')
                .map(|(option, value)| (option.to_owned(), value.to_owned()))
                .ok_or_else(|| MetadataError::NoValue(entry.to_owned()))
        })
        .collect()
}

/// Reads the metadata of a local video and sets its options as mpv properties. Returns how many
/// were set. A parse error is logged and sets nothing, and playback goes on. A failed write stops
/// the rest and is logged the same way.
pub fn apply_from_metadata(sink: &impl PropertySink, url: &str, metadata: Option<&str>) -> usize {
    let Some(metadata) = metadata.filter(|_| reads_metadata(url)) else {
        return 0;
    };
    let args = match mpv_args(metadata) {
        Ok(args) => args,
        Err(e) => {
            tracing::error!("Failed to read video metadata: {e}");
            return 0;
        }
    };
    for (applied, (option, value)) in args.iter().enumerate() {
        if let Err(e) = sink.set_str(option, value) {
            tracing::error!("Failed to read video metadata: {e}");
            return applied;
        }
    }
    args.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpv::MpvError;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Recorder(RefCell<Vec<(String, String)>>);

    impl PropertySink for Recorder {
        fn set_str(&self, name: &str, value: &str) -> Result<(), MpvError> {
            self.0
                .borrow_mut()
                .push((name.to_owned(), value.to_owned()));
            Ok(())
        }
    }

    fn metadata_with(args: &str) -> String {
        serde_json::json!({ MPV_ARGS_TAG: args, "title": "x" }).to_string()
    }

    #[test]
    fn splits_each_entry_at_the_first_equals_sign() {
        let args = mpv_args(&metadata_with("ytdl-format=best;http-header-fields=A=b")).unwrap();
        assert_eq!(
            args,
            vec![
                ("ytdl-format".to_owned(), "best".to_owned()),
                ("http-header-fields".to_owned(), "A=b".to_owned()),
            ]
        );
    }

    #[test]
    fn missing_tag_gives_no_options() {
        let json = serde_json::json!({ "title": "x" }).to_string();
        assert_eq!(mpv_args(&json).unwrap(), Vec::new());
    }

    #[test]
    fn malformed_metadata_is_an_error() {
        assert!(matches!(
            mpv_args("not json"),
            Err(MetadataError::NotAMap(_))
        ));
        assert!(matches!(
            mpv_args(&metadata_with("a=1;broken")),
            Err(MetadataError::NoValue(entry)) if entry == "broken"
        ));
    }

    #[test]
    fn only_local_videos_are_read() {
        assert!(reads_metadata("file:///a.mkv"));
        assert!(reads_metadata("content://x"));
        assert!(reads_metadata("data://abc"));
        assert!(!reads_metadata("https://example.test/a.m3u8"));
    }

    #[test]
    fn applies_options_to_a_local_video() {
        let rec = Recorder::default();
        let meta = metadata_with("speed=1.5");
        assert_eq!(apply_from_metadata(&rec, "file:///a.mkv", Some(&meta)), 1);
        assert_eq!(
            *rec.0.borrow(),
            vec![("speed".to_owned(), "1.5".to_owned())]
        );
    }

    #[test]
    fn online_video_and_bad_metadata_set_nothing() {
        let rec = Recorder::default();
        let meta = metadata_with("speed=1.5");
        assert_eq!(apply_from_metadata(&rec, "https://x/a", Some(&meta)), 0);
        assert_eq!(apply_from_metadata(&rec, "file:///a.mkv", Some("{")), 0);
        assert_eq!(apply_from_metadata(&rec, "file:///a.mkv", None), 0);
        assert!(rec.0.borrow().is_empty());
    }
}
