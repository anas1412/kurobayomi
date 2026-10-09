//! Viewer settings (T-44a-004, T-44a-005, T-44a-018, B§1, D§3). The initial values come from the preference store,
//! never from Kotlin field initialisers. `diff` is the reaction table of B§1.2: a change-only reaction fires for a
//! value that differs from the one the viewer holds, and never for the first value it reads.

use std::sync::mpsc::Receiver;

use kuro_prefs::reader::{ReaderHideThreshold, ReaderPrefs, TappingInvertMode};
use kuro_prefs::store::Changed;

use super::nav::NavMode;

/// The settings a viewer reads, as plain values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewerSettings {
    pub long_tap: bool,
    pub page_transitions: bool,
    pub double_tap_anim_ms: u32,
    pub volume_keys: bool,
    pub volume_keys_inverted: bool,
    pub always_show_transition: bool,
    pub overlay_on_start: bool,
    pub nav_mode_pager: NavMode,
    pub nav_mode_webtoon: NavMode,
    pub tapping_pager: TappingInvertMode,
    pub tapping_webtoon: TappingInvertMode,
    /// Read live on each scroll (B§7.6 proposal), so it is not part of the reactions.
    pub hide_threshold: i32,
    pub navigate_pan: bool,
    pub landscape_zoom: bool,
    pub image_scale_type: i32,
    pub zoom_start: i32,
    pub reader_theme: i32,
    pub crop_pager: bool,
    pub crop_webtoon: bool,
    pub side_padding: i32,
    pub zoom_out_disabled: bool,
    pub double_tap_zoom: bool,
    pub dual_split_pager: bool,
    pub dual_invert_pager: bool,
    pub dual_rotate_pager: bool,
    pub dual_rotate_invert_pager: bool,
    pub dual_split_webtoon: bool,
    pub dual_invert_webtoon: bool,
    pub dual_rotate_webtoon: bool,
    pub dual_rotate_invert_webtoon: bool,
}

/// Which viewer's reactions to compute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Pager,
    Webtoon,
}

/// What a viewer does when a setting changes (B§1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    /// The navigation table or its inversion changed: re-apply the regions and the overlay.
    Navigation,
    /// Page images decode again in place, keeping the position (B§6.7, B§7.7, T-44a-007).
    RefreshImages,
    /// A dual-page split was switched off: remove the inserts (B§1.2 note).
    RemoveSplits,
    /// The theme changed on the webtoon, which restarts the screen upstream (restyled in place here).
    Restyle,
    ZoomOutDisabled,
    DoubleTapZoom,
}

impl ViewerSettings {
    /// Reads every value from the store. Unknown stored values fall back to their defaults (B§1.3).
    pub fn from_prefs(p: &ReaderPrefs) -> Self {
        Self {
            long_tap: p.read_with_long_tap.get(),
            page_transitions: p.page_transitions.get(),
            double_tap_anim_ms: p.double_tap_anim_speed.get().max(0) as u32,
            volume_keys: p.read_with_volume_keys.get(),
            volume_keys_inverted: p.read_with_volume_keys_inverted.get(),
            always_show_transition: p.always_show_chapter_transition.get(),
            overlay_on_start: p.show_navigation_overlay_on_start.get(),
            nav_mode_pager: NavMode::from_stored(p.navigation_mode_pager.get()),
            nav_mode_webtoon: NavMode::from_stored(p.navigation_mode_webtoon.get()),
            tapping_pager: p.pager_nav_inverted.get(),
            tapping_webtoon: p.webtoon_nav_inverted.get(),
            hide_threshold: p.reader_hide_threshold.get().threshold(),
            navigate_pan: p.navigate_to_pan.get(),
            landscape_zoom: p.landscape_zoom.get(),
            image_scale_type: p.image_scale_type.get(),
            zoom_start: p.zoom_start.get(),
            reader_theme: p.reader_theme.get(),
            crop_pager: p.crop_borders.get(),
            crop_webtoon: p.crop_borders_webtoon.get(),
            side_padding: p.webtoon_side_padding.get(),
            zoom_out_disabled: p.webtoon_disable_zoom_out.get(),
            double_tap_zoom: p.webtoon_double_tap_zoom_enabled.get(),
            dual_split_pager: p.dual_page_split_paged.get(),
            dual_invert_pager: p.dual_page_invert_paged.get(),
            dual_rotate_pager: p.dual_page_rotate_to_fit.get(),
            dual_rotate_invert_pager: p.dual_page_rotate_to_fit_invert.get(),
            dual_split_webtoon: p.dual_page_split_webtoon.get(),
            dual_invert_webtoon: p.dual_page_invert_webtoon.get(),
            dual_rotate_webtoon: p.dual_page_rotate_to_fit_webtoon.get(),
            dual_rotate_invert_webtoon: p.dual_page_rotate_to_fit_invert_webtoon.get(),
        }
    }
}

/// The reactions a change from `old` to `new` causes for `family`. Each reaction is listed once, in the order the
/// settings are declared. Values that did not change cause nothing.
pub fn diff(old: &ViewerSettings, new: &ViewerSettings, family: Family) -> Vec<Reaction> {
    let mut out = Vec::new();
    let mut push = |r: Reaction, changed: bool| {
        if changed && !out.contains(&r) {
            out.push(r);
        }
    };
    match family {
        Family::Pager => {
            push(Reaction::Navigation, old.nav_mode_pager != new.nav_mode_pager || old.tapping_pager != new.tapping_pager);
            push(
                Reaction::RefreshImages,
                old.image_scale_type != new.image_scale_type
                    || old.zoom_start != new.zoom_start
                    || old.reader_theme != new.reader_theme
                    || old.crop_pager != new.crop_pager
                    || old.landscape_zoom != new.landscape_zoom
                    || old.dual_invert_pager != new.dual_invert_pager
                    || old.dual_rotate_pager != new.dual_rotate_pager
                    || old.dual_rotate_invert_pager != new.dual_rotate_invert_pager
                    || old.dual_split_pager != new.dual_split_pager,
            );
            push(Reaction::RemoveSplits, old.dual_split_pager && !new.dual_split_pager);
        }
        Family::Webtoon => {
            push(Reaction::Navigation, old.nav_mode_webtoon != new.nav_mode_webtoon || old.tapping_webtoon != new.tapping_webtoon);
            push(
                Reaction::RefreshImages,
                old.crop_webtoon != new.crop_webtoon
                    || old.side_padding != new.side_padding
                    || old.dual_split_webtoon != new.dual_split_webtoon
                    || old.dual_invert_webtoon != new.dual_invert_webtoon
                    || old.dual_rotate_webtoon != new.dual_rotate_webtoon
                    || old.dual_rotate_invert_webtoon != new.dual_rotate_invert_webtoon,
            );
            push(Reaction::Restyle, old.reader_theme != new.reader_theme);
            push(Reaction::ZoomOutDisabled, old.zoom_out_disabled != new.zoom_out_disabled);
            push(Reaction::DoubleTapZoom, old.double_tap_zoom != new.double_tap_zoom);
        }
    }
    out
}

/// Follows the preference store for a live viewer (T-44a-005). Reads the store again only after it published a
/// change, and reports a result only when it differs from what the viewer holds (`distinctUntilChanged`). The first
/// read is the viewer's initial value and is never reported, so change-only reactions (`drop(1)`) do not fire on it.
pub struct SettingsWatch {
    prefs: ReaderPrefs,
    events: Receiver<Changed>,
    held: ViewerSettings,
}

impl SettingsWatch {
    /// Subscribes first and takes `held` as the initial value, so a write after this call is not missed.
    pub fn new(prefs: ReaderPrefs, held: ViewerSettings) -> Self {
        let events = prefs.store().subscribe();
        Self { prefs, events, held }
    }

    /// The new settings when the store changed them since the last call, otherwise `None`. Never blocks.
    pub fn poll(&mut self) -> Option<ViewerSettings> {
        let mut changed = false;
        while self.events.try_recv().is_ok() {
            changed = true;
        }
        if !changed {
            return None;
        }
        let new = ViewerSettings::from_prefs(&self.prefs);
        (new != self.held).then(|| {
            self.held = new.clone();
            new
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    #[test]
    fn the_watch_reports_a_changed_value_once_and_never_the_initial_one() {
        let p = prefs();
        let mut watch = SettingsWatch::new(p.clone(), ViewerSettings::from_prefs(&p));
        assert_eq!(watch.poll(), None, "the initial value is not a change");
        p.webtoon_disable_zoom_out.set(true).unwrap();
        let new = watch.poll().expect("the write is reported");
        assert!(new.zoom_out_disabled);
        assert_eq!(watch.poll(), None, "reported once");
    }

    #[test]
    fn a_write_of_another_key_or_the_same_value_reports_nothing() {
        let p = prefs();
        let mut watch = SettingsWatch::new(p.clone(), ViewerSettings::from_prefs(&p));
        p.page_transitions.set(true).unwrap();
        p.folder_per_manga.set(true).unwrap();
        assert_eq!(watch.poll(), None, "the viewer holds the same settings");
    }

    #[test]
    fn several_writes_between_polls_give_the_latest_values_in_one_report() {
        let p = prefs();
        let mut watch = SettingsWatch::new(p.clone(), ViewerSettings::from_prefs(&p));
        p.reader_theme.set(2).unwrap();
        p.reader_theme.set(0).unwrap();
        p.navigation_mode_webtoon.set(3).unwrap();
        let new = watch.poll().expect("reported");
        assert_eq!((new.reader_theme, new.nav_mode_webtoon), (0, NavMode::Edge));
    }

    fn prefs() -> ReaderPrefs {
        ReaderPrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn an_empty_store_gives_the_observed_defaults() {
        let s = ViewerSettings::from_prefs(&prefs());
        assert!(s.long_tap && s.page_transitions && s.always_show_transition && s.navigate_pan && s.landscape_zoom);
        assert!(!s.volume_keys && !s.volume_keys_inverted && !s.overlay_on_start);
        assert_eq!(s.double_tap_anim_ms, 500);
        assert_eq!(s.hide_threshold, 31);
        assert_eq!((s.nav_mode_pager, s.nav_mode_webtoon), (NavMode::Default, NavMode::Default));
        assert_eq!((s.tapping_pager, s.tapping_webtoon), (TappingInvertMode::NONE, TappingInvertMode::NONE));
        assert_eq!((s.image_scale_type, s.zoom_start, s.reader_theme), (1, 1, 1));
        assert_eq!(s.side_padding, 0);
    }

    #[test]
    fn stored_values_are_read_and_unknown_codes_fall_back() {
        let p = prefs();
        p.navigation_mode_pager.set(3).unwrap();
        p.navigation_mode_webtoon.set(77).unwrap();
        p.reader_hide_threshold.set(ReaderHideThreshold::HIGHEST).unwrap();
        let s = ViewerSettings::from_prefs(&p);
        assert_eq!(s.nav_mode_pager, NavMode::Edge);
        assert_eq!(s.nav_mode_webtoon, NavMode::Default);
        assert_eq!(s.hide_threshold, 5);
    }

    #[test]
    fn an_unchanged_value_causes_no_reaction_and_the_first_read_causes_none() {
        let s = ViewerSettings::from_prefs(&prefs());
        assert!(diff(&s, &s.clone(), Family::Pager).is_empty());
        assert!(diff(&s, &s.clone(), Family::Webtoon).is_empty());
    }

    #[test]
    fn navigation_changes_reapply_the_table_for_the_right_viewer_only() {
        let old = ViewerSettings::from_prefs(&prefs());
        let mut new = old.clone();
        new.nav_mode_webtoon = NavMode::Edge;
        assert!(diff(&old, &new, Family::Pager).is_empty());
        assert_eq!(diff(&old, &new, Family::Webtoon), [Reaction::Navigation]);
        let mut inverted = old.clone();
        inverted.tapping_pager = TappingInvertMode::BOTH;
        assert_eq!(diff(&old, &inverted, Family::Pager), [Reaction::Navigation]);
    }

    #[test]
    fn a_switched_off_split_removes_the_inserts_and_refreshes_once() {
        let mut old = ViewerSettings::from_prefs(&prefs());
        old.dual_split_pager = true;
        let mut new = old.clone();
        new.dual_split_pager = false;
        new.dual_invert_pager = true;
        assert_eq!(diff(&old, &new, Family::Pager), [Reaction::RefreshImages, Reaction::RemoveSplits]);
    }

    #[test]
    fn the_webtoon_theme_restyles_and_the_pager_theme_refreshes() {
        let old = ViewerSettings::from_prefs(&prefs());
        let mut new = old.clone();
        new.reader_theme = 2;
        assert_eq!(diff(&old, &new, Family::Webtoon), [Reaction::Restyle]);
        assert_eq!(diff(&old, &new, Family::Pager), [Reaction::RefreshImages]);
    }

    #[test]
    fn zoom_out_and_double_tap_zoom_have_their_own_reactions() {
        let old = ViewerSettings::from_prefs(&prefs());
        let mut new = old.clone();
        new.zoom_out_disabled = true;
        new.double_tap_zoom = false;
        assert_eq!(diff(&old, &new, Family::Webtoon), [Reaction::ZoomOutDisabled, Reaction::DoubleTapZoom]);
    }
}
