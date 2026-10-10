//! Info screen (T-49b-026): a full page with an icon, headline, subtitle, content and a bottom bar of
//! an accept button and an optional reject button.

use std::rc::Rc;

use gtk::prelude::*;

use crate::theme::style::class;

/// Gap above the icon in px.
pub const TOP_GAP: i32 = 48;
/// Icon size in px.
pub const ICON_SIZE: i32 = 48;
/// The icon's colour class: primary (not the amber filter tint).
pub const ICON_CLASS: &str = class::PRIMARY_ICON;
/// Subtitle opacity.
pub const SUBTITLE_ALPHA: f64 = 0.78;
/// Subtitle vertical padding in px.
pub const SUBTITLE_PAD_V: i32 = 8;
/// Horizontal padding of the page in px.
pub const PAD_H: i32 = 16;

/// The reject button shows only when both its text and its handler are given.
pub fn show_reject(reject_label: Option<&str>, has_handler: bool) -> bool {
    reject_label.is_some() && has_handler
}

/// The content of an info page.
pub struct InfoScreen<'a> {
    pub icon: &'a str,
    pub headline: &'a str,
    pub subtitle: &'a str,
    pub accept_label: &'a str,
    /// The accept button is insensitive when false.
    pub can_accept: bool,
    pub on_accept: Rc<dyn Fn()>,
    pub reject_label: Option<&'a str>,
    pub on_reject: Option<Rc<dyn Fn()>>,
}

impl InfoScreen<'_> {
    /// Builds the page: the header and `content` above a bottom bar with the buttons.
    pub fn build(&self, content: &impl IsA<gtk::Widget>) -> gtk::Box {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_margin_start(PAD_H);
        body.set_margin_end(PAD_H);
        body.set_margin_top(TOP_GAP);
        body.set_halign(gtk::Align::Fill);
        body.set_valign(gtk::Align::Start);
        body.set_vexpand(true);

        let icon = gtk::Image::from_icon_name(self.icon);
        icon.set_pixel_size(ICON_SIZE);
        icon.add_css_class(ICON_CLASS);
        body.append(&icon);

        let headline = gtk::Label::new(Some(self.headline));
        headline.set_wrap(true);
        body.append(&headline);

        let subtitle = gtk::Label::new(Some(self.subtitle));
        subtitle.set_wrap(true);
        subtitle.set_opacity(SUBTITLE_ALPHA);
        subtitle.set_margin_top(SUBTITLE_PAD_V);
        subtitle.set_margin_bottom(SUBTITLE_PAD_V);
        body.append(&subtitle);

        body.append(content);
        page.append(&body);

        let bar = gtk::Box::new(gtk::Orientation::Vertical, 8);
        bar.add_css_class(class::INFO_BAR);
        bar.set_margin_start(PAD_H);
        bar.set_margin_end(PAD_H);
        let accept = gtk::Button::with_label(self.accept_label);
        accept.add_css_class(class::BUTTON_FILLED);
        accept.set_hexpand(true);
        accept.set_sensitive(self.can_accept);
        let on_accept = self.on_accept.clone();
        accept.connect_clicked(move |_| on_accept());
        bar.append(&accept);
        if show_reject(self.reject_label, self.on_reject.is_some()) {
            if let (Some(label), Some(on_reject)) = (self.reject_label, self.on_reject.clone()) {
                let reject = gtk::Button::with_label(label);
                reject.add_css_class(class::BUTTON_TEXT);
                reject.connect_clicked(move |_| on_reject());
                bar.append(&reject);
            }
        }
        page.append(&bar);
        page
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_needs_text_and_a_handler() {
        assert!(show_reject(Some("Cancel"), true));
        assert!(!show_reject(Some("Cancel"), false));
        assert!(!show_reject(None, true));
    }

    #[test]
    fn the_icon_is_primary_not_the_filter_tint() {
        assert_eq!(ICON_CLASS, "kb-primary-icon");
        assert_ne!(ICON_CLASS, class::ACTIVE);
    }

    #[test]
    fn page_geometry() {
        assert_eq!((TOP_GAP, ICON_SIZE, PAD_H, SUBTITLE_PAD_V), (48, 48, 16, 8));
        assert_eq!(SUBTITLE_ALPHA, 0.78);
    }
}
