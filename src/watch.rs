//! Binds a live query to the GTK main loop (T-08a-032). Every published result or error runs the callback there.

use gtk::glib;
use kuro_db::subscribe::{Published, Subscription};

/// Runs `on_change` on the GTK main loop for each publication of `sub`, until the database handle goes away. The
/// watch is read only after `changed` resolves and its value is cloned out, so no borrow is held across an await.
pub fn bind_watch<T: 'static>(mut sub: Subscription<T>, on_change: impl Fn(Published<T>) + 'static) {
    glib::spawn_future_local(async move {
        while let Some(published) = sub.changed().await {
            on_change(published);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use gtk::glib;
    use kuro_db::handle::Db;

    use super::bind_watch;

    #[test]
    fn the_first_result_reaches_the_callback_on_the_main_context() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = kuro_db::schema_unified::db_path(dir.path());
        let writer = kuro_db::schema_unified::open_file(&path).expect("schema");
        let db = Arc::new(Db::new(&path, writer).expect("manga handle"));
        let rt = kuro_core::runtime::build().expect("runtime");
        let _entered = rt.enter();
        let sub = db.subscribe(&["categories"], Duration::ZERO, |c| {
            Ok(c.query_row("SELECT count(*) FROM categories WHERE kind = 'manga'", [], |r| r.get::<_, i64>(0))?)
        });
        let ctx = glib::MainContext::new();
        let seen: Rc<RefCell<Vec<i64>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        ctx.with_thread_default(|| {
            bind_watch(sub, move |published| sink.borrow_mut().push(*published.expect("the count query succeeds")));
            let deadline = Instant::now() + Duration::from_secs(5);
            while seen.borrow().is_empty() && Instant::now() < deadline {
                ctx.iteration(false);
                std::thread::sleep(Duration::from_millis(5));
            }
        })
        .expect("the test thread owns the context");
        assert_eq!(*seen.borrow(), vec![1], "the schema seeds the Default category row");
    }
}
