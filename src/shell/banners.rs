//! App-state banners above the content: indexing and downloaded-only, in that order (T-36a-018). Incognito has no banner
//! (user, 2026-10-14): the panel's Incognito switch shows it.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

/// How often the sources of the banner state are read.
pub const POLL_MS: u64 = 300;

/// The state the banners show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BannerState {
    pub indexing: bool,
    pub downloaded_only: bool,
}

/// One banner. Declaration order is the display order, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKind {
    Indexing,
    DownloadedOnly,
}

impl BannerKind {
    pub const ALL: [BannerKind; 2] = [BannerKind::Indexing, BannerKind::DownloadedOnly];

    /// The label text key (`download_notifier_cache_renewal`, `label_downloaded_only`).
    pub fn text(self) -> String {
        tr(match self {
            Self::Indexing => MsgKey::download_notifier_cache_renewal,
            Self::DownloadedOnly => MsgKey::label_downloaded_only,
        })
    }

    fn on(self, state: BannerState) -> bool {
        match self {
            Self::Indexing => state.indexing,
            Self::DownloadedOnly => state.downloaded_only,
        }
    }
}

/// One revealer per banner. `update` shows and hides them; the reveal is an animated slide, so the banner is visible
/// on the frame after the state changes.
pub struct Banners {
    pub widget: gtk::Box,
    revealers: Vec<(BannerKind, gtk::Revealer)>,
}

impl Banners {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let revealers = BannerKind::ALL
            .into_iter()
            .map(|kind| {
                let label = gtk::Label::builder().label(kind.text()).css_classes(["kb-banner"]).hexpand(true).build();
                let revealer = gtk::Revealer::builder().transition_type(gtk::RevealerTransitionType::SlideDown).child(&label).build();
                widget.append(&revealer);
                (kind, revealer)
            })
            .collect();
        Self { widget, revealers }
    }

    pub fn update(&self, state: BannerState) {
        for (kind, revealer) in &self.revealers {
            revealer.set_reveal_child(kind.on(state));
        }
    }
}

/// Reads `source` every [`POLL_MS`] and updates `banners` when the state changed, until the widget is gone. The
/// indexing flag comes from the download index (`checking_indicator_visible`), the other two from the preferences.
pub fn watch(banners: Rc<Banners>, source: impl Fn() -> BannerState + 'static) {
    let last: Cell<Option<BannerState>> = Cell::new(None);
    let weak = banners.widget.downgrade();
    let tick = move || {
        if weak.upgrade().is_none() {
            return gtk::glib::ControlFlow::Break;
        }
        let now = source();
        if last.replace(Some(now)) != Some(now) {
            banners.update(now);
        }
        gtk::glib::ControlFlow::Continue
    };
    tick();
    gtk::glib::timeout_add_local(Duration::from_millis(POLL_MS), tick);
}

impl Default for Banners {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_texts_are_the_two_catalogue_strings() {
        assert_eq!(BannerKind::Indexing.text(), tr(MsgKey::download_notifier_cache_renewal));
        assert_eq!(BannerKind::DownloadedOnly.text(), tr(MsgKey::label_downloaded_only));
    }
}
