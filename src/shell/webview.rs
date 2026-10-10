//! WebView screen chrome (Kotlin `WebViewScreenContent`, T-36a-049): the header with close, title and URL, Back and Forward,
//! the overflow menu in its fixed order, and the load progress bar.
//!
//! The rules are pure. The page engine (WebKitGTK, T-36a-052) reports `can_go_back`, `can_go_forward`, the page title and
//! the load state; this module turns them into what the header shows.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

/// The title: the page title when the page has one, otherwise the title the screen was opened with.
pub fn title(page_title: Option<&str>, initial_title: &str) -> String {
    page_title.unwrap_or(initial_title).to_string()
}

/// The overflow actions, in the order the menu lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overflow {
    Refresh,
    Share,
    OpenInBrowser,
    ClearCookies,
}

/// Overflow menu order (`WebViewScreenContent`).
pub const OVERFLOW: [Overflow; 4] = [Overflow::Refresh, Overflow::Share, Overflow::OpenInBrowser, Overflow::ClearCookies];

impl Overflow {
    pub fn label(self) -> String {
        tr(match self {
            Self::Refresh => MsgKey::action_webview_refresh,
            Self::Share => MsgKey::action_share,
            Self::OpenInBrowser => MsgKey::action_open_in_browser,
            Self::ClearCookies => MsgKey::pref_clear_cookies,
        })
    }
}

/// The page engine's load state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoadState {
    Idle,
    /// The engine is starting; no progress is known.
    Initialising,
    /// A page is loading. `None` when the engine reports no progress.
    Loading(Option<f64>),
}

/// What the progress bar shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress {
    Hidden,
    Indeterminate,
    Fraction(f64),
}

/// Indeterminate while initialising, determinate while loading (a missing progress counts as complete), hidden when idle.
pub fn progress(state: LoadState) -> Progress {
    match state {
        LoadState::Idle => Progress::Hidden,
        LoadState::Initialising => Progress::Indeterminate,
        LoadState::Loading(fraction) => Progress::Fraction(fraction.unwrap_or(1.0).clamp(0.0, 1.0)),
    }
}

/// The header of the WebView screen. Built once; the screen updates it as the engine reports changes.
pub struct Header {
    pub widget: gtk::HeaderBar,
    title: gtk::Label,
    subtitle: gtk::Label,
    back: gtk::Button,
    forward: gtk::Button,
    progress: gtk::ProgressBar,
    indeterminate: Rc<Cell<bool>>,
}

/// Builds the header. `close`, `back`, `forward` and `overflow` run on the GTK thread when the matching control is used.
pub fn header(
    close: impl Fn() + 'static,
    back: impl Fn() + 'static,
    forward: impl Fn() + 'static,
    overflow: impl Fn(Overflow) + 'static,
) -> Header {
    let title = gtk::Label::builder().ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["title"]).build();
    let subtitle = gtk::Label::builder().ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["subtitle"]).build();
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 0);
    titles.append(&title);
    titles.append(&subtitle);

    let widget = gtk::HeaderBar::new();
    widget.set_title_widget(Some(&titles));
    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.connect_clicked(move |_| close());
    widget.pack_start(&close_button);

    let back_button = gtk::Button::from_icon_name("go-previous-symbolic");
    back_button.set_tooltip_text(Some(&tr(MsgKey::action_webview_back)));
    back_button.set_sensitive(false);
    back_button.connect_clicked(move |_| back());
    let forward_button = gtk::Button::from_icon_name("go-next-symbolic");
    forward_button.set_tooltip_text(Some(&tr(MsgKey::action_webview_forward)));
    forward_button.set_sensitive(false);
    forward_button.connect_clicked(move |_| forward());
    widget.pack_end(&overflow_menu(overflow));
    widget.pack_end(&forward_button);
    widget.pack_end(&back_button);

    let progress = gtk::ProgressBar::new();
    progress.set_visible(false);
    let indeterminate = Rc::new(Cell::new(false));
    // the indeterminate bar is pulsed on a timer; the timer ends with the bar
    let weak = progress.downgrade();
    let pulsing = indeterminate.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(100), move || match weak.upgrade() {
        Some(bar) => {
            if pulsing.get() {
                bar.pulse();
            }
            gtk::glib::ControlFlow::Continue
        }
        None => gtk::glib::ControlFlow::Break,
    });

    Header { widget, title, subtitle, back: back_button, forward: forward_button, progress, indeterminate }
}

fn overflow_menu(overflow: impl Fn(Overflow) + 'static) -> gtk::MenuButton {
    let popover_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let popover = gtk::Popover::new();
    let overflow = Rc::new(overflow);
    for item in OVERFLOW {
        let button = gtk::Button::with_label(&item.label());
        button.add_css_class("flat");
        let overflow = overflow.clone();
        let popover = popover.clone();
        button.connect_clicked(move |_| {
            popover.popdown();
            overflow(item);
        });
        popover_box.append(&button);
    }
    popover.set_child(Some(&popover_box));
    gtk::MenuButton::builder().icon_name("view-more-symbolic").popover(&popover).build()
}

impl Header {
    /// Shows the page title (or `initial_title`) and the current URL as the subtitle.
    pub fn set_page(&self, page_title: Option<&str>, initial_title: &str, url: &str) {
        self.title.set_text(&title(page_title, initial_title));
        self.subtitle.set_text(url);
    }

    /// Back and Forward are sensitive only when the engine can go that way.
    pub fn set_navigation(&self, can_go_back: bool, can_go_forward: bool) {
        self.back.set_sensitive(can_go_back);
        self.forward.set_sensitive(can_go_forward);
    }

    /// Applies the load state to the progress bar.
    pub fn set_load(&self, state: LoadState) {
        match progress(state) {
            Progress::Hidden => {
                self.indeterminate.set(false);
                self.progress.set_visible(false);
            }
            Progress::Indeterminate => {
                self.indeterminate.set(true);
                self.progress.set_visible(true);
            }
            Progress::Fraction(fraction) => {
                self.indeterminate.set(false);
                self.progress.set_visible(true);
                self.progress.set_fraction(fraction);
            }
        }
    }

    /// The progress bar, for the screen to place under the content.
    pub fn progress_bar(&self) -> &gtk::ProgressBar {
        &self.progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_title_wins_over_the_initial_title() {
        assert_eq!(title(Some("Chapter list"), "Source"), "Chapter list");
        assert_eq!(title(None, "Source"), "Source");
    }

    #[test]
    fn progress_is_indeterminate_while_initialising_and_hidden_when_idle() {
        assert_eq!(progress(LoadState::Initialising), Progress::Indeterminate);
        assert_eq!(progress(LoadState::Idle), Progress::Hidden);
    }

    #[test]
    fn a_loading_page_without_progress_reads_as_complete() {
        assert_eq!(progress(LoadState::Loading(None)), Progress::Fraction(1.0));
        assert_eq!(progress(LoadState::Loading(Some(0.4))), Progress::Fraction(0.4));
        assert_eq!(progress(LoadState::Loading(Some(3.0))), Progress::Fraction(1.0));
    }

    #[test]
    fn overflow_lists_refresh_share_open_and_clear_cookies_in_that_order() {
        assert_eq!(OVERFLOW, [Overflow::Refresh, Overflow::Share, Overflow::OpenInBrowser, Overflow::ClearCookies]);
        assert_eq!(Overflow::Refresh.label(), tr(MsgKey::action_webview_refresh));
        assert_eq!(Overflow::ClearCookies.label(), tr(MsgKey::pref_clear_cookies));
    }
}
