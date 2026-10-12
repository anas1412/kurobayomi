//! The cover of a local entry (area 34, behaviour 9.3): the `file://` thumbnail that `scan` stored, shown cropped to the
//! card. A row without a thumbnail, or a file that is gone, gives an empty picture and the card keeps its placeholder
//! look. Decoding goes through GTK's picture widget (ponytail: not the kuro-images cache yet).

use gtk::gio;
use gtk::prelude::*;

/// Only a `file://` thumbnail is a local cover; anything else leaves the picture empty.
pub fn is_local_thumbnail(url: &str) -> bool {
    url.starts_with("file://")
}

/// A cropped picture of the thumbnail, sized `width` by `height`. The library card calls it for a local entry.
pub fn cover_widget(thumbnail_url: Option<&str>, width: i32, height: i32) -> gtk::Overlay {
    let picture = gtk::Picture::builder().content_fit(gtk::ContentFit::Cover).can_shrink(true).build();
    if let Some(url) = thumbnail_url.filter(|u| is_local_thumbnail(u)) {
        picture.set_file(Some(&gio::File::for_uri(url)));
    }
    // exactly `width` x `height`: a large thumbnail must not make the row or card larger
    crate::ui::sized(&picture, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_file_url_is_a_local_cover() {
        assert!(is_local_thumbnail("file:///home/a/Series/Ch%201/001.png"));
        assert!(!is_local_thumbnail("https://example.org/cover.jpg"));
        assert!(!is_local_thumbnail(""));
    }
}
