//! Player state store (area 45 p2 T-45b-002, R§5). Every mutation goes through `Store::update`,
//! which mirrors the Kotlin `update { }` of the view model. After the change, each subscriber gets
//! one `Change` per field group that actually changed. Subscribers receive `&PlayerState` only, so a
//! subscriber cannot mutate the store while it is being notified; the queueing of the Kotlin design
//! (`glib::idle_add_local_once`) is therefore not needed here. Pure: no GTK.

use crate::state::PlayerState;

/// A field group of `PlayerState` that changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Change {
    /// `pos` or `duration`.
    Position,
    /// `paused`.
    Paused,
    /// `is_loading`, `is_loading_episode`, `is_loading_hosters`, `is_loading_tracks`.
    Loading,
    /// `playback_speed`.
    Speed,
    /// `controls_shown`.
    Controls,
    /// `seek_bar_shown`.
    SeekBar,
    /// `controls_locked`.
    Locked,
    /// `sheet` or `dismiss_sheet`.
    Sheet,
    /// `panel`.
    Panel,
    /// `dialog`.
    Dialog,
    /// `pip_active`.
    Pip,
    /// `keep_awake`.
    KeepAwake,
    /// `restore_volume_on_resume`.
    Volume,
    /// `primary_button` or `primary_button_title`.
    PrimaryButton,
    /// `is_exiting`.
    Exiting,
}

/// The change groups between two states, in declaration order, each at most once.
pub fn changes(before: &PlayerState, after: &PlayerState) -> Vec<Change> {
    let checks = [
        (
            Change::Position,
            before.pos != after.pos || before.duration != after.duration,
        ),
        (Change::Paused, before.paused != after.paused),
        (
            Change::Loading,
            before.is_loading != after.is_loading
                || before.is_loading_episode != after.is_loading_episode
                || before.is_loading_hosters != after.is_loading_hosters
                || before.is_loading_tracks != after.is_loading_tracks,
        ),
        (Change::Speed, before.playback_speed != after.playback_speed),
        (
            Change::Controls,
            before.controls_shown != after.controls_shown,
        ),
        (
            Change::SeekBar,
            before.seek_bar_shown != after.seek_bar_shown,
        ),
        (
            Change::Locked,
            before.controls_locked != after.controls_locked,
        ),
        (
            Change::Sheet,
            before.sheet != after.sheet || before.dismiss_sheet != after.dismiss_sheet,
        ),
        (Change::Panel, before.panel != after.panel),
        (Change::Dialog, before.dialog != after.dialog),
        (Change::Pip, before.pip_active != after.pip_active),
        (Change::KeepAwake, before.keep_awake != after.keep_awake),
        (
            Change::Volume,
            before.restore_volume_on_resume != after.restore_volume_on_resume,
        ),
        (
            Change::PrimaryButton,
            before.primary_button != after.primary_button
                || before.primary_button_title != after.primary_button_title,
        ),
        (Change::Exiting, before.is_exiting != after.is_exiting),
    ];
    checks
        .into_iter()
        .filter_map(|(change, changed)| changed.then_some(change))
        .collect()
}

type Subscriber = Box<dyn FnMut(Change, &PlayerState)>;

/// The player state with its subscribers.
pub struct Store {
    state: PlayerState,
    subscribers: Vec<Subscriber>,
}

impl Store {
    pub fn new(state: PlayerState) -> Self {
        Self {
            state,
            subscribers: Vec::new(),
        }
    }

    pub fn state(&self) -> &PlayerState {
        &self.state
    }

    /// Adds a subscriber. It is called once per changed field group, after each update.
    pub fn subscribe(&mut self, subscriber: impl FnMut(Change, &PlayerState) + 'static) {
        self.subscribers.push(Box::new(subscriber));
    }

    /// Applies `mutate` to the state, then notifies the subscribers of each group that changed.
    /// Returns the value `mutate` returned.
    pub fn update<R>(&mut self, mutate: impl FnOnce(&mut PlayerState) -> R) -> R {
        let before = self.state.clone();
        let out = mutate(&mut self.state);
        for change in changes(&before, &self.state) {
            for subscriber in &mut self.subscribers {
                subscriber(change, &self.state);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::enums::{Panels, Sheets};

    fn recording(store: &mut Store) -> Rc<RefCell<Vec<Change>>> {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        store.subscribe(move |change, _| sink.borrow_mut().push(change));
        seen
    }

    #[test]
    fn setting_a_sheet_clears_the_panel_and_the_dialog() {
        let mut store = Store::new(PlayerState::default());
        store.update(|s| s.show_panel(Panels::AudioDelay));
        store.update(|s| s.show_sheet(Sheets::More));
        let state = store.state();
        assert_eq!(state.sheet, Sheets::More);
        assert_eq!(state.panel, Panels::None);
        assert!(!state.controls_shown);
    }

    #[test]
    fn a_panel_clears_the_sheet_and_a_dialog_clears_both() {
        let mut store = Store::new(PlayerState::default());
        store.update(|s| s.show_sheet(Sheets::Chapters));
        store.update(|s| s.show_panel(Panels::SubtitleDelay));
        assert_eq!(store.state().sheet, Sheets::None);
        store.update(|s| s.show_dialog(crate::enums::Dialogs::EpisodeList));
        assert_eq!(store.state().panel, Panels::None);
        assert_eq!(store.state().sheet, Sheets::None);
    }

    #[test]
    fn at_most_one_overlay_is_open_after_any_sequence() {
        let mut store = Store::new(PlayerState::default());
        store.update(|s| s.show_dialog(crate::enums::Dialogs::EpisodeList));
        store.update(|s| s.show_sheet(Sheets::AudioTracks));
        let s = store.state();
        let open = [
            s.sheet != Sheets::None,
            s.panel != Panels::None,
            s.dialog != crate::enums::Dialogs::None,
        ];
        assert_eq!(open.iter().filter(|o| **o).count(), 1);
    }

    #[test]
    fn controls_do_not_show_while_an_overlay_is_open() {
        let mut store = Store::new(PlayerState::default());
        store.update(|s| s.show_sheet(Sheets::More));
        assert!(!store.update(|s| s.show_controls()));
        assert!(!store.state().controls_shown);
        store.update(|s| s.show_sheet(Sheets::None));
        assert!(store.state().controls_shown);
    }

    #[test]
    fn hide_controls_preference_starts_the_chrome_hidden() {
        let state = PlayerState::with_hide_controls(true);
        assert!(!state.controls_shown);
        assert!(!state.seek_bar_shown);
        assert!(PlayerState::with_hide_controls(false).controls_shown);
    }

    #[test]
    fn subscribers_get_one_change_per_changed_group() {
        let mut store = Store::new(PlayerState::default());
        let seen = recording(&mut store);
        store.update(|s| {
            s.pos = 10.0;
            s.duration = 90.0;
            s.paused = Some(true);
            s.is_loading = true;
            s.is_loading_tracks = true;
        });
        assert_eq!(
            *seen.borrow(),
            vec![Change::Position, Change::Paused, Change::Loading]
        );
    }

    #[test]
    fn an_update_that_changes_nothing_notifies_nobody() {
        let mut store = Store::new(PlayerState::default());
        let seen = recording(&mut store);
        store.update(|s| s.playback_speed = 1.0);
        assert!(seen.borrow().is_empty());
    }

    #[test]
    fn opening_a_sheet_notifies_sheet_panel_and_controls_once_each() {
        let mut store = Store::new(PlayerState::default());
        let seen = recording(&mut store);
        store.update(|s| s.show_sheet(Sheets::PlaybackSpeed));
        assert_eq!(*seen.borrow(), vec![Change::Controls, Change::Sheet]);
    }

    #[test]
    fn update_returns_the_closure_value() {
        let mut store = Store::new(PlayerState::default());
        assert!(store.update(|s| s.show_seek_bar()));
        assert!(store.state().seek_bar_shown);
    }
}
