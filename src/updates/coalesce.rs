//! A live query on the GTK main loop with a quiet-time coalesce (T-42a-013): a burst of publications collapses into
//! the newest one, delivered once the burst has been quiet for `quiet_ms`. Dropping or aborting the returned handle
//! stops the watch, which is how a search change cancels the previous query.

use std::time::Duration;

use gtk::glib;
use kuro_db::subscribe::{Published, Subscription};

/// Runs `on_change` on the main loop for each coalesced publication of `sub`.
pub fn watch_coalesced<T: 'static>(
    mut sub: Subscription<T>,
    quiet_ms: u64,
    on_change: impl Fn(Published<T>) + 'static,
) -> glib::JoinHandle<()> {
    glib::spawn_future_local(async move {
        while let Some(mut latest) = sub.changed().await {
            let mut closed = false;
            loop {
                match glib::future_with_timeout(Duration::from_millis(quiet_ms), sub.changed()).await {
                    Ok(Some(next)) => latest = next,
                    Ok(None) => {
                        closed = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
            on_change(latest);
            if closed {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::Instant;

    use kuro_data::manga::{Manga, MangaRepo};
    use kuro_db::handle::Db;

    use super::*;

    #[test]
    fn a_burst_of_writes_ends_with_the_newest_value() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("manga schema");
        let db = Arc::new(Db::new(&path, writer).expect("manga handle"));
        let rt = kuro_core::runtime::build().expect("runtime");
        let _entered = rt.enter();
        let sub = db.subscribe(&["mangas"], Duration::ZERO, |c| {
            Ok(c.query_row("SELECT count(*) FROM mangas", [], |r| r.get::<_, i64>(0))?)
        });
        let ctx = glib::MainContext::new();
        let seen: Rc<RefCell<Vec<i64>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        ctx.with_thread_default(|| {
            let _handle = watch_coalesced(sub, 50, move |p| sink.borrow_mut().push(*p.expect("the count query succeeds")));
            let deadline = Instant::now() + Duration::from_secs(5);
            while seen.borrow().is_empty() && Instant::now() < deadline {
                ctx.iteration(false);
                std::thread::sleep(Duration::from_millis(5));
            }
            let first = *seen.borrow().last().expect("the first result arrived");
            for n in 0..4 {
                MangaRepo::new(db.clone())
                    .insert(&Manga { source: 1, url: format!("u{n}"), title: format!("t{n}"), ..Manga::create() })
                    .expect("insert a manga");
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            while *seen.borrow().last().unwrap_or(&0) != first + 4 && Instant::now() < deadline {
                ctx.iteration(false);
                std::thread::sleep(Duration::from_millis(5));
            }
        })
        .expect("the test thread owns the context");
        let seen = seen.borrow();
        assert_eq!(*seen.last().expect("results"), seen[0] + 4, "the newest count is the one shown last");
    }
}
