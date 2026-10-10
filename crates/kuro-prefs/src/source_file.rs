//! Per-source preference files: `<config>/sources/source_<id>.toml` (T-02a-027).
//!
//! Each source has its own file, so two sources never share one. Each key is written as
//! `key = { t = "long", v = 5 }`, the same tagged form the main file uses for unknown keys, so an
//! Int and a Long read back as the kind they were written with. Writes use the main store's
//! debounced flusher and the same temp-file-then-rename save.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Value};

use crate::store::{Backend, PrefStore};
use crate::toml_backend::{decode_as, tagged, write_atomic};
use crate::values::PrefValue;
use crate::PrefError;

/// Directory under the config directory that holds one file per source.
pub const SOURCES_DIR: &str = "sources";

/// `<config_dir>/sources/source_<id>.toml`.
pub fn source_file_path(config_dir: &Path, source_id: i64) -> PathBuf {
    config_dir.join(SOURCES_DIR).join(format!("source_{source_id}.toml"))
}

/// Reads and writes one source's values in one file.
pub struct SourceFileBackend {
    path: PathBuf,
}

impl SourceFileBackend {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Backend for SourceFileBackend {
    fn load(&self) -> Result<BTreeMap<String, PrefValue>, PrefError> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(e.into()),
        };
        let doc: DocumentMut = text
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", self.path.display())))?;
        let mut out = BTreeMap::new();
        for (key, item) in doc.as_table().iter() {
            let decoded = item
                .as_value()
                .and_then(Value::as_inline_table)
                .and_then(|t| decode_as(t.get("v")?, t.get("t")?.as_str()?));
            match decoded {
                Some(v) => {
                    out.insert(key.to_owned(), v);
                }
                None => tracing::warn!("Invalid value for {key} in {}; ignoring", self.path.display()),
            }
        }
        Ok(out)
    }

    fn save(&self, all: &BTreeMap<String, PrefValue>) -> Result<(), PrefError> {
        let mut doc = DocumentMut::new();
        for (key, v) in all {
            doc.as_table_mut()
                .insert(key, Item::Value(Value::InlineTable(tagged(v))));
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|source| PrefError::Dir {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        write_atomic(&self.path, &doc.to_string())
    }
}

/// The preferences of one source, in `sources/source_<id>.toml`. It derefs to [`PrefStore`], so the
/// settings widgets bind to it the same way they bind to the main store.
pub struct SourceFilePrefs(PrefStore);

impl SourceFilePrefs {
    /// Opens the file of `source_id` under `config_dir`, creating nothing until a value is written.
    pub fn open(config_dir: &Path, source_id: i64) -> Result<Self, PrefError> {
        let backend = SourceFileBackend::new(source_file_path(config_dir, source_id));
        Ok(Self(PrefStore::new(Box::new(backend))?))
    }
}

impl Deref for SourceFilePrefs {
    type Target = PrefStore;

    fn deref(&self) -> &PrefStore {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref::Pref;
    use crate::values::PrefValue;

    fn dir() -> tempfile::TempDir {
        tempfile::TempDir::new().unwrap()
    }

    #[test]
    fn each_source_has_its_own_file_under_sources() {
        let d = dir();
        assert_eq!(source_file_path(d.path(), 7), d.path().join("sources").join("source_7.toml"));
        let a = SourceFilePrefs::open(d.path(), 7).unwrap();
        let b = SourceFilePrefs::open(d.path(), 8).unwrap();
        a.set_raw("k", PrefValue::Bool(true));
        a.flush().unwrap();
        b.flush().unwrap();
        assert!(d.path().join("sources/source_7.toml").is_file());
        assert_eq!(b.get_raw("k"), None, "source 8 never sees source 7's value");
    }

    #[test]
    fn int_long_float_string_bool_and_set_survive_a_restart() {
        let d = dir();
        let tags: std::collections::BTreeSet<String> = ["q\"uote", "back\\slash", "ü"].into_iter().map(str::to_owned).collect();
        {
            let s = SourceFilePrefs::open(d.path(), 3).unwrap();
            s.set_raw("i", PrefValue::Int(-4));
            s.set_raw("l", PrefValue::Long(9_000_000_000));
            s.set_raw("f", PrefValue::Float(0.85));
            s.set_raw("s", PrefValue::Str("hi \"there\"".into()));
            s.set_raw("b", PrefValue::Bool(false));
            s.set_raw("set", PrefValue::Set(tags.clone()));
            s.flush().unwrap();
        }
        let s = SourceFilePrefs::open(d.path(), 3).unwrap();
        assert_eq!(s.get_raw("i"), Some(PrefValue::Int(-4)));
        assert_eq!(s.get_raw("l"), Some(PrefValue::Long(9_000_000_000)));
        assert_eq!(s.get_raw("f"), Some(PrefValue::Float(0.85)));
        assert_eq!(s.get_raw("s"), Some(PrefValue::Str("hi \"there\"".into())));
        assert_eq!(s.get_raw("b"), Some(PrefValue::Bool(false)));
        assert_eq!(s.get_raw("set"), Some(PrefValue::Set(tags)));
    }

    #[test]
    fn a_wrong_kind_read_deletes_the_key_and_returns_the_default() {
        let d = dir();
        let s = SourceFilePrefs::open(d.path(), 5).unwrap();
        s.set_raw("quality", PrefValue::Str("high".into()));
        let pref: Pref<i32> = s.pref("quality", 80);
        assert_eq!(pref.get(), 80);
        assert_eq!(s.get_raw("quality"), None);
    }

    #[test]
    fn a_value_set_just_before_quit_is_on_disk_after_a_bounded_shutdown_flush() {
        let d = dir();
        let s = SourceFilePrefs::open(d.path(), 21).unwrap();
        s.set_raw("quality", PrefValue::Int(2));
        let started = std::time::Instant::now();
        let done = s.flush_within(crate::store::SHUTDOWN_FLUSH_TIMEOUT);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(matches!(done, Some(Ok(()))), "{done:?}");
        drop(s);
        let again = SourceFilePrefs::open(d.path(), 21).unwrap();
        assert_eq!(again.get_raw("quality"), Some(PrefValue::Int(2)));
    }

    #[test]
    fn a_missing_file_opens_empty_and_writes_nothing() {
        let d = dir();
        let s = SourceFilePrefs::open(d.path(), 11).unwrap();
        assert!(s.all().is_empty());
    }

    #[test]
    fn a_corrupt_file_is_an_error_not_an_empty_store() {
        let d = dir();
        fs::create_dir_all(d.path().join("sources")).unwrap();
        fs::write(d.path().join("sources/source_2.toml"), "not = [valid").unwrap();
        assert!(SourceFilePrefs::open(d.path(), 2).is_err());
    }
}
