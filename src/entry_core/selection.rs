//! Chapter selection engine and the keyboard / pointer gesture map.
//! Upstream: `MangaScreenModel.kt:971-1057` (toggle, select all, invert). Plan: `p2-behavior.md` 6.8.
//!
//! Selection is keyed by chapter id over the raw list. Anchors are chapter ids, resolved against the
//! current view on every gesture, so a filter change does not move them to another row.

use std::collections::BTreeSet;

/// How a toggle was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// A program changed the selection (no anchors move).
    Programmatic,
    /// A plain tap, a Ctrl-click or Space.
    Tap,
    /// A long press, a right click, a Shift-click or Shift+Space. Selecting it extends a range.
    LongPress,
}

/// The selection set and the two range anchors (first and last selected row, as chapter ids).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    ids: BTreeSet<i64>,
    anchors: [Option<i64>; 2],
}

impl Selection {
    pub fn contains(&self, id: i64) -> bool {
        self.ids.contains(&id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Selected chapter ids in ascending order.
    pub fn ids(&self) -> impl Iterator<Item = i64> + '_ {
        self.ids.iter().copied()
    }

    /// Drops selected ids that no longer exist (a refresh removed the chapter). Keeps the rest.
    pub fn retain_existing(&mut self, existing: &[i64]) {
        self.ids.retain(|id| existing.contains(id));
        self.anchors = [
            self.anchors[0].filter(|a| existing.contains(a)),
            self.anchors[1].filter(|a| existing.contains(a)),
        ];
    }

    /// Sets every raw chapter id to `on`, and resets the anchors. Acts on the raw list, not the view (E-SEL-4).
    pub fn set_all(&mut self, raw_ids: &[i64], on: bool) {
        for &id in raw_ids {
            if on {
                self.ids.insert(id);
            } else {
                self.ids.remove(&id);
            }
        }
        self.anchors = [None, None];
    }

    /// Flips every raw chapter id, and resets the anchors.
    pub fn invert(&mut self, raw_ids: &[i64]) {
        for &id in raw_ids {
            if !self.ids.remove(&id) {
                self.ids.insert(id);
            }
        }
        self.anchors = [None, None];
    }

    /// Clears the selection and leaves selection mode. Mark, bookmark and download actions call this (E-SEL-5).
    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchors = [None, None];
    }

    /// Toggles `id` in the view `view` (chapter ids in view order). Returns true when the selection changed.
    /// `selected` is the requested state. A request that matches the current state changes nothing.
    pub fn toggle(&mut self, view: &[i64], id: i64, selected: bool, pick: Pick) -> bool {
        let Some(idx) = view.iter().position(|&v| v == id).map(|i| i as i64) else {
            return false;
        };
        if self.ids.contains(&id) == selected {
            return false;
        }
        let first_selection = !view.iter().any(|v| self.ids.contains(v));
        if selected {
            self.ids.insert(id);
        } else {
            self.ids.remove(&id);
        }

        let mut p0 = self.anchor_index(view, 0);
        let mut p1 = self.anchor_index(view, 1);
        match pick {
            Pick::LongPress if selected => {
                if first_selection {
                    p0 = idx;
                    p1 = idx;
                } else {
                    let range = if idx < p0 {
                        let r = (idx + 1)..p0;
                        p0 = idx;
                        r
                    } else if idx > p1 {
                        let r = (p1 + 1)..idx;
                        p1 = idx;
                        r
                    } else {
                        0..0
                    };
                    for i in range {
                        self.ids.insert(view[i as usize]);
                    }
                }
            }
            Pick::Tap => {
                if !selected {
                    if idx == p0 {
                        p0 = self.first_selected_index(view);
                    } else if idx == p1 {
                        p1 = self.last_selected_index(view);
                    }
                } else if p0 < 0 && p1 < 0 {
                    // ponytail: upstream sets only the second anchor when none exists (E-SEL-3). Both are set here.
                    p0 = idx;
                    p1 = idx;
                } else if idx < p0 {
                    p0 = idx;
                } else if idx > p1 {
                    p1 = idx;
                }
            }
            _ => {}
        }
        self.anchors = [self.id_at(view, p0), self.id_at(view, p1)];
        true
    }

    fn anchor_index(&self, view: &[i64], slot: usize) -> i64 {
        self.anchors[slot]
            .and_then(|a| view.iter().position(|&v| v == a))
            .map_or(-1, |i| i as i64)
    }

    fn id_at(&self, view: &[i64], index: i64) -> Option<i64> {
        usize::try_from(index)
            .ok()
            .and_then(|i| view.get(i))
            .copied()
    }

    fn first_selected_index(&self, view: &[i64]) -> i64 {
        view.iter()
            .position(|v| self.ids.contains(v))
            .map_or(-1, |i| i as i64)
    }

    fn last_selected_index(&self, view: &[i64]) -> i64 {
        view.iter()
            .rposition(|v| self.ids.contains(v))
            .map_or(-1, |i| i as i64)
    }
}

/// A user command from the keyboard or pointer, before it is applied to the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Tap,
    LongPress,
    SelectAll,
    Invert,
    ClearSelection,
    CloseDialog,
}

/// Back and Escape clear a selection first and pop the page only when nothing is selected. A selection hidden by a
/// filter still counts (E-04), so the check is over the whole selection, not the visible rows.
pub fn back_clears_selection(selected_total: usize) -> bool {
    selected_total > 0
}

/// Modifier keys held during a key press or click.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

/// The keys the details page reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Space,
    Escape,
    /// A lowercase letter, with Ctrl or Shift applied through `Mods`.
    Letter(char),
}

/// Keyboard map (`p2-ui-parity.md` 6). `in_selection` is whether selection mode is on; `dialog_open` is whether
/// a dialog is showing (Escape closes the dialog first).
pub fn key_command(key: Key, mods: Mods, in_selection: bool, dialog_open: bool) -> Option<Command> {
    match key {
        Key::Space if mods.shift => Some(Command::LongPress),
        Key::Space => Some(Command::Tap),
        Key::Escape if dialog_open => Some(Command::CloseDialog),
        Key::Escape if in_selection => Some(Command::ClearSelection),
        Key::Escape => None,
        Key::Letter('a') if mods.ctrl => Some(Command::SelectAll),
        Key::Letter('i') if mods.ctrl => Some(Command::Invert),
        Key::Letter(_) => None,
    }
}

/// Pointer map. A long press or right click selects and starts a range. Shift-click extends a range,
/// Ctrl-click toggles. A plain click returns `Tap`, and the caller opens the chapter when not in selection mode.
pub fn click_command(mods: Mods, long_press_or_right: bool) -> Command {
    if long_press_or_right || mods.shift {
        Command::LongPress
    } else {
        Command::Tap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Vec<i64> {
        vec![10, 11, 12, 13, 14]
    }

    #[test]
    fn tap_toggle_on_off() {
        let v = view();
        let mut s = Selection::default();
        assert!(s.toggle(&v, 12, true, Pick::Tap));
        assert!(s.contains(12));
        assert!(
            !s.toggle(&v, 12, true, Pick::Tap),
            "same state is no change"
        );
        assert!(s.toggle(&v, 12, false, Pick::Tap));
        assert!(s.is_empty());
    }

    #[test]
    fn long_press_range_forward() {
        let v = view();
        let mut s = Selection::default();
        s.toggle(&v, 11, true, Pick::LongPress);
        s.toggle(&v, 13, true, Pick::LongPress);
        assert_eq!(s.ids().collect::<Vec<_>>(), vec![11, 12, 13]);
    }

    #[test]
    fn long_press_range_backward() {
        let v = view();
        let mut s = Selection::default();
        s.toggle(&v, 13, true, Pick::LongPress);
        s.toggle(&v, 11, true, Pick::LongPress);
        assert_eq!(s.ids().collect::<Vec<_>>(), vec![11, 12, 13]);
    }

    #[test]
    fn long_press_inside_range_only_self() {
        let v = view();
        let mut s = Selection::default();
        s.toggle(&v, 11, true, Pick::LongPress);
        s.toggle(&v, 13, true, Pick::LongPress);
        s.toggle(&v, 12, false, Pick::Tap);
        // 12 is deselected. A long press on it inside the range selects only 12.
        s.toggle(&v, 12, true, Pick::LongPress);
        assert_eq!(s.ids().collect::<Vec<_>>(), vec![11, 12, 13]);
        let mut t = Selection::default();
        t.toggle(&v, 11, true, Pick::LongPress);
        t.toggle(&v, 14, true, Pick::LongPress);
        t.toggle(&v, 12, false, Pick::LongPress);
        t.toggle(&v, 12, true, Pick::LongPress);
        assert!(t.contains(12));
    }

    #[test]
    fn set_all_uses_raw_ids() {
        // The view shows only ids 1 and 3, but the raw list has 2 and 4 as well.
        let raw = [1, 2, 3, 4];
        let mut s = Selection::default();
        s.set_all(&raw, true);
        assert!(
            s.contains(2) && s.contains(4),
            "hidden rows are selected too"
        );
        assert_eq!(s.len(), 4);
    }

    #[test]
    fn invert_flips_raw() {
        let raw = [1, 2, 3];
        let mut s = Selection::default();
        s.toggle(&[1, 2, 3], 2, true, Pick::Tap);
        s.invert(&raw);
        assert_eq!(s.ids().collect::<Vec<_>>(), vec![1, 3]);
    }

    #[test]
    fn toggle_missing_id_is_noop() {
        let v = view();
        let mut s = Selection::default();
        assert!(!s.toggle(&v, 99, true, Pick::Tap));
        assert!(s.is_empty());
    }

    #[test]
    fn anchors_survive_filter_change() {
        // Anchors are chapter ids. After a filter hides row 11, the range is measured from 12 in the new view.
        let full = view();
        let mut s = Selection::default();
        s.toggle(&full, 12, true, Pick::LongPress);
        let filtered = vec![10, 12, 13, 14];
        assert!(s.toggle(&filtered, 14, true, Pick::LongPress));
        assert_eq!(s.ids().collect::<Vec<_>>(), vec![12, 13, 14]);
    }

    #[test]
    fn clear_and_key_map() {
        let v = view();
        let mut s = Selection::default();
        s.toggle(&v, 10, true, Pick::Tap);
        s.clear();
        assert!(s.is_empty());
        assert_eq!(
            key_command(
                Key::Space,
                Mods {
                    shift: true,
                    ctrl: false
                },
                true,
                false
            ),
            Some(Command::LongPress)
        );
        assert_eq!(
            key_command(Key::Space, Mods::default(), true, false),
            Some(Command::Tap)
        );
        assert_eq!(
            key_command(Key::Escape, Mods::default(), true, true),
            Some(Command::CloseDialog)
        );
        assert_eq!(
            key_command(Key::Escape, Mods::default(), true, false),
            Some(Command::ClearSelection)
        );
        assert_eq!(
            key_command(Key::Escape, Mods::default(), false, false),
            None
        );
        assert_eq!(
            key_command(
                Key::Letter('a'),
                Mods {
                    shift: false,
                    ctrl: true
                },
                false,
                false
            ),
            Some(Command::SelectAll)
        );
        assert_eq!(
            key_command(
                Key::Letter('i'),
                Mods {
                    shift: false,
                    ctrl: true
                },
                false,
                false
            ),
            Some(Command::Invert)
        );
        assert_eq!(
            click_command(
                Mods {
                    shift: true,
                    ctrl: false
                },
                false
            ),
            Command::LongPress
        );
        assert_eq!(
            click_command(
                Mods {
                    shift: false,
                    ctrl: true
                },
                false
            ),
            Command::Tap
        );
        assert_eq!(click_command(Mods::default(), true), Command::LongPress);
    }

    #[test]
    fn retain_existing_drops_removed_chapters() {
        let mut s = Selection::default();
        s.set_all(&[1, 2, 3], true);
        s.retain_existing(&[1, 3]);
        assert_eq!(s.ids().collect::<Vec<_>>(), vec![1, 3]);
    }
}
