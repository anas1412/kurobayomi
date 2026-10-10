//! Backup flag tags (`PreferenceValues.kt:3-9`, T-09a-053). The tags look like bit masks but are opaque strings: they
//! are never OR-ed together (Q-BK-1). Unknown tags are dropped when the stored set is read, so writing drops them too
//! (E-BK-03).
//!
//! Decision (Q-BK-2): `backupFlags` has no caller outside the domain module in Aniyomi. The key stays for backup-format
//! parity, and area 21 decides whether the Linux UI shows it.

use std::collections::BTreeSet;

use kuro_prefs::values::{Decoded, PrefType, PrefValue};

/// Stored tags of the backup flags, for import tolerance and the id-remapping code.
pub const FLAG_CATEGORIES: &str = "1";
pub const FLAG_CHAPTERS: &str = "2";
pub const FLAG_HISTORY: &str = "4";
pub const FLAG_TRACK: &str = "8";
pub const FLAG_SETTINGS: &str = "10";
pub const FLAG_EXT_SETTINGS: &str = "20";
pub const FLAG_EXTENSIONS: &str = "40";

/// One backup flag. `tag()` is the stored string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackupFlag {
    Categories,
    Chapters,
    History,
    Track,
    Settings,
    ExtSettings,
    Extensions,
}

impl BackupFlag {
    pub const ALL: [BackupFlag; 7] = [
        BackupFlag::Categories,
        BackupFlag::Chapters,
        BackupFlag::History,
        BackupFlag::Track,
        BackupFlag::Settings,
        BackupFlag::ExtSettings,
        BackupFlag::Extensions,
    ];

    /// The stored string of this flag.
    pub fn tag(self) -> &'static str {
        match self {
            BackupFlag::Categories => FLAG_CATEGORIES,
            BackupFlag::Chapters => FLAG_CHAPTERS,
            BackupFlag::History => FLAG_HISTORY,
            BackupFlag::Track => FLAG_TRACK,
            BackupFlag::Settings => FLAG_SETTINGS,
            BackupFlag::ExtSettings => FLAG_EXT_SETTINGS,
            BackupFlag::Extensions => FLAG_EXTENSIONS,
        }
    }

    /// The flag with this exact tag, or `None` for any other string (`"3"` is not a flag).
    pub fn from_tag(tag: &str) -> Option<Self> {
        BackupFlag::ALL.into_iter().find(|flag| flag.tag() == tag)
    }
}

/// The set of backup flags stored under `backup_flags`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupFlags(pub BTreeSet<BackupFlag>);

impl Default for BackupFlags {
    /// `{"1","2","4","8"}`: categories, chapters, history and track.
    fn default() -> Self {
        BackupFlags(BTreeSet::from([
            BackupFlag::Categories,
            BackupFlag::Chapters,
            BackupFlag::History,
            BackupFlag::Track,
        ]))
    }
}

impl PrefType for BackupFlags {
    fn encode(&self) -> PrefValue {
        PrefValue::Set(self.0.iter().map(|flag| flag.tag().to_owned()).collect())
    }

    fn decode(v: &PrefValue) -> Decoded<Self> {
        match v {
            PrefValue::Set(tags) => Decoded::Ok(BackupFlags(
                tags.iter().filter_map(|tag| BackupFlag::from_tag(tag)).collect(),
            )),
            _ => Decoded::WrongType,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_the_seven_upstream_strings() {
        let tags: Vec<&str> = BackupFlag::ALL.iter().map(|f| f.tag()).collect();
        assert_eq!(tags, ["1", "2", "4", "8", "10", "20", "40"]);
    }

    #[test]
    fn from_tag_is_exact_and_never_combines_masks() {
        assert_eq!(BackupFlag::from_tag("10"), Some(BackupFlag::Settings));
        assert_eq!(BackupFlag::from_tag("3"), None, "1 | 2 is not a flag");
        assert_eq!(BackupFlag::from_tag("12"), None);
        assert_eq!(BackupFlag::from_tag(""), None);
    }

    #[test]
    fn the_default_set_is_categories_chapters_history_and_track() {
        let stored = BackupFlags::default().encode();
        assert_eq!(
            BackupFlags::default().0,
            BTreeSet::from([BackupFlag::Categories, BackupFlag::Chapters, BackupFlag::History, BackupFlag::Track])
        );
        assert_eq!(stored, PrefValue::Set(BTreeSet::from(["1".into(), "2".into(), "4".into(), "8".into()])));
    }

    #[test]
    fn unknown_tags_are_dropped_on_read_and_so_on_write() {
        let stored = PrefValue::Set(BTreeSet::from(["1".into(), "80".into(), "x".into()]));
        let Decoded::Ok(flags) = BackupFlags::decode(&stored) else {
            panic!("a set decodes");
        };
        assert_eq!(flags.0, BTreeSet::from([BackupFlag::Categories]));
        assert_eq!(flags.encode(), PrefValue::Set(BTreeSet::from(["1".into()])));
    }

    #[test]
    fn a_non_set_value_is_the_wrong_type() {
        assert!(matches!(BackupFlags::decode(&PrefValue::Int(3)), Decoded::WrongType));
    }
}
