//! The badge of the "Updates" entry in the sidebar (T-26a-101): the new manga updates plus the new anime updates the
//! update passes counted, hidden at zero and when the "show updates count" switch is off. The Updates page resets the
//! counters when it opens; this only shows them.
//!
//! The label keeps its place when hidden (its opacity goes to zero), so the rows do not shift.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::library::LibraryPrefs;

use crate::bind;

/// The badge text for the two counters, or `None` when the badge is hidden: both counters at zero, or the switch off.
pub fn text(manga: i32, anime: i32, show: bool) -> Option<String> {
    let total = i64::from(manga.max(0)) + i64::from(anime.max(0));
    (show && total > 0).then(|| total.to_string())
}

fn paint(label: &gtk::Label, (manga, anime, show): (i32, i32, bool)) {
    let shown = text(manga, anime, show);
    label.set_label(shown.as_deref().unwrap_or(""));
    label.set_opacity(if shown.is_some() { 1.0 } else { 0.0 });
}

/// Shows the live count in `label`, now and on every change of the counters or of the switch.
pub fn bind(label: &gtk::Label, library: &LibraryPrefs) {
    let state = Rc::new(Cell::new((0_i32, 0_i32, true)));
    let (manga, anime, show) = (state.clone(), state.clone(), state);
    bind::bind(label, &library.new_manga_updates_count, move |l, count| {
        manga.set((count, manga.get().1, manga.get().2));
        paint(l, manga.get());
    });
    bind::bind(label, &library.new_anime_updates_count, move |l, count| {
        anime.set((anime.get().0, count, anime.get().2));
        paint(l, anime.get());
    });
    bind::bind(label, &library.new_show_updates_count, move |l, on| {
        show.set((show.get().0, show.get().1, on));
        paint(l, show.get());
    });
}

/// Shows the stored extension update count in `label`, hidden at zero (the Browse entry's badge).
pub fn bind_extension_updates(label: &gtk::Label, source: &kuro_prefs::source::SourcePrefs) {
    bind::bind(label, &source.manga_extension_updates_count, move |l, count| paint(l, (count, 0, true)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_badge_is_the_manga_count_plus_the_anime_count() {
        assert_eq!(text(3, 4, true).as_deref(), Some("7"));
        assert_eq!(text(5, 0, true).as_deref(), Some("5"));
        assert_eq!(text(0, 2, true).as_deref(), Some("2"));
    }

    #[test]
    fn the_badge_is_hidden_at_zero_and_when_the_switch_is_off() {
        assert_eq!(text(0, 0, true), None);
        assert_eq!(text(3, 4, false), None);
        assert_eq!(text(-2, 0, true), None, "a stray negative counter never shows");
    }

    #[test]
    fn the_placeholder_is_gone_from_the_sidebar() {
        // T-26a-101: no hard-coded count remains in the sidebar's badge table.
        let main = include_str!("../main.rs");
        assert!(!main.contains("\"updates\" => \"8\""));
        assert!(!main.contains("\"updates\" => \"0\"") && !main.contains("\"browse\" => \"1\""));
    }
}
