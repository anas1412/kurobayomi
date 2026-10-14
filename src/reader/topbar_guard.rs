//! The reader's top bar (back, title, chapter, bookmark, menu) hides only with auto-hide (user, 2026-10-14: the
//! `auto_hide_bars` setting, which replaced the earlier rule that it never hides). With the setting off it stays
//! revealed. This guard fails when `set_menu` hides the top bar without that setting.

#[cfg(test)]
mod tests {
    #[test]
    fn the_reader_top_bar_hides_only_with_auto_hide() {
        let source = include_str!("screen.rs");
        let body = source.split("fn set_menu(").nth(1).expect("set_menu exists");
        let body = &body[..body.find("\n}\n").expect("end of set_menu")];
        assert!(body.contains("screen.top.set_reveal_child(visible || !screen.auto_hide);"), "the top bar follows the menu only with auto-hide");
    }
}
