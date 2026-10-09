//! Where the app goes once a migration is done (T-41a-005, `MigrateMangaSearchScreen.onPopScreen`): with an entry page
//! on top, pop back to it and push the new entry; otherwise the new entry replaces the migrate page.

/// The two ways out of a migration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterMigrate {
    /// `popUntil { items.contains(lastItem) }`, then push the new entry.
    PopToLastThenPush,
    /// `navigator.replace(new entry)`.
    Replace,
}

pub fn after_migrate(top_is_entry_page: bool) -> AfterMigrate {
    if top_is_entry_page { AfterMigrate::PopToLastThenPush } else { AfterMigrate::Replace }
}

/// Runs the way out: `top` is the page now on top of the stack, `push_new` opens the migrated entry.
pub fn finish_migrate(top: &gtk::Widget, top_is_entry_page: bool, push_new: impl FnOnce()) {
    match after_migrate(top_is_entry_page) {
        AfterMigrate::PopToLastThenPush => {
            let last = top.clone();
            crate::ui::pop_until(move |page| *page == last);
        }
        AfterMigrate::Replace => crate::ui::pop(),
    }
    push_new();
}

#[cfg(test)]
pub(crate) mod tests {
    use gtk::prelude::*;

    use super::*;

    fn page(name: &str) -> gtk::Label {
        let page = gtk::Label::new(Some(name));
        page.set_widget_name(name);
        page
    }

    fn top() -> String {
        crate::ui::window().child().and_downcast::<gtk::Stack>().and_then(|s| s.visible_child()).map(|w| w.widget_name().to_string()).unwrap_or_default()
    }

    /// Lets the 400 ms removal timers of popped pages run.
    fn settle() {
        let context = gtk::glib::MainContext::default();
        let end = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while std::time::Instant::now() < end {
            while context.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Needs the navigation stack, so it runs inside the one GTK check of the lane (`smoke::browse_widgets_smoke`).
    pub(crate) fn smoke() {
        let app = gtk::Application::builder().application_id("org.kurobayomi.browse.test").build();
        let _ = app.register(gtk::gio::Cancellable::NONE);
        let window = gtk::ApplicationWindow::new(&app);
        let stack = gtk::Stack::new();
        window.set_child(Some(&stack));
        crate::ui::nav_init(&stack, &window);
        let (root, details, search) = (page("root"), page("details"), page("search"));
        for p in [&root, &details, &search] {
            crate::ui::push(p);
        }
        assert_eq!(top(), "search");

        // pop_until stops at the page that satisfies the predicate, and never pops the root
        crate::ui::pop_until(|w| w.widget_name() == "root");
        assert_eq!(top(), "root");
        crate::ui::pop_until(|_| false);
        assert_eq!(top(), "root", "the root stays");
        settle();

        // an entry page on top: nothing is popped, the new entry is pushed over it
        crate::ui::push(&page("entry"));
        let pushed = std::cell::Cell::new(false);
        let current = stack.visible_child().expect("page");
        finish_migrate(&current, true, || {
            pushed.set(true);
            crate::ui::push(&page("migrated"));
        });
        assert!(pushed.get());
        assert_eq!(top(), "migrated");
        settle();
        crate::ui::pop();
        assert_eq!(top(), "entry", "the entry page was kept under the new one");
        settle();

        // anything else on top: the migrate page is replaced by the new entry
        crate::ui::push(&page("migrate search"));
        finish_migrate(&stack.visible_child().expect("page"), false, || crate::ui::push(&page("replacement")));
        assert_eq!(top(), "replacement");
        settle();
        crate::ui::pop();
        assert_eq!(top(), "entry", "the migrate page is gone");
        settle();
        window.close();
    }

    #[test]
    fn an_entry_page_on_top_is_popped_back_to_and_anything_else_is_replaced() {
        assert_eq!(after_migrate(true), AfterMigrate::PopToLastThenPush);
        assert_eq!(after_migrate(false), AfterMigrate::Replace);
    }
}
