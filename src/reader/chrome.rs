//! Reader chrome: the initial load error card (T-43a-024) and the top bar (T-43a-040).
//!
//! The pure helpers hold the text and the source-dependent overflow rule, so they are tested without GTK.
//! The widget builders only assemble them.

use std::rc::Rc;

use gtk::prelude::*;

use super::model::LoadError;
use super::strings;

/// The card text for an initial load failure. A blank message shows the generic text, so no card is empty. The
/// downloaded-only text is shown as is, since it is not a page-load failure (T-43a-074).
pub fn error_card_message(error: &LoadError) -> String {
    if error.message == strings::CHAPTER_NOT_DOWNLOADED {
        return error.message.clone();
    }
    let message = if error.message.trim().is_empty() { strings::UNKNOWN_ERROR } else { error.message.as_str() };
    strings::failed_to_load_pages(message)
}

/// The overflow entries of the top bar. HTTP sources get the three web entries; local sources get none.
pub fn overflow_items(is_http_source: bool) -> &'static [&'static str] {
    if is_http_source {
        &[strings::ACTION_OPEN_IN_WEB_VIEW, strings::ACTION_OPEN_IN_BROWSER, strings::ACTION_SHARE]
    } else {
        &[]
    }
}

/// The web address of a chapter for the overflow entries: only an absolute http or https url has one. A relative
/// source url needs the source's base url, which the reader does not have.
pub fn web_url(chapter_url: &str) -> Option<String> {
    let url = chapter_url.trim();
    (url.starts_with("https://") || url.starts_with("http://")).then(|| url.to_owned())
}

/// The bookmark action's label for the chapter's state.
pub fn bookmark_label(bookmarked: bool) -> &'static str {
    if bookmarked { strings::ACTION_REMOVE_BOOKMARK } else { strings::ACTION_BOOKMARK }
}

/// The error card: the message, a Retry button and a Close button. The reader does not close by itself.
pub fn build_error_card(error: &LoadError, on_retry: impl Fn() + 'static, on_close: impl Fn() + 'static) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 12);
    card.set_halign(gtk::Align::Center);
    card.set_valign(gtk::Align::Center);
    card.set_margin_start(16);
    card.set_margin_end(16);
    let label = gtk::Label::new(Some(&error_card_message(error)));
    label.set_wrap(true);
    label.set_max_width_chars(48);
    card.append(&label);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::Center);
    let retry = gtk::Button::with_label(strings::RETRY);
    retry.connect_clicked(move |_| on_retry());
    let close = gtk::Button::with_label(strings::CLOSE);
    close.connect_clicked(move |_| on_close());
    row.append(&retry);
    row.append(&close);
    card.append(&row);
    card
}

/// What the top bar shows and does. Callbacks are plain closures, run on the GTK thread.
pub struct TopBar {
    pub manga_title: String,
    pub chapter_name: String,
    pub bookmarked: bool,
    pub is_http_source: bool,
}

/// The top bar actions. `on_overflow` gets the index into [`overflow_items`].
pub struct TopBarActions {
    pub on_back: Box<dyn Fn()>,
    pub on_open_manga: Box<dyn Fn()>,
    pub on_bookmark: Box<dyn Fn()>,
    pub on_overflow: Box<dyn Fn(usize)>,
}

/// The top bar in a revealer that slides in and out in 200 ms.
pub fn build_top_bar(bar: &TopBar, actions: TopBarActions) -> gtk::Revealer {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_margin_start(6);
    row.set_margin_end(6);

    let back = gtk::Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some("Back"));
    back.update_property(&[gtk::accessible::Property::Label("Back")]);
    back.connect_clicked(move |_| (actions.on_back)());
    row.append(&back);

    let title_block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    title_block.set_hexpand(true);
    let title = gtk::Label::new(Some(&bar.manga_title));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.add_css_class("heading");
    let subtitle = gtk::Label::new(Some(&bar.chapter_name));
    subtitle.set_xalign(0.0);
    subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    subtitle.add_css_class("caption");
    title_block.append(&title);
    title_block.append(&subtitle);
    let open = gtk::Button::builder().child(&title_block).has_frame(false).build();
    open.connect_clicked(move |_| (actions.on_open_manga)());
    row.append(&open);

    let bookmark = gtk::ToggleButton::builder()
        .icon_name(if bar.bookmarked { "bookmark-filled-symbolic" } else { "bookmark-new-symbolic" })
        .active(bar.bookmarked)
        .tooltip_text(bookmark_label(bar.bookmarked))
        .build();
    bookmark.connect_clicked(move |_| (actions.on_bookmark)());
    bookmark.update_property(&[gtk::accessible::Property::Label(bookmark_label(bar.bookmarked))]);
    row.append(&bookmark);

    let items = overflow_items(bar.is_http_source);
    if !items.is_empty() {
        let popover = gtk::Popover::new();
        let entries = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let on_overflow = Rc::new(actions.on_overflow);
        for (index, text) in items.iter().enumerate() {
            let item = gtk::Button::with_label(text);
            item.add_css_class("flat");
            let on_overflow = Rc::clone(&on_overflow);
            let closing = popover.clone();
            item.connect_clicked(move |_| {
                closing.popdown();
                on_overflow(index);
            });
            entries.append(&item);
        }
        popover.set_child(Some(&entries));
        let menu = gtk::MenuButton::builder().icon_name("view-more-symbolic").popover(&popover).build();
        row.append(&menu);
    }

    let revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .transition_duration(200)
        .reveal_child(true)
        .child(&row)
        .build();
    revealer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_card_message_uses_the_load_error_and_never_a_blank_text() {
        assert_eq!(error_card_message(&LoadError::new("No pages found")), "Failed to load pages: No pages found");
        assert_eq!(error_card_message(&LoadError::new("  ")), "Failed to load pages: Unknown error");
    }

    #[test]
    fn the_downloaded_only_card_has_no_failure_prefix() {
        let error = LoadError::new(crate::reader::session::list_error_message(&crate::reader::chapters::ListError::CurrentChapterMissing));
        assert_eq!(
            error_card_message(&error),
            "The chapter is not downloaded. Turn off downloaded-only mode or download it first."
        );
    }

    #[test]
    fn overflow_items_only_for_http_sources() {
        assert_eq!(overflow_items(true), ["Open in WebView", "Open in browser", "Share"]);
        assert!(overflow_items(false).is_empty());
    }

    #[test]
    fn only_an_absolute_chapter_url_has_a_web_address() {
        assert_eq!(web_url(" https://example.org/c/1 ").as_deref(), Some("https://example.org/c/1"));
        assert_eq!(web_url("/c/1"), None);
        assert_eq!(web_url(""), None);
    }

    #[test]
    fn bookmark_label_follows_the_state() {
        assert_eq!(bookmark_label(false), "Bookmark chapter");
        assert_eq!(bookmark_label(true), "Unbookmark chapter");
    }
}
