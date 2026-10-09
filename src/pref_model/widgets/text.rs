//! Text row (Kotlin `TextPreferenceWidget`): static title and optional subtitle, activatable only with a handler.

use std::rc::Rc;

use super::super::model::ItemCommon;
use super::row::{self, PrefRow, RowSpec};

pub fn build(common: &ItemCommon, on_click: Option<Rc<dyn Fn()>>, trailing: Option<&gtk::Widget>) -> PrefRow {
    row::build(RowSpec {
        title: &common.title,
        subtitle: common.subtitle.as_deref(),
        icon: common.icon,
        trailing,
        on_click,
    })
}
