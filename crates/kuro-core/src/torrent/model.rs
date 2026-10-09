//! TorrServer JSON models (`Torrent.kt`, `TorrentRequest.kt`) with the server's field names.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FileStatDto {
    #[serde(default)]
    pub id: Option<i32>,
    pub path: String,
    pub length: i64,
}

/// The server's torrent record. Unknown fields are ignored.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TorrentDto {
    pub title: String,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub torrent_size: Option<i64>,
    #[serde(default)]
    pub trackers: Option<Vec<String>>,
    #[serde(default)]
    pub file_stats: Option<Vec<FileStatDto>>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Body of `POST /torrents`. Empty fields and `save_to_db: false` are omitted.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct TorrentRequest {
    pub action: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hash: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub link: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub poster: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub data: String,
    #[serde(skip_serializing_if = "is_false")]
    pub save_to_db: bool,
}

impl TorrentRequest {
    pub fn add(link: &str, title: &str, save: bool) -> Self {
        Self {
            action: "add".into(),
            hash: String::new(),
            link: link.into(),
            title: title.into(),
            poster: String::new(),
            data: String::new(),
            save_to_db: save,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_request_omits_empty_fields_and_a_false_save_flag() {
        let json = serde_json::to_string(&TorrentRequest::add("magnet:?xt=1", "Ep", false)).unwrap();
        assert_eq!(json, r#"{"action":"add","link":"magnet:?xt=1","title":"Ep"}"#);
    }

    #[test]
    fn add_request_with_save_true_has_the_flag() {
        let json = serde_json::to_string(&TorrentRequest::add("l", "t", true)).unwrap();
        assert!(json.contains(r#""save_to_db":true"#));
    }

    #[test]
    fn dto_decodes_the_server_names_and_ignores_unknown_fields() {
        let t: TorrentDto = serde_json::from_str(
            r#"{"title":"T","hash":"abc","torrent_size":12,"trackers":["u"],"stat":5,"file_stats":[{"id":2,"path":"a/b","length":4,"extra":1}]}"#,
        )
        .unwrap();
        assert_eq!(t.torrent_size, Some(12));
        assert_eq!(t.file_stats, Some(vec![FileStatDto { id: Some(2), path: "a/b".into(), length: 4 }]));
    }

    #[test]
    fn dto_accepts_a_missing_file_stats() {
        let t: TorrentDto = serde_json::from_str(r#"{"title":"T"}"#).unwrap();
        assert_eq!(t.file_stats, None);
        assert_eq!(t.hash, None);
    }

    #[test]
    fn dto_round_trips() {
        let t = TorrentDto { title: "T".into(), hash: Some("h".into()), torrent_size: Some(1), trackers: None, file_stats: None };
        assert_eq!(serde_json::from_str::<TorrentDto>(&serde_json::to_string(&t).unwrap()).unwrap(), t);
    }
}
