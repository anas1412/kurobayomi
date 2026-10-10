//! The base list item widget of the manga and anime migration screens (T-38a-044, T-39a-061): `BaseMangaListItem` and
//! `BaseAnimeListItem` upstream. A row of 76 px with 16 px side and 3 px vertical padding and three slots, cover, content
//! and actions, laid out as `list_row` says. A click on the row runs the row action; a click on the cover runs the cover
//! action, which defaults to the row action. The migration screens fill the slots; this file knows nothing of them.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Orientation as O};

use super::list_row::{PAD_HORIZONTAL, PAD_VERTICAL, ROW_HEIGHT, SLOTS, Slot, cover_action};

/// What a click in the row runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Row,
    Cover,
}

/// The row. `cover` and `actions` are optional slots; `content` takes the free width.
pub struct ListItem {
    pub root: GBox,
}

/// Builds one row. `on_cover` is the action of a click on the cover, or `None` to run `on_row` there too.
pub fn build(
    cover: Option<&gtk::Widget>,
    content: &gtk::Widget,
    actions: Option<&gtk::Widget>,
    on_row: Rc<dyn Fn()>,
    on_cover: Option<Rc<dyn Fn()>>,
) -> ListItem {
    let root = GBox::new(O::Horizontal, 16);
    root.set_height_request(ROW_HEIGHT);
    root.set_margin_start(PAD_HORIZONTAL);
    root.set_margin_end(PAD_HORIZONTAL);
    root.set_margin_top(PAD_VERTICAL);
    root.set_margin_bottom(PAD_VERTICAL);
    root.set_valign(Align::Center);
    let cover_target = cover_action(Target::Row, on_cover.as_ref().map(|_| Target::Cover));
    for slot in SLOTS {
        match slot {
            Slot::Cover => {
                if let Some(cover) = cover {
                    attach_click(cover, cover_target, &on_row, on_cover.as_ref());
                    root.append(cover);
                }
            }
            Slot::Content => {
                content.set_hexpand(true);
                content.set_valign(Align::Center);
                root.append(content);
            }
            Slot::Actions => {
                if let Some(actions) = actions {
                    root.append(actions);
                }
            }
        }
    }
    attach_click(root.upcast_ref(), Target::Row, &on_row, None);
    ListItem { root }
}

/// A click on `widget` runs the row action, or the cover action for `Target::Cover`. The gesture claims the press, so a
/// click on the cover does not also run the row action.
fn attach_click(widget: &gtk::Widget, target: Target, on_row: &Rc<dyn Fn()>, on_cover: Option<&Rc<dyn Fn()>>) {
    let run = match (target, on_cover) {
        (Target::Cover, Some(cover)) => Rc::clone(cover),
        _ => Rc::clone(on_row),
    };
    let click = gtk::GestureClick::builder().button(1).build();
    click.connect_released(move |gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        run();
    });
    widget.add_controller(click);
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn click(widget: &impl IsA<gtk::Widget>) {
        let controllers = widget.observe_controllers();
        let gesture = (0..controllers.n_items()).filter_map(|i| controllers.item(i).and_downcast::<gtk::GestureClick>()).next().expect("a click gesture");
        gesture.emit_by_name::<()>("released", &[&1i32, &0.0f64, &0.0f64]);
    }

    #[test]
    fn the_row_has_the_upstream_size_padding_and_slot_order() {
        crate::testing::with_display(|| {
            let (cover, content, actions) = (gtk::Label::new(Some("c")), gtk::Label::new(Some("t")), gtk::Label::new(Some("a")));
            let item = build(Some(cover.upcast_ref()), content.upcast_ref(), Some(actions.upcast_ref()), Rc::new(|| {}), None);
            let root = &item.root;
            assert_eq!(root.height_request(), 76);
            assert_eq!((root.margin_start(), root.margin_end(), root.margin_top(), root.margin_bottom()), (16, 16, 3, 3));
            let kids: Vec<gtk::Widget> = std::iter::successors(root.first_child(), |w| w.next_sibling()).collect();
            assert_eq!(kids.len(), 3);
            assert!(kids[0] == *cover.upcast_ref::<gtk::Widget>() && kids[1] == *content.upcast_ref::<gtk::Widget>() && kids[2] == *actions.upcast_ref::<gtk::Widget>(), "cover, content, actions");
        });
    }

    #[test]
    fn a_cover_click_runs_the_row_action_unless_the_cover_has_its_own() {
        crate::testing::with_display(|| {
            let (rows, covers) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
            let on_row: Rc<dyn Fn()> = Rc::new({
                let rows = Rc::clone(&rows);
                move || rows.set(rows.get() + 1)
            });
            let cover = gtk::Label::new(None);
            let content = gtk::Label::new(None);
            let plain = build(Some(cover.upcast_ref()), content.upcast_ref(), None, Rc::clone(&on_row), None);
            click(&cover);
            assert_eq!((rows.get(), covers.get()), (1, 0), "the cover defaults to the row action");
            click(&plain.root);
            assert_eq!(rows.get(), 2);

            let on_cover: Rc<dyn Fn()> = Rc::new({
                let covers = Rc::clone(&covers);
                move || covers.set(covers.get() + 1)
            });
            let (cover2, content2) = (gtk::Label::new(None), gtk::Label::new(None));
            let own = build(Some(cover2.upcast_ref()), content2.upcast_ref(), None, on_row, Some(on_cover));
            click(&cover2);
            assert_eq!((rows.get(), covers.get()), (2, 1), "its own cover action runs instead");
            click(&own.root);
            assert_eq!(rows.get(), 3);
        });
    }
}
