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

/// The header with Back on the left (`on_back`) and `title` in the centre. The right side is left to the caller: the
/// first widget it packs with `pack_end` sits at the far right.
pub fn header(title: &impl IsA<gtk::Widget>, on_back: impl Fn() + 'static) -> gtk::HeaderBar {
    let header = gtk::HeaderBar::new();
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
    fn a_chapter_without_a_name_leaves_the_manga_alone() {
        assert_eq!(page_title("Manga", ""), "Manga");
        assert_eq!(page_title("Manga", "  "), "Manga");
    }
}
