//! The header of an embedded page, shared by the reader and the player (docs/design/player-reader.md): a themed
//! `HeaderBar` with Back on the left, the title centred, and the page's own buttons on the right. The title text and
//! the label are pure or thin helpers; [`header`] only assembles them.
//!
//! The player and the reader both build their header with [`header`], so the two stay identical; the player adds its
//! quality and tracks menus on the right.

use gtk::prelude::*;

/// The title of a manga page: `<manga> - <chapter>`, or the manga alone when the chapter has no name.
pub fn page_title(manga: &str, chapter: &str) -> String {
    if chapter.trim().is_empty() { manga.to_owned() } else { format!("{manga} - {chapter}") }
}

/// A title label in the header: one line, ellipsized, with the `title` style.
pub fn title_label(text: &str) -> gtk::Label {
    gtk::Label::builder().label(text).css_classes(["title"]).ellipsize(gtk::pango::EllipsizeMode::End).width_chars(24).max_width_chars(100).build()
}

/// A two-line header title: the title on top and a small dim subtitle under it (the page position in the reader). The
/// subtitle line keeps its height while it is empty, so the title does not jump when the text comes and goes.
#[derive(Clone)]
pub struct TwoLineTitle {
    pub root: gtk::Box,
    subtitle: gtk::Label,
}

impl TwoLineTitle {
    /// Shows `text` under the title, or clears the line for `None`.
    pub fn set_subtitle(&self, text: Option<&str>) {
        self.subtitle.set_label(text.unwrap_or(""));
    }

    pub fn subtitle(&self) -> String {
        self.subtitle.label().to_string()
    }
}

/// A centred two-line title for [`header`]: `text` as [`title_label`], and an empty subtitle under it.
pub fn two_line_title(text: &str) -> TwoLineTitle {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.set_halign(gtk::Align::Center);
    root.set_valign(gtk::Align::Center);
    root.append(&title_label(text));
    let subtitle = gtk::Label::builder().css_classes(["subtitle", "dim-label"]).ellipsize(gtk::pango::EllipsizeMode::End).build();
    root.append(&subtitle);
    TwoLineTitle { root, subtitle }
}

/// `Page 3 of 12`, or `None` when there is no page on screen (a page or total not above zero, or a transition card).
pub fn page_position(page: i32, total: i32) -> Option<String> {
    (page > 0 && total > 0).then(|| format!("Page {page} of {total}"))
}

/// The header with Back on the left (`on_back`) and `title` in the centre. The right side is left to the caller: the
/// first widget it packs with `pack_end` sits at the far right.
pub fn header(title: &impl IsA<gtk::Widget>, on_back: impl Fn() + 'static) -> gtk::HeaderBar {
    let header = crate::ui::header_bar();
    header.set_decoration_layout(Some(&crate::ui::decoration(false)));
    header.set_title_widget(Some(title.upcast_ref()));
    let back = crate::ui::icon_button("go-previous-symbolic", "Back");
    back.connect_clicked(move |_| on_back());
    header.pack_start(&back);
    crate::ui::no_sticky_focus(&header);
    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_joins_the_manga_and_the_chapter() {
        assert_eq!(page_title("Manga", "Chapter 3"), "Manga - Chapter 3");
    }

    #[test]
    fn the_position_needs_a_page_and_a_total_above_zero() {
        assert_eq!(page_position(1, 23).as_deref(), Some("Page 1 of 23"));
        assert_eq!(page_position(-1, 23), None, "a transition card has no page");
        assert_eq!(page_position(0, 23), None);
        assert_eq!(page_position(1, 0), None);
    }

    #[test]
    fn the_subtitle_follows_set_subtitle() {
        crate::testing::with_display(|| {
            let title = two_line_title("Manga - Chapter 3");
            assert_eq!(title.subtitle(), "");
            title.set_subtitle(Some("Page 1 of 23"));
            assert_eq!(title.subtitle(), "Page 1 of 23");
            title.set_subtitle(None);
            assert_eq!(title.subtitle(), "");
        });
    }

    #[test]
    fn a_chapter_without_a_name_leaves_the_manga_alone() {
        assert_eq!(page_title("Manga", ""), "Manga");
        assert_eq!(page_title("Manga", "  "), "Manga");
    }
}
