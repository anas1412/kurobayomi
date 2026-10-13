//! The unattended library update (T-26a-020, T-26a-029): the `--library-update=auto|manual` option, the hand-over of a
//! manual request to the running app, and the user timer that runs the automatic pass without a window (mode B).
//!
//! Decisions:
//! - `main` reads the option before GTK starts, like `--run-backup`, so `shell::argv` never sees it as an option of its own.
//! - `manual` hands the request to the running app over D-Bus (`app.library-update`) and exits 0. With no app running
//!   there is nobody to hand it to, so the same process runs the pass itself.
//! - The unit folder is an argument: `kuro-platform` has no accessor for the user unit folder and is outside this lane.
//!   Nothing here writes a unit by itself; the settings action calls [`enable_timer`] after the user asks for it.
//! - Mode A (the background portal, autostart) and mode B (this timer) are mutually exclusive: [`Selection`] turns one
//!   off when the other is turned on, so a pass is never started twice for one interval.

#[cfg(test)]
use std::io;
#[cfg(test)]
use std::path::{Path, PathBuf};

use crate::library_update::pass::Trigger;
use crate::misc::runner;

/// The option name, `--library-update=<mode>`.
pub const OPTION: &str = "--library-update";

/// What `--library-update` asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The timer's run: one pass per kind, exit 0.
    Auto,
    /// A request for the running app to start a pass now.
    Manual,
}

/// The mode named on the command line: `Ok(None)` when the option is absent, an error text when its value is not
/// `auto` or `manual`. The last occurrence wins.
pub fn parse_mode(args: &[String]) -> Result<Option<Mode>, String> {
    let mut found = None;
    for arg in args {
        let Some(value) = arg.strip_prefix(OPTION).and_then(|rest| rest.strip_prefix('=')) else { continue };
        found = Some(match value {
            "auto" => Mode::Auto,
            "manual" => Mode::Manual,
            other => return Err(format!("{OPTION} takes auto or manual, got {other:?}")),
        });
    }
    Ok(found)
}

/// Runs the option from `main`: the exit status of the process.
pub fn run(mode: Mode) -> u8 {
    match mode {
        Mode::Auto => runner::run_headless(Trigger::Auto),
        Mode::Manual => forward_manual(),
    }
}

/// Activates `app.library-update("manual")` on the running app and exits 0 (T-26a-029). Without a running app, or
/// without a session bus, the pass runs here instead.
fn forward_manual() -> u8 {
    use gtk::gio;
    use gtk::prelude::*;
    let app = gio::Application::new(Some(crate::theme::icons::APP_ID), gio::ApplicationFlags::default());
    if app.register(None::<&gio::Cancellable>).is_ok() && app.is_remote() {
        app.activate_action(runner::UPDATE_ACTION, Some(&"manual".to_variant()));
        if let Some(bus) = app.dbus_connection() {
            if let Err(error) = bus.flush_sync(None::<&gio::Cancellable>) {
                eprintln!("kurobayomi: the request may not have reached the running app: {error}");
            }
        }
        return 0;
    }
    // release the name before the pass, so a window opened meanwhile becomes the primary instance
    drop(app);
    runner::run_headless(Trigger::Manual)
}

#[cfg(test)]
/// The two ways a library update runs while no window is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    /// Mode A: the app stays running through the background portal.
    Portal,
    /// Mode B: a user timer starts `kurobayomi --library-update=auto`.
    Timer,
}

#[cfg(test)]
/// Which background mode is on. At most one is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    active: Option<Background>,
}

#[cfg(test)]
impl Selection {
    pub fn active(&self) -> Option<Background> {
        self.active
    }

    /// Turns `want` on. Returns the other mode that was on, which the caller must withdraw.
    pub fn turn_on(&mut self, want: Background) -> Option<Background> {
        self.active.replace(want).filter(|old| *old != want)
    }

    /// Turns `which` off; another active mode is left alone.
    pub fn turn_off(&mut self, which: Background) {
        if self.active == Some(which) {
            self.active = None;
        }
    }
}

#[cfg(test)]
pub const SERVICE_NAME: &str = "kurobayomi-library-update.service";
#[cfg(test)]
pub const TIMER_NAME: &str = "kurobayomi-library-update.timer";
#[cfg(test)]
/// The timer fires up to this long before the interval is over: with jitter, a pass starts in `[N h - 10 min, N h]`.
const JITTER_MINUTES: u32 = 10;

#[cfg(test)]
/// The service the timer starts. `exe` is the absolute path of the app.
pub fn service_unit(exe: &Path) -> String {
    format!(
        "[Unit]\nDescription=Kurobayomi library update\n\n[Service]\nType=oneshot\nExecStart=\"{}\" {OPTION}=auto\nNice=10\nIOSchedulingClass=idle\n",
        exe.display()
    )
}

#[cfg(test)]
/// The timer for an interval of `hours` (at least 1).
pub fn timer_unit(hours: u32) -> String {
    let minutes = hours.max(1) * 60 - JITTER_MINUTES;
    format!(
        "[Unit]\nDescription=Run the Kurobayomi library update every {hours} hours\n\n[Timer]\nOnBootSec=5min\nOnUnitActiveSec={minutes}min\nRandomizedDelaySec={JITTER_MINUTES}min\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n"
    )
}

#[cfg(test)]
fn write_file(path: &Path, text: &str) -> io::Result<()> {
    let part = path.with_extension("part");
    std::fs::write(&part, text)?;
    std::fs::rename(part, path)
}

#[cfg(test)]
/// Writes the two units into `dir` (the user's unit folder). Call only from the settings action the user chose.
pub fn write_units(dir: &Path, exe: &Path, hours: u32) -> io::Result<[PathBuf; 2]> {
    if hours == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "an interval of 0 hours means automatic updates are off"));
    }
    std::fs::create_dir_all(dir)?;
    let (service, timer) = (dir.join(SERVICE_NAME), dir.join(TIMER_NAME));
    write_file(&service, &service_unit(exe))?;
    write_file(&timer, &timer_unit(hours))?;
    Ok([service, timer])
}

#[cfg(test)]
/// Removes the units written by [`write_units`]. A missing file is fine.
pub fn remove_units(dir: &Path) -> io::Result<()> {
    for name in [SERVICE_NAME, TIMER_NAME] {
        match std::fs::remove_file(dir.join(name)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
/// The settings action "run updates with a timer": writes the units and makes mode B the active mode. Returns the mode
/// that was on before and must now be withdrawn, if any.
pub fn enable_timer(selection: &mut Selection, dir: &Path, exe: &Path, hours: u32) -> io::Result<Option<Background>> {
    write_units(dir, exe, hours)?;
    Ok(selection.turn_on(Background::Timer))
}

#[cfg(test)]
/// Turning the timer off removes its units and clears the mode.
pub fn disable_timer(selection: &mut Selection, dir: &Path) -> io::Result<()> {
    remove_units(dir)?;
    selection.turn_off(Background::Timer);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kuro-headless-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_option_names_auto_or_manual_and_nothing_else() {
        // T-26a-020, T-26a-029
        assert_eq!(parse_mode(&args(&["--library-update=auto"])), Ok(Some(Mode::Auto)));
        assert_eq!(parse_mode(&args(&["x.tachibk", "--library-update=manual"])), Ok(Some(Mode::Manual)));
        assert_eq!(parse_mode(&args(&["--run-backup"])), Ok(None));
        assert_eq!(parse_mode(&args(&["--library-update-now"])), Ok(None));
        assert!(parse_mode(&args(&["--library-update=sometimes"])).is_err());
        assert!(parse_mode(&args(&["--library-update="])).is_err());
    }

    #[test]
    fn the_service_runs_the_auto_pass_and_the_timer_keeps_the_jitter_window() {
        // T-26a-020: `N h - 10 min` plus up to 10 min of random delay stays inside `[N h - 10 min, N h]`
        let service = service_unit(Path::new("/usr/bin/kurobayomi"));
        assert!(service.contains("ExecStart=\"/usr/bin/kurobayomi\" --library-update=auto"));
        assert!(service.contains("Type=oneshot"));
        let timer = timer_unit(2);
        assert!(timer.contains("OnUnitActiveSec=110min"));
        assert!(timer.contains("RandomizedDelaySec=10min"));
        assert!(timer.contains("WantedBy=timers.target"));
    }

    #[test]
    fn units_are_written_only_by_the_explicit_call_and_never_for_interval_zero() {
        // T-26a-020
        let dir = scratch("units");
        assert!(!dir.exists(), "nothing is written before the user acts");
        assert!(write_units(&dir, Path::new("/usr/bin/kurobayomi"), 0).is_err());
        assert!(!dir.exists());
        let [service, timer] = write_units(&dir, Path::new("/usr/bin/kurobayomi"), 6).unwrap();
        assert!(std::fs::read_to_string(service).unwrap().contains("--library-update=auto"));
        assert!(std::fs::read_to_string(timer).unwrap().contains("OnUnitActiveSec=350min"));
        remove_units(&dir).unwrap();
        assert!(!dir.join(TIMER_NAME).exists());
        remove_units(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_portal_and_the_timer_are_never_on_together() {
        // T-26a-020: mode A and mode B are mutually exclusive
        let dir = scratch("exclusive");
        let mut selection = Selection::default();
        assert_eq!(selection.turn_on(Background::Portal), None);
        assert_eq!(selection.turn_on(Background::Portal), None, "turning the same mode on again withdraws nothing");
        let withdrawn = enable_timer(&mut selection, &dir, Path::new("/usr/bin/kurobayomi"), 3).unwrap();
        assert_eq!(withdrawn, Some(Background::Portal));
        assert_eq!(selection.active(), Some(Background::Timer));
        assert_eq!(selection.turn_on(Background::Portal), Some(Background::Timer));
        selection.turn_off(Background::Timer);
        assert_eq!(selection.active(), Some(Background::Portal), "turning off the other mode leaves this one on");
        enable_timer(&mut selection, &dir, Path::new("/usr/bin/kurobayomi"), 3).unwrap();
        disable_timer(&mut selection, &dir).unwrap();
        assert_eq!(selection.active(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
