//! Source settings descriptors (`ConfigurableSource.kt`, `PreferenceScreen`; T-16a-035). A source declares its rows as
//! plain data and the settings screen renders them. No androidx type is involved. A category nests child rows, and a
//! key may appear only once per source.

use std::collections::BTreeSet;

/// One row of a source's settings screen.
#[derive(Debug, Clone, PartialEq)]
pub enum SourcePref {
    /// A switch. Stored as a bool.
    Switch { key: String, title: String, summary: Option<String>, default: bool },
    /// A single choice. `entry_values[i]` is stored and `entries[i]` is shown.
    List {
        key: String,
        title: String,
        summary: Option<String>,
        entries: Vec<String>,
        entry_values: Vec<String>,
        default: String,
    },
    /// A multi choice. Stored as a set of values.
    MultiSelect {
        key: String,
        title: String,
        summary: Option<String>,
        entries: Vec<String>,
        entry_values: Vec<String>,
        default: Vec<String>,
    },
    /// A text field. Stored as a string; an empty string is allowed.
    EditText { key: String, title: String, summary: Option<String>, default: String },
    /// A heading that nests child rows. It stores nothing.
    Category { title: String, children: Vec<SourcePref> },
}

/// A descriptor list that names one key twice.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrefDescriptorError {
    #[error("duplicate preference key: {0}")]
    DuplicateKey(String),
}

impl SourcePref {
    /// The preference key this row reads and writes. `None` for a category, which never stores a value.
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::Switch { key, .. }
            | Self::List { key, .. }
            | Self::MultiSelect { key, .. }
            | Self::EditText { key, .. } => Some(key),
            Self::Category { .. } => None,
        }
    }
}

/// Rejects a descriptor list in which one key appears twice, at any depth. The list is returned unchanged when it is
/// valid, so a source can build its rows and check them in one expression.
pub fn check_descriptors(prefs: Vec<SourcePref>) -> Result<Vec<SourcePref>, PrefDescriptorError> {
    let mut seen = BTreeSet::new();
    collect_keys(&prefs, &mut seen)?;
    Ok(prefs)
}

fn collect_keys(prefs: &[SourcePref], seen: &mut BTreeSet<String>) -> Result<(), PrefDescriptorError> {
    for pref in prefs {
        if let Some(key) = pref.key() {
            if !seen.insert(key.to_owned()) {
                return Err(PrefDescriptorError::DuplicateKey(key.to_owned()));
            }
        }
        if let SourcePref::Category { children, .. } = pref {
            collect_keys(children, seen)?;
        }
    }
    Ok(())
}

/// The preference key namespace of a source: `source_<id>` (EC-PR-01).
pub fn preference_key(source_id: i64) -> String {
    format!("source_{source_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn switch(key: &str) -> SourcePref {
        SourcePref::Switch { key: key.into(), title: "T".into(), summary: None, default: false }
    }

    #[test]
    fn the_preference_key_is_the_source_id_with_the_prefix() {
        assert_eq!(preference_key(4_851_548_778_462_321_757), "source_4851548778462321757");
    }

    #[test]
    fn the_key_of_a_negative_and_a_large_id_is_the_decimal_text() {
        assert_eq!(preference_key(-1), "source_-1");
        assert_eq!(preference_key(1_234_567_890_123), "source_1234567890123");
    }

    #[test]
    fn a_category_never_names_a_key_and_a_stored_row_does() {
        let category = SourcePref::Category { title: "Quality".into(), children: vec![switch("hd")] };
        assert_eq!(category.key(), None);
        assert_eq!(switch("nsfw").key(), Some("nsfw"));
    }

    #[test]
    fn a_category_nests_children_and_its_keys_are_checked_too() {
        let nested = SourcePref::Category { title: "A".into(), children: vec![switch("a"), switch("b")] };
        assert!(check_descriptors(vec![nested.clone(), switch("c")]).is_ok());
        let clash = check_descriptors(vec![nested, switch("b")]).unwrap_err();
        assert_eq!(clash, PrefDescriptorError::DuplicateKey("b".into()));
    }

    #[test]
    fn a_duplicate_key_is_rejected_at_construction_and_the_message_names_it() {
        let err = check_descriptors(vec![switch("x"), switch("x")]).unwrap_err();
        assert_eq!(err.to_string(), "duplicate preference key: x");
    }

    #[test]
    fn an_empty_list_is_valid() {
        assert_eq!(check_descriptors(Vec::new()).unwrap(), Vec::<SourcePref>::new());
    }
}
