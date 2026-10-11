//! The Mini player group of the player settings (T-48a-017). It is shown only when the display can run a mini player.
//! The other groups of the old Internal player page are cut by the settings diet.
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::Preference;
use crate::settings_ui::rows::{group, switch};

/// What the display lets the mini player do (T-48a-017).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiniPlayerSupport {
    /// An X11 display: the small window can be forced on top (mpv `ontop`).
    AlwaysOnTop,
    /// A Wayland compositor: the small window is shown but cannot be forced on top (no protocol for it).
    Windowed,
}

/// The support a display of GDK type `display_type` gives. `None` for any other (or no) display: the group is hidden.
pub fn mini_player_support(display_type: Option<&str>) -> Option<MiniPlayerSupport> {
    match display_type? {
        "GdkX11Display" => Some(MiniPlayerSupport::AlwaysOnTop),
        "GdkWaylandDisplay" => Some(MiniPlayerSupport::Windowed),
        _ => None,
    }
}

/// The probe on the running display. Before GTK is up there is no display, so the answer is `None`.
pub fn probe_mini_player() -> Option<MiniPlayerSupport> {
    if !gtk::is_initialized_main_thread() {
        return None;
    }
    let display = gtk::gdk::Display::default()?;
    mini_player_support(Some(gtk::glib::prelude::ObjectExt::type_(&display).name()))
}

/// The Mini player group (the PiP prefs under their desktop label).
pub fn mini_player_group(prefs: &Prefs) -> Preference {
    let p = &prefs.player;
    group(
        "Mini player",
        vec![
            switch("Enable mini player", &p.enable_pip, true),
            switch("Open mini player on exit", &p.pip_on_exit, true),
            switch("Show mini player episode change toasts", &p.pip_episode_toasts, true),
            switch("Switch next to previous in mini player", &p.pip_replace_with_previous, true),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mini_player_probe_follows_the_display_type() {
        assert_eq!(mini_player_support(Some("GdkX11Display")), Some(MiniPlayerSupport::AlwaysOnTop));
        assert_eq!(mini_player_support(Some("GdkWaylandDisplay")), Some(MiniPlayerSupport::Windowed));
        assert_eq!(mini_player_support(Some("GdkBroadwayDisplay")), None);
        assert_eq!(mini_player_support(None), None);
    }

    #[test]
    fn the_mini_player_group_has_its_four_switches() {
        let prefs = Prefs::in_memory();
        let Preference::Group(g) = mini_player_group(&prefs) else { panic!("a group") };
        assert_eq!(g.title, "Mini player");
        assert_eq!(g.items.len(), 4);
    }
}
