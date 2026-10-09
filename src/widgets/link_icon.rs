//! Link icon button (T-49a-032): a flat icon button in the primary tint that opens a URL with
//! `gtk::UriLauncher`. The accessible name is the label. A failed launch is logged, never a crash.

use gtk::gio;
use gtk::prelude::*;

/// Warning text for a failed launch.
pub fn launch_failure_message(error: &str) -> String {
    format!("could not open link: {error}")
}

/// Opens `url` in the desktop's handler. The failure is logged as a warning.
pub fn open_url(url: &str) {
    let launcher = gtk::UriLauncher::new(url);
    launcher.launch(None::<&gtk::Window>, None::<&gio::Cancellable>, |result| {
        if let Err(error) = result {
            tracing::warn!("{}", launch_failure_message(&error.to_string()));
        }
    });
}

/// A flat icon button that opens `url` when clicked.
pub fn link_icon(icon_name: &str, label: &str, url: &str) -> gtk::Button {
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.add_css_class(crate::theme::style::class::LINK_ICON);
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.set_child(Some(&icon));
    button.set_tooltip_text(Some(label));
    button.set_margin_start(4);
    button.set_margin_end(4);
    button.set_margin_top(4);
    button.set_margin_bottom(4);
    let url = url.to_owned();
    button.connect_clicked(move |_| open_url(&url));
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_message_names_the_error() {
        assert_eq!(launch_failure_message("no handler"), "could not open link: no handler");
    }
}
