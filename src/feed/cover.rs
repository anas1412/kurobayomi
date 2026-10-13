//! Cover thumbnails for the widgets (T-50a-013, T-50a-014). The app renders a rounded 58:87 thumbnail per cover into
//! the widget cover cache; the CLI only looks the file up and never downloads, decodes or writes (T-50a-016).

use std::path::Path;


use super::UpdateRow;

/// File stem of a cover's thumbnail: SHA-256 of `<thumbnail url>|<cover last modified>`, so a changed cover is
/// rendered again.
pub fn cache_key(url: &str, last_modified: i64) -> String {
    kuro_core::util::hash::sha256_hex(format!("{url}|{last_modified}").as_bytes())
}

fn file_name(key: &str) -> String {
    format!("{key}.png")
}

/// Path of the cached thumbnail of a row, or `None` (placeholder) when it has no cover URL or no file yet.
pub fn cached_path(cache_dir: &Path, row: &UpdateRow) -> Option<String> {
    let url = row.thumbnail_url.as_deref().filter(|u| !u.is_empty())?;
    let path = cache_dir.join(file_name(&cache_key(url, row.cover_last_modified)));
    path.is_file().then(|| path.to_str().map(str::to_owned)).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    

    fn row(url: Option<&str>, lm: i64) -> UpdateRow {
        UpdateRow { entry_id: 1, title: "t".into(), update_id: 1, thumbnail_url: url.map(str::to_owned), cover_last_modified: lm }
    }

    #[test]
    fn key_changes_with_the_modified_time() {
        assert_ne!(cache_key("u", 1), cache_key("u", 2));
        assert_eq!(cache_key("file:///covers/a.jpg", 5), "3fcfa9fb9e6a59bf28fe1380e51adb69b792d4b092d57a9c91ff7aba6d1c45e7");
    }

    #[test]
    fn lookup_is_cache_only() {
        let dir = tempfile::tempdir().unwrap();
        let r = row(Some("file:///c.jpg"), 3);
        assert_eq!(cached_path(dir.path(), &r), None);
        let p = dir.path().join(file_name(&cache_key("file:///c.jpg", 3)));
        std::fs::write(&p, b"x").unwrap();
        assert_eq!(cached_path(dir.path(), &r).as_deref(), p.to_str());
        assert_eq!(cached_path(dir.path(), &row(None, 3)), None);
    }

}
