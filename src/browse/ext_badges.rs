//! What an extension row says about running here (T-19a-066): the badges "Not runnable on Linux" and "Unsigned package",
//! the disabled install, and the count line of a store row. The anime extension host publishes the badge of each listed
//! package it cannot run; the shared extension list reads them by package name, so its row model does not change.

use std::cell::RefCell;
use std::collections::BTreeMap;

use kuro_extensions::anime::browse::RowBadges;
use kuro_i18n::{MsgKey, tr, tr_plural};

thread_local! {
    static BADGES: RefCell<BTreeMap<String, RowBadges>> = const { RefCell::new(BTreeMap::new()) };
    static STORE_COUNTS: RefCell<BTreeMap<String, usize>> = const { RefCell::new(BTreeMap::new()) };
}

/// Replaces the per-store counts of entries hidden as not runnable, keyed by store index URL.
pub fn publish_store_counts(counts: BTreeMap<String, usize>) {
    STORE_COUNTS.with(|c| *c.borrow_mut() = counts);
}

/// The count line of the store with this index URL, or `None` when it hides nothing. Read on the GTK thread.
pub fn store_note_for(index_url: &str) -> Option<String> {
    store_note(STORE_COUNTS.with(|c| c.borrow().get(index_url).copied().unwrap_or(0)))
}

/// Replaces the badges: one entry per package that carries any.
pub fn publish(badges: BTreeMap<String, RowBadges>) {
    BADGES.with(|b| *b.borrow_mut() = badges);
}

/// The badge of a row. A split row's key is `<pkg>-<source id>`, so the package is looked up too.
pub fn of(row_pkg: &str) -> Option<RowBadges> {
    BADGES.with(|b| {
        let badges = b.borrow();
        badges.get(row_pkg).or_else(|| {
            let (pkg, id) = row_pkg.rsplit_once('-')?;
            id.parse::<i64>().ok().and_then(|_| badges.get(pkg))
        }).copied()
    })
}

/// The badge texts of a row, in order.
pub fn labels(badges: RowBadges) -> Vec<String> {
    let mut out = Vec::new();
    if badges.not_runnable {
        out.push(tr(MsgKey::ext_not_runnable_on_linux));
    }
    if badges.unsigned {
        out.push(tr(MsgKey::ext_unsigned_package));
    }
    out
}

/// The line of a store row that counts the entries hidden as not runnable: "N entries not runnable on Linux". Nothing
/// for a store with none.
pub fn store_note(hidden: usize) -> Option<String> {
    (hidden > 0).then(|| tr_plural(MsgKey::ext_store_not_runnable_hidden, i64::try_from(hidden).unwrap_or(i64::MAX), &[]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_19a_066_the_badge_texts_and_the_store_note_read_as_the_plan_words() {
        assert_eq!(labels(RowBadges { not_runnable: true, unsigned: false }), ["Not runnable on Linux"]);
        assert_eq!(labels(RowBadges { not_runnable: false, unsigned: true }), ["Unsigned package"]);
        assert!(labels(RowBadges { not_runnable: false, unsigned: false }).is_empty());
        assert_eq!(store_note(0), None);
        assert_eq!(store_note(1).as_deref(), Some("1 entry not runnable on Linux"));
        assert_eq!(store_note(3).as_deref(), Some("3 entries not runnable on Linux"));
    }

    #[test]
    fn a_store_row_reads_its_own_count() {
        publish_store_counts(BTreeMap::from([("https://a/i.json".to_owned(), 2)]));
        assert_eq!(store_note_for("https://a/i.json").as_deref(), Some("2 entries not runnable on Linux"));
        assert_eq!(store_note_for("https://b/i.json"), None);
        publish_store_counts(BTreeMap::new());
        assert_eq!(store_note_for("https://a/i.json"), None);
    }

    #[test]
    fn a_split_row_key_finds_the_badge_of_its_package() {
        let badge = RowBadges { not_runnable: true, unsigned: false };
        publish(BTreeMap::from([("eu.apk".to_owned(), badge)]));
        assert_eq!(of("eu.apk"), Some(badge));
        assert_eq!(of("eu.apk-1234"), Some(badge));
        assert_eq!(of("eu.apk-other"), None);
        assert_eq!(of("eu.other-1234"), None);
        publish(BTreeMap::new());
        assert_eq!(of("eu.apk"), None);
    }
}
