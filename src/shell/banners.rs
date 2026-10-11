//! App-state banners above the content: indexing, downloaded-only and incognito, in that order (T-36a-018).

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

/// How often the sources of the banner state are read.
pub const POLL_MS: u64 = 300;

/// The colour of each banner: its role and the role of its text (Material secondary, tertiary and primary).
pub fn css() -> String {
    BannerKind::ALL
        .into_iter()
        .map(|kind| {
            let (bg, fg) = match kind {
                BannerKind::Indexing => ("secondary", "on_secondary"),
                BannerKind::DownloadedOnly => ("tertiary", "on_tertiary"),
                BannerKind::Incognito => ("primary", "on_primary"),
            };
            format!(".{} {{ background-color: @kuro_{bg}; color: @kuro_{fg}; padding: 4px 16px; }}\n", kind.css_class())
        })
        .collect()
}

thread_local! {
    static CSS_LOADED: Cell<bool> = const { Cell::new(false) };
}

fn ensure_css() {
    if CSS_LOADED.with(|done| done.replace(true)) {
        return;
    }
    let Some(display) = gtk::gdk::Display::default() else { return };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css());
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

/// The state the banners show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BannerState {
    pub indexing: bool,
    pub downloaded_only: bool,
    pub incognito: bool,
}

/// One banner. Declaration order is the display order, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKind {
    Indexing,
    DownloadedOnly,
    Incognito,
}

impl BannerKind {
    pub const ALL: [BannerKind; 3] = [BannerKind::Indexing, BannerKind::DownloadedOnly, BannerKind::Incognito];

    /// The label text key (`download_notifier_cache_renewal`, `label_downloaded_only`, `pref_incognito_mode`).
    pub fn text(self) -> String {
        tr(match self {
            Self::Indexing => MsgKey::download_notifier_cache_renewal,
            Self::DownloadedOnly => MsgKey::label_downloaded_only,
            Self::Incognito => MsgKey::pref_incognito_mode,
        })
    }

    /// The colour role: secondary, tertiary, primary. Used as a CSS class on the banner.
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Indexing => "kuro-banner-secondary",
            Self::DownloadedOnly => "kuro-banner-tertiary",
            Self::Incognito => "kuro-banner-primary",
        }
    }

    fn on(self, state: BannerState) -> bool {
        match self {
            Self::Indexing => state.indexing,
            Self::DownloadedOnly => state.downloaded_only,
            Self::Incognito => state.incognito,
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
        ensure_css();
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let revealers = BannerKind::ALL
            .into_iter()
            .map(|kind| {
                let label = gtk::Label::builder().label(kind.text()).css_classes([kind.css_class()]).hexpand(true).build();
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
    fn css_colours_each_banner_by_its_role_pair() {
        let css = css();
        assert!(css.contains(".kuro-banner-secondary { background-color: @kuro_secondary; color: @kuro_on_secondary; padding: 4px 16px; }"));
        assert!(css.contains(".kuro-banner-tertiary { background-color: @kuro_tertiary; color: @kuro_on_tertiary; padding: 4px 16px; }"));
        assert!(css.contains(".kuro-banner-primary { background-color: @kuro_primary; color: @kuro_on_primary; padding: 4px 16px; }"));
    }

    #[test]
    fn banner_texts_are_the_three_catalogue_strings() {
        assert_eq!(BannerKind::Indexing.text(), tr(MsgKey::download_notifier_cache_renewal));
        assert_eq!(BannerKind::DownloadedOnly.text(), tr(MsgKey::label_downloaded_only));
        assert_eq!(BannerKind::Incognito.text(), tr(MsgKey::pref_incognito_mode));
    }

    #[test]
    fn each_banner_has_its_colour_role() {
        assert_eq!(BannerKind::Indexing.css_class(), "kuro-banner-secondary");
        assert_eq!(BannerKind::DownloadedOnly.css_class(), "kuro-banner-tertiary");
        assert_eq!(BannerKind::Incognito.css_class(), "kuro-banner-primary");
    }
}
