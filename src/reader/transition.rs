//! Chapter transition card (`ChapterTransition.kt`). T-43a-022.
//!
//! [`content`] is the pure model: which header and chapter go in each block, the fallback text for a missing
//! side, and the gap warning. [`build`] turns it into GTK widgets. Names are cut to 5 lines and scanlators to 2
//! by the labels; the card is 460 px wide.

use kuro_data::chapter::Chapter;
use kuro_domain::items::missing::item_gap;
use gtk::prelude::*;
use gtk::pango;

use super::model::ChapterTransition;
use super::strings;

/// One chapter in a block: its header, name, scanlator and download state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChapterLine {
    pub header: &'static str,
    pub name: String,
    pub scanlator: Option<String>,
    pub downloaded: bool,
}

/// A block of the card: a chapter, or the "no chapter" notice for a side that does not exist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Chapter(ChapterLine),
    NoChapter(&'static str),
}

/// The whole card. `gap` is the missing-chapter count between the two chapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub top: Slot,
    pub bottom: Slot,
    pub gap: i32,
}

impl Content {
    /// The warning between the two blocks: only when the bottom block shows a chapter and the gap is positive.
    pub fn gap_warning(&self) -> Option<String> {
        match self.bottom {
            Slot::Chapter(_) if self.gap > 0 => Some(strings::missing_chapters_warning(self.gap)),
            _ => None,
        }
    }
}

fn line(header: &'static str, chapter: &Chapter, downloaded: bool) -> ChapterLine {
    ChapterLine {
        header,
        name: chapter.name.clone(),
        scanlator: chapter.scanlator.clone(),
        downloaded,
    }
}

/// `ChapterTransition` (`:49-85`). `from_downloaded` and `to_downloaded` are the download lookups for each side.
pub fn content(transition: &ChapterTransition, from_downloaded: bool, to_downloaded: bool) -> Content {
    let from = &transition.from().chapter;
    let to = transition.to().map(|c| &c.chapter);
    match transition {
        ChapterTransition::Prev { .. } => Content {
            top: match to {
                Some(to) => Slot::Chapter(line(strings::TRANSITION_PREVIOUS, to, to_downloaded)),
                None => Slot::NoChapter(strings::TRANSITION_NO_PREVIOUS),
            },
            bottom: Slot::Chapter(line(strings::TRANSITION_CURRENT, from, from_downloaded)),
            // calculateChapterGap(currChapter, goingToChapter)
            gap: item_gap(Some(from), to),
        },
        ChapterTransition::Next { .. } => Content {
            top: Slot::Chapter(line(strings::TRANSITION_FINISHED, from, from_downloaded)),
            bottom: match to {
                Some(to) => Slot::Chapter(line(strings::TRANSITION_NEXT, to, to_downloaded)),
                None => Slot::NoChapter(strings::TRANSITION_NO_NEXT),
            },
            // calculateChapterGap(goingToChapter, currChapter)
            gap: item_gap(to, Some(from)),
        },
    }
}

/// The GTK card. The width is fixed at 460 px, the largest it is laid out at.
pub fn build(content: &Content) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 24);
    column.set_halign(gtk::Align::Center);
    column.set_size_request(460, -1);
    column.append(&slot_widget(&content.top));
    if let Some(warning) = content.gap_warning() {
        column.append(&notice(&warning, "dialog-warning-symbolic", "error"));
    }
    column.append(&slot_widget(&content.bottom));
    column
}

fn slot_widget(slot: &Slot) -> gtk::Widget {
    match slot {
        Slot::Chapter(line) => chapter_text(line).upcast(),
        Slot::NoChapter(text) => notice(text, "dialog-information-symbolic", "accent").upcast(),
    }
}

fn chapter_text(line: &ChapterLine) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 4);
    block.set_halign(gtk::Align::Center);
    let header = gtk::Label::new(Some(line.header));
    header.add_css_class("title-4");
    block.append(&header);

    let name_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    name_row.set_halign(gtk::Align::Center);
    if line.downloaded {
        let check = gtk::Image::from_icon_name("emblem-ok-symbolic");
        check.set_tooltip_text(Some(strings::LABEL_DOWNLOADED));
        name_row.append(&check);
    }
    let name = gtk::Label::new(Some(&line.name));
    name.set_wrap(true);
    name.set_lines(5);
    name.set_ellipsize(pango::EllipsizeMode::End);
    name.set_justify(gtk::Justification::Center);
    name.add_css_class("title-2");
    name_row.append(&name);
    block.append(&name_row);

    if let Some(scanlator) = line.scanlator.as_deref().filter(|s| !s.is_empty()) {
        let by = gtk::Label::new(Some(scanlator));
        by.set_wrap(true);
        by.set_lines(2);
        by.set_ellipsize(pango::EllipsizeMode::End);
        by.add_css_class("dim-label");
        block.append(&by);
    }
    block
}

fn notice(text: &str, icon: &str, css: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.set_halign(gtk::Align::Center);
    row.set_margin_start(16);
    row.set_margin_end(16);
    row.set_margin_top(12);
    row.set_margin_bottom(12);
    row.add_css_class("card");
    let image = gtk::Image::from_icon_name(icon);
    image.add_css_class(css);
    row.append(&image);
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_max_width_chars(40);
    row.append(&label);
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::model::ReaderChapter;
    use std::rc::Rc;

    fn chapter(id: i64, number: f64, name: &str, scanlator: Option<&str>) -> Rc<ReaderChapter> {
        let mut c = Chapter::create();
        c.id = id;
        c.url = format!("/c/{id}");
        c.name = name.into();
        c.chapter_number = number;
        c.scanlator = scanlator.map(String::from);
        ReaderChapter::new(c)
    }

    #[test]
    fn prev_shows_previous_over_the_target_and_current_under() {
        let from = chapter(2, 2.0, "Two", Some("S"));
        let to = chapter(1, 1.0, "One", None);
        let t = ChapterTransition::Prev { from, to: Some(to) };
        let c = content(&t, false, true);
        assert_eq!(c.top, Slot::Chapter(ChapterLine { header: "Previous:", name: "One".into(), scanlator: None, downloaded: true }));
        assert_eq!(c.bottom, Slot::Chapter(ChapterLine { header: "Current:", name: "Two".into(), scanlator: Some("S".into()), downloaded: false }));
        assert_eq!(c.gap, 0);
        assert_eq!(c.gap_warning(), None);
    }

    #[test]
    fn next_shows_finished_over_the_current_and_next_under_the_target() {
        let from = chapter(1, 1.0, "One", None);
        let to = chapter(2, 2.0, "Two", None);
        let t = ChapterTransition::Next { from, to: Some(to) };
        let c = content(&t, true, false);
        assert!(matches!(&c.top, Slot::Chapter(l) if l.header == "Finished:" && l.name == "One" && l.downloaded));
        assert!(matches!(&c.bottom, Slot::Chapter(l) if l.header == "Next:" && l.name == "Two"));
    }

    #[test]
    fn a_missing_target_shows_the_no_chapter_text_in_its_slot() {
        let from = chapter(2, 2.0, "Two", None);
        let prev = ChapterTransition::Prev { from: Rc::clone(&from), to: None };
        assert_eq!(content(&prev, false, false).top, Slot::NoChapter("There's no previous chapter"));
        let next = ChapterTransition::Next { from, to: None };
        assert_eq!(content(&next, false, false).bottom, Slot::NoChapter("There's no next chapter"));
    }

    #[test]
    fn gap_warning_only_when_the_gap_is_positive_and_the_bottom_shows_a_chapter() {
        let from = chapter(1, 5.0, "Five", None);
        let to = chapter(2, 9.0, "Nine", None);
        let next = ChapterTransition::Next { from: Rc::clone(&from), to: Some(Rc::clone(&to)) };
        let c = content(&next, false, false);
        assert_eq!(c.gap, 3);
        assert_eq!(c.gap_warning().as_deref(), Some("Skipping 3 chapters, either the source is missing them or they have been filtered out"));

        let close = chapter(3, 5.5, "Five and a half", None);
        let no_gap = ChapterTransition::Next { from, to: Some(close) };
        assert_eq!(content(&no_gap, false, false).gap_warning(), None);

        let unknown = chapter(4, -1.0, "?", None);
        let unknown_next = ChapterTransition::Next { from: unknown, to: Some(to) };
        assert_eq!(content(&unknown_next, false, false).gap, 0, "unknown numbers never warn");
    }
}
