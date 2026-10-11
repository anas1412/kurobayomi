//! The reader's top bar (back, title, chapter, bookmark, detach) is never hidden: the user ruled that out. Only the
//! bottom bar follows the menu toggle. This guard fails when `set_menu` starts hiding the top bar again.

#[cfg(test)]
mod tests {
    #[test]
    fn the_reader_top_bar_is_never_hidden_by_the_menu_toggle() {
        let source = include_str!("screen.rs");
        let body = source.split("fn set_menu(").nth(1).expect("set_menu exists");
        let body = &body[..body.find("\n}\n").expect("end of set_menu")];
        assert!(body.contains("screen.top.set_reveal_child(true);"), "the top bar stays revealed");
        assert!(!body.contains("screen.top.set_reveal_child(visible)"), "the top bar does not follow the menu");
    }
}
