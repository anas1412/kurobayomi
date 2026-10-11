//! The Filter tab of the library settings sheet (T-37a-037; B-12.1 and U-3.1). The rows, their order, the state each
//! one shows and what a tap writes are pure and tested here. `build` draws the rows and writes each tap at once.

use std::rc::Rc;

use gtk::Orientation as O;
use gtk::prelude::*;
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::tristate::TriState;

use crate::ui;

/// One filter a row stands for. `Unseen` is "Unseen" for anime and "Unread" for manga; `Tracked` holds the tracker id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Downloaded,
    Unseen,
    Completed,
    Tracked(i64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Label {
    Msg(MsgKey),
    /// A tracker's own name (one row per logged-in tracker when there are several).
    Tracker(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Heading(MsgKey),
    Tri { filter: Filter, label: Label },
}

/// Decision (T-37a-057): the interval-custom row is not drawn on the desktop build; upstream draws it only in debug
/// builds. The filter still runs when the restriction is on (`t2::filter`).
/// The rows top to bottom (B-12.1). `trackers` are the logged-in trackers as (id, name), in tracker order: none gives no
/// tracker row, one gives a single row, more give a heading and one row each.
pub fn items(kind: EntryKind, trackers: &[(i64, String)]) -> Vec<Item> {
    let unseen_key = match kind {
        EntryKind::Anime => MsgKey::action_filter_unseen,
        EntryKind::Manga => MsgKey::action_filter_unread,
    };
    let mut rows = vec![
        Item::Tri { filter: Filter::Downloaded, label: Label::Msg(MsgKey::label_downloaded) },
        Item::Tri { filter: Filter::Unseen, label: Label::Msg(unseen_key) },
        Item::Tri { filter: Filter::Completed, label: Label::Msg(MsgKey::completed) },
    ];
    match trackers {
        [] => {}
        [(id, _)] => rows.push(Item::Tri { filter: Filter::Tracked(*id), label: Label::Msg(MsgKey::action_filter_tracked) }),
        many => {
            rows.push(Item::Heading(MsgKey::action_filter_tracked));
            rows.extend(many.iter().map(|(id, name)| Item::Tri {
                filter: Filter::Tracked(*id),
                label: Label::Tracker(name.clone()),
            }));
        }
    }
    rows
}

/// The state a row shows and whether a tap may change it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub state: TriState,
    pub locked: bool,
}

/// Global downloaded-only forces the downloaded row to Is and locks it (B-12.1, U-3.1); every other row shows what is
/// stored.
pub fn shown(filter: Filter, stored: TriState, downloaded_only: bool) -> Shown {
    if filter == Filter::Downloaded && downloaded_only {
        Shown { state: TriState::ENABLED_IS, locked: true }
    } else {
        Shown { state: stored, locked: false }
    }
}

/// The state a tap writes: the cycle Off, Is, Not, Off (B-4.3). `None` when the row is locked.
pub fn tapped(shown: Shown) -> Option<TriState> {
    (!shown.locked).then(|| shown.state.next())
}

/// Word beside the checkbox. The checkbox alone is ambiguous for Not (see U-3.1).
pub fn state_word(state: TriState) -> &'static str {
    match state {
        TriState::DISABLED => "Off",
        TriState::ENABLED_IS => "Is",
        TriState::ENABLED_NOT => "Not",
    }
}

/// Runs `f` on the stored preference that `filter` of `kind` reads and writes.
fn with_pref<R>(prefs: &LibraryPrefs, kind: EntryKind, filter: Filter, f: impl FnOnce(&kuro_prefs::pref::Pref<TriState>) -> R) -> R {
    match (kind, filter) {
        (EntryKind::Anime, Filter::Downloaded) => f(&prefs.filter_downloaded_anime),
        (EntryKind::Manga, Filter::Downloaded) => f(&prefs.filter_downloaded_manga),
        (EntryKind::Anime, Filter::Unseen) => f(&prefs.filter_unseen),
        (EntryKind::Manga, Filter::Unseen) => f(&prefs.filter_unread),
        (EntryKind::Anime, Filter::Completed) => f(&prefs.filter_completed_anime),
        (EntryKind::Manga, Filter::Completed) => f(&prefs.filter_completed_manga),
        (EntryKind::Anime, Filter::Tracked(id)) => f(&prefs.filter_tracked_anime(id as i32)),
        (EntryKind::Manga, Filter::Tracked(id)) => f(&prefs.filter_tracked_manga(id as i32)),
    }
}

pub fn read(prefs: &LibraryPrefs, kind: EntryKind, filter: Filter) -> TriState {
    with_pref(prefs, kind, filter, |p| p.get())
}

pub fn write(prefs: &LibraryPrefs, kind: EntryKind, filter: Filter, state: TriState) -> Result<(), kuro_prefs::PrefError> {
    with_pref(prefs, kind, filter, |p| p.set(state))
}

/// The tab as a widget. `on_change` runs after each write, so the library reloads with the new filter.
pub fn build(
    kind: EntryKind,
    prefs: Rc<LibraryPrefs>,
    downloaded_only: bool,
    trackers: &[(i64, String)],
    on_change: Rc<dyn Fn()>,
) -> gtk::Widget {
    let col = ui::column();
    for item in items(kind, trackers) {
        match item {
            Item::Heading(key) => {
                let heading = gtk::Label::builder().label(tr(key)).xalign(0.0).css_classes(["heading"]).build();
                col.append(&heading);
            }
            Item::Tri { filter, label } => {
                let text = match label {
                    Label::Msg(key) => tr(key),
                    Label::Tracker(name) => name,
                };
                col.append(&tri_row(kind, prefs.clone(), downloaded_only, filter, &text, on_change.clone()));
            }
        }
    }
    ui::scrolled(&col).upcast()
}

/// Paints the checkbox (Is checked, Not inconsistent) and the state word from a row's shown state.
fn paint(check: &gtk::CheckButton, word: &gtk::Label, shown: Shown) {
    check.set_active(shown.state != TriState::DISABLED);
    check.set_inconsistent(shown.state == TriState::ENABLED_NOT);
    word.set_label(state_word(shown.state));
}

fn tri_row(
    kind: EntryKind,
    prefs: Rc<LibraryPrefs>,
    downloaded_only: bool,
    filter: Filter,
    text: &str,
    on_change: Rc<dyn Fn()>,
) -> gtk::Box {
    let row = gtk::Box::new(O::Horizontal, 12);
    let check = gtk::CheckButton::new();
    // The whole row takes the tap, so the checkbox never toggles on its own and cannot disagree with the cycle.
    check.set_can_target(false);
    check.set_focusable(false);
    let label = gtk::Label::builder().label(text).xalign(0.0).hexpand(true).build();
    let word = gtk::Label::builder().css_classes(["dim-label"]).build();
    row.append(&check);
    row.append(&label);
    row.append(&word);
    let initial = shown(filter, read(&prefs, kind, filter), downloaded_only);
    paint(&check, &word, initial);
    if initial.locked {
        row.set_sensitive(false);
        return row;
    }
    let click = gtk::GestureClick::new();
    let (check_c, word_c) = (check.clone(), word.clone());
    click.connect_released(move |_, _, _, _| {
        let current = shown(filter, read(&prefs, kind, filter), downloaded_only);
        let Some(next) = tapped(current) else { return };
        match write(&prefs, kind, filter, next) {
            Ok(()) => {
                paint(&check_c, &word_c, shown(filter, next, downloaded_only));
                on_change();
            }
            Err(e) => tracing::error!(error = %e, "saving a library filter failed"),
        }
    });
    row.add_controller(click);
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    fn prefs() -> LibraryPrefs {
        LibraryPrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn t_37a_057_the_interval_row_is_not_drawn_on_the_desktop() {
        for kind in [EntryKind::Anime, EntryKind::Manga] {
            for trackers in [vec![], vec![(1, "AniList".to_owned())], vec![(1, "A".to_owned()), (2, "B".to_owned())]] {
                let rows = items(kind, &trackers);
                assert!(
                    rows.iter().all(|row| !matches!(row, Item::Tri { label: Label::Msg(MsgKey::action_filter_interval_custom), .. })),
                    "{kind:?} with {} trackers",
                    trackers.len()
                );
            }
        }
    }

    #[test]
    fn the_rows_follow_b_12_1_for_anime_with_no_tracker() {
        let rows = items(EntryKind::Anime, &[]);
        assert_eq!(
            rows,
            vec![
                Item::Tri { filter: Filter::Downloaded, label: Label::Msg(MsgKey::label_downloaded) },
                Item::Tri { filter: Filter::Unseen, label: Label::Msg(MsgKey::action_filter_unseen) },
                Item::Tri { filter: Filter::Completed, label: Label::Msg(MsgKey::completed) },
            ]
        );
    }

    #[test]
    fn manga_calls_the_unseen_row_unread() {
        let rows = items(EntryKind::Manga, &[]);
        assert_eq!(rows[1], Item::Tri { filter: Filter::Unseen, label: Label::Msg(MsgKey::action_filter_unread) });
    }

    #[test]
    fn one_tracker_is_one_row_and_many_are_a_heading_then_one_row_each() {
        let one = items(EntryKind::Anime, &[(7, "MyAnimeList".into())]);
        assert_eq!(one.len(), 4);
        assert_eq!(one[3], Item::Tri { filter: Filter::Tracked(7), label: Label::Msg(MsgKey::action_filter_tracked) });

        let many = items(EntryKind::Manga, &[(1, "AniList".into()), (2, "Kitsu".into())]);
        assert_eq!(many.len(), 3 + 1 + 2);
        assert_eq!(many[3], Item::Heading(MsgKey::action_filter_tracked));
        assert_eq!(many[4], Item::Tri { filter: Filter::Tracked(1), label: Label::Tracker("AniList".into()) });
        assert_eq!(many[5], Item::Tri { filter: Filter::Tracked(2), label: Label::Tracker("Kitsu".into()) });
    }

    #[test]
    fn downloaded_only_locks_the_downloaded_row_on_is() {
        assert_eq!(
            shown(Filter::Downloaded, TriState::DISABLED, true),
            Shown { state: TriState::ENABLED_IS, locked: true }
        );
        assert_eq!(shown(Filter::Downloaded, TriState::ENABLED_NOT, false), Shown { state: TriState::ENABLED_NOT, locked: false });
        assert_eq!(shown(Filter::Completed, TriState::ENABLED_NOT, true), Shown { state: TriState::ENABLED_NOT, locked: false });
    }

    #[test]
    fn a_tap_cycles_off_is_not_off_and_a_locked_row_takes_no_tap() {
        let off = Shown { state: TriState::DISABLED, locked: false };
        let is = Shown { state: TriState::ENABLED_IS, locked: false };
        let not = Shown { state: TriState::ENABLED_NOT, locked: false };
        assert_eq!(tapped(off), Some(TriState::ENABLED_IS));
        assert_eq!(tapped(is), Some(TriState::ENABLED_NOT));
        assert_eq!(tapped(not), Some(TriState::DISABLED));
        assert_eq!(tapped(Shown { state: TriState::ENABLED_IS, locked: true }), None);
    }

    #[test]
    fn the_state_words_are_off_is_and_not() {
        assert_eq!(state_word(TriState::DISABLED), "Off");
        assert_eq!(state_word(TriState::ENABLED_IS), "Is");
        assert_eq!(state_word(TriState::ENABLED_NOT), "Not");
    }

    #[test]
    fn a_write_is_read_back_for_each_kind_and_filter() {
        let p = prefs();
        write(&p, EntryKind::Anime, Filter::Unseen, TriState::ENABLED_IS).expect("write");
        write(&p, EntryKind::Manga, Filter::Unseen, TriState::ENABLED_NOT).expect("write");
        write(&p, EntryKind::Anime, Filter::Tracked(7), TriState::ENABLED_NOT).expect("write");
        assert_eq!(read(&p, EntryKind::Anime, Filter::Unseen), TriState::ENABLED_IS);
        assert_eq!(read(&p, EntryKind::Manga, Filter::Unseen), TriState::ENABLED_NOT);
        assert_eq!(read(&p, EntryKind::Anime, Filter::Tracked(7)), TriState::ENABLED_NOT);
        assert_eq!(read(&p, EntryKind::Manga, Filter::Tracked(7)), TriState::DISABLED, "anime and manga keep separate keys");
        assert_eq!(read(&p, EntryKind::Manga, Filter::Downloaded), TriState::DISABLED);
    }
}
