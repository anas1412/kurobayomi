//! Tracker removal from an entry (T-38b-036, T-39b-046; E-TRK-12, D-5, D-11). The local unlink always runs. The remote
//! delete runs only for a deletable tracker whose checkbox is set, and the checkbox is unchecked by default. The order
//! follows the kind of entry, not the tracker: a manga unlinks locally first and deletes remotely after (T-38b-036),
//! an anime deletes remotely first and unlinks after (T-39b-046). Both errors are reported.

use kuro_track::model::Kind;

/// One step of the removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Local,
    Remote,
}

/// The checkbox shows only for a deletable tracker.
pub fn checkbox_shown(deletable: bool) -> bool {
    deletable
}

/// The checkbox starts unchecked.
pub const CHECKBOX_DEFAULT: bool = false;

/// The remote delete runs only when the tracker can delete and the checkbox is set (E-TRK-12).
pub fn remote_delete(deletable: bool, checkbox_on: bool) -> bool {
    deletable && checkbox_on
}

/// The steps in order. The local unlink is always one of them. A manga runs it first, an anime last.
pub fn steps(kind: Kind, remote: bool) -> Vec<Step> {
    match (kind, remote) {
        (_, false) => vec![Step::Local],
        (Kind::Manga, true) => vec![Step::Local, Step::Remote],
        (Kind::Anime, true) => vec![Step::Remote, Step::Local],
    }
}

/// The error texts to show, remote first, then local. Both are reported when both fail (D-11).
pub fn errors(remote: Option<String>, local: Option<String>) -> Vec<String> {
    remote.into_iter().chain(local).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_delete_needs_a_deletable_tracker_and_the_checkbox() {
        assert!(!CHECKBOX_DEFAULT, "unchecked by default");
        assert!(!checkbox_shown(false));
        assert!(remote_delete(true, true));
        assert!(!remote_delete(true, false));
        assert!(!remote_delete(false, true));
    }

    #[test]
    fn the_local_unlink_always_runs_and_the_order_follows_the_kind() {
        assert_eq!(steps(Kind::Manga, false), vec![Step::Local]);
        assert_eq!(steps(Kind::Anime, false), vec![Step::Local]);
        assert_eq!(steps(Kind::Manga, true), vec![Step::Local, Step::Remote], "manga: remote after the local unlink (T-38b-036)");
        assert_eq!(steps(Kind::Anime, true), vec![Step::Remote, Step::Local], "anime: remote before the local row goes (T-39b-046)");
    }

    #[test]
    fn both_errors_are_reported_remote_first() {
        assert_eq!(errors(Some("r".into()), Some("l".into())), vec!["r".to_owned(), "l".to_owned()]);
        assert_eq!(errors(None, Some("l".into())), vec!["l".to_owned()]);
        assert!(errors(None, None).is_empty());
    }
}
