//! Startup work on the GTK side: the texture-limit probe (T-01a-022), the splash window shown when migrations take
//! longer than 300 ms (T-01a-043) and the launcher badge for unread updates (T-01a-025).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use chrono::Local;
use gtk::glib;
use gtk::prelude::*;
use kuro_domain::kind::EntryKind;
use kuro_domain::updates::repo::UpdatesRepo;
use kuro_domain::updates::window::three_months_ago;
use kuro_platform::linux::launcher;

use crate::bind;
use crate::data;
use crate::watch::bind_watch;

/// How long migrations may run before the splash window shows.
pub const SPLASH_DELAY: Duration = Duration::from_millis(300);

/// `GL_MAX_TEXTURE_SIZE`.
const GL_MAX_TEXTURE_SIZE: u32 = 0x0D33;

/// Reads `GL_MAX_TEXTURE_SIZE` in a short-lived GL context and records it for `device_texture_limit()`. Without a
/// display or a GL context nothing is recorded and the limit falls back to 16384.
pub fn probe_texture_limit() {
    if let Some(size) = gl_max_texture_size() {
        kuro_platform::device::record_texture_probe(size);
    }
}

fn gl_max_texture_size() -> Option<i32> {
    let display = gtk::gdk::Display::default()?;
    let context = display.create_gl_context().ok()?;
    context.realize().ok()?;
    context.make_current();
    let size = read_gl_integer(GL_MAX_TEXTURE_SIZE);
    gtk::gdk::GLContext::clear_current();
    size.filter(|s| *s > 0)
}

/// `glGetIntegerv(pname)` through libepoxy, which GDK has loaded: each GL entry point is a function-pointer variable
/// named `epoxy_<name>`. Only valid while a GL context is current.
fn read_gl_integer(pname: u32) -> Option<i32> {
    type GetIntegerv = unsafe extern "C" fn(u32, *mut i32);
    // SAFETY: the symbol is the address of epoxy's `epoxy_glGetIntegerv` pointer variable; it is read once, and the
    // function it points to is called with a valid out pointer while the probe context is current.
    unsafe {
        let var = libc::dlsym(libc::RTLD_DEFAULT, c"epoxy_glGetIntegerv".as_ptr()) as *const Option<GetIntegerv>;
        let function = (*var.as_ref()?)?;
        let mut value = 0;
        function(pname, &mut value);
        Some(value)
    }
}

/// Runs `work` on a worker thread while the main loop keeps turning. When it is still running after `delay`,
/// `show_splash` runs once. `finish` then gets the result and the splash, so the caller presents its window first and
/// removes the splash after. No splash is created when `work` ends within `delay`.
pub fn run_with_splash<T, S>(
    work: impl FnOnce() -> T + Send + 'static,
    delay: Duration,
    show_splash: impl FnOnce() -> S + 'static,
    finish: impl FnOnce(T, Option<S>) + 'static,
) where
    T: Send + 'static,
    S: 'static,
{
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(work());
    });
    let splash: Rc<RefCell<Option<S>>> = Rc::default();
    let done = Rc::new(Cell::new(false));
    {
        let (splash, done) = (Rc::clone(&splash), Rc::clone(&done));
        glib::spawn_future_local(async move {
            glib::timeout_future(delay).await;
            if !done.get() {
                *splash.borrow_mut() = Some(show_splash());
            }
        });
    }
    glib::spawn_future_local(async move {
        let result = rx.recv().await.expect("the startup worker panicked");
        done.set(true);
        let splash = splash.borrow_mut().take();
        finish(result, splash);
    });
}

/// The splash: a small undecorated window with the app name and a spinner. It takes no focus, and the caller
/// destroys it after presenting the main window.
pub fn splash_window(app: &gtk::Application) -> gtk::Window {
    let spinner = gtk::Spinner::builder().spinning(true).build();
    spinner.set_size_request(32, 32);
    let name = gtk::Label::builder().label("Kurobayomi").css_classes(["title-2"]).build();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    for widget in [name.upcast_ref::<gtk::Widget>(), spinner.upcast_ref()] {
        column.append(widget);
    }
    column.set_margin_top(32);
    column.set_margin_bottom(32);
    column.set_margin_start(48);
    column.set_margin_end(48);
    let window = gtk::Window::builder()
        .application(app)
        .decorated(false)
        .resizable(false)
        .can_focus(false)
        .focusable(false)
        .child(&column)
        .build();
    window.present();
    window
}

/// The launcher entry name of the application: GApplication ids are the desktop file's name.
pub fn desktop_file_name(app_id: &str) -> String {
    format!("{app_id}.desktop")
}

/// Keeps the launcher badge at the unread updates of the last three months, cleared at 0 and hidden while the app
/// lock is on. The window ties the lock-preference binding to its lifetime. Needs the data to be installed.
pub fn publish_launcher_badge(win: &gtk::ApplicationWindow, app_id: &str) {
    let desktop = desktop_file_name(app_id);
    let counts = Rc::new(Cell::new([0_i64; 2]));
    let push: Rc<dyn Fn()> = Rc::new({
        let counts = Rc::clone(&counts);
        move || {
            let locked = data::app().security.use_authenticator.get();
            let badge = launcher::badge(counts.get().iter().sum(), locked);
            let desktop = desktop.clone();
            tokio::spawn(async move { launcher::publish(&desktop, badge).await });
        }
    });
    let since = three_months_ago(&Local::now());
    for (slot, kind) in [EntryKind::Manga, EntryKind::Anime].into_iter().enumerate() {
        let (counts, push) = (Rc::clone(&counts), Rc::clone(&push));
        let feed = UpdatesRepo::new(data::app().db(kind)).watch_with_read(kind, false, since);
        bind_watch(feed, move |rows| {
            let Ok(rows) = rows else { return };
            let mut all = counts.get();
            all[slot] = rows.len() as i64;
            counts.set(all);
            push();
        });
    }
    let lock = data::app().security.use_authenticator.clone();
    bind::bind(win, &lock, move |_, _| push());
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Instant;

    use super::*;

    /// Runs `body` on a private main context that the test thread owns, and pumps it until `done` holds.
    fn on_context<R>(body: impl FnOnce() -> R, done: impl Fn(&R) -> bool, extra: Duration) -> R {
        let ctx = glib::MainContext::new();
        ctx.with_thread_default(|| {
            let result = body();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !done(&result) && Instant::now() < deadline {
                ctx.iteration(false);
                std::thread::sleep(Duration::from_millis(2));
            }
            // let a late timer fire, to prove it does nothing
            let end = Instant::now() + extra;
            while Instant::now() < end {
                ctx.iteration(false);
                std::thread::sleep(Duration::from_millis(2));
            }
            result
        })
        .expect("the test thread owns the context")
    }

    fn run(work_for: Duration, extra: Duration) -> (Rc<Cell<u32>>, Rc<Cell<Option<bool>>>) {
        let shown = Rc::new(Cell::new(0));
        let finished = Rc::new(Cell::new(None));
        let (s, f, watched) = (Rc::clone(&shown), Rc::clone(&finished), Rc::clone(&finished));
        on_context(
            move || {
                run_with_splash(
                    move || std::thread::sleep(work_for),
                    Duration::from_millis(60),
                    move || s.set(s.get() + 1),
                    move |(), splash| f.set(Some(splash.is_some())),
                );
                watched
            },
            |f| f.get().is_some(),
            extra,
        );
        (shown, finished)
    }

    #[test]
    fn no_splash_when_the_work_ends_within_the_delay() {
        // the pump keeps turning past the 60 ms delay: the timer must not create a splash after the work is done
        let (shown, finished) = run(Duration::ZERO, Duration::from_millis(120));
        assert_eq!(finished.get(), Some(false));
        assert_eq!(shown.get(), 0);
    }

    #[test]
    fn a_splash_shows_once_when_the_work_outlasts_the_delay() {
        let (shown, finished) = run(Duration::from_millis(200), Duration::ZERO);
        assert_eq!(shown.get(), 1);
        assert_eq!(finished.get(), Some(true), "finish receives the splash so the caller can remove it");
    }

    #[test]
    fn the_desktop_file_is_the_application_id() {
        assert_eq!(desktop_file_name("io.github.blackbox.Kurobayomi"), "io.github.blackbox.Kurobayomi.desktop");
    }
}
