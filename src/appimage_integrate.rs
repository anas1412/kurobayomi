//! The AppImage in the applications menu (user, 2026-10-14). At every start as an AppImage the app adds itself, unless
//! the user took it out with About's switch. Added means its own launcher (`.desktop`, named after the app id so a dock
//! matches the window to it), icons and, for the stable app, its file types are copied into the user's data folder
//! (`~/.local/share`, the place every desktop on Arch, Ubuntu, Mint and Fedora reads), with the launcher pointing at the
//! AppImage where it is; the menu, icon and file-type caches are renewed after a change. The AppImage itself stays where the user put it: a self-update replaces that same file,
//! and every start points the launcher at the file's place again, so a moved AppImage keeps working. Files are written
//! only when they changed. When another tool (Gear Lever, AppImageLauncher) already made a launcher for this AppImage,
//! the app adds none, so the menu does not show it twice.

use std::path::{Path, PathBuf};

use crate::channel::APP_ID;

/// The AppImage's launcher with every `Exec` pointing at `appimage`, and a `TryExec` that hides it once the file is gone.
fn launcher(bundled: &str, appimage: &Path) -> String {
    let exec = exec_quote(&appimage.to_string_lossy());
    let mut out = String::new();
    for line in bundled.lines() {
        match line.strip_prefix("Exec=kurobayomi") {
            Some(rest) => out.push_str(&format!("Exec={exec}{rest}\n")),
            None if line.starts_with("TryExec=") => {}
            None => out.push_str(&format!("{line}\n")),
        }
        if line == "[Desktop Entry]" {
            out.push_str(&format!("TryExec={}\n", appimage.to_string_lossy()));
        }
    }
    out
}

/// A path as one argument of a desktop file's `Exec`: in double quotes, with `"`, `` ` ``, `$` and `\` escaped (the
/// desktop entry spec). A path without any of those or spaces stays as it is.
fn exec_quote(path: &str) -> String {
    if !path.contains(|c: char| c.is_whitespace() || "\"`$\\'><~|&;*?#()".contains(c)) {
        return path.to_owned();
    }
    let mut quoted = String::from("\"");
    for c in path.chars() {
        if "\"`$\\".contains(c) {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

/// Whether a launcher other than `own` in `applications` already starts `appimage` (made by Gear Lever,
/// AppImageLauncher or by hand).
fn launched_elsewhere(applications: &Path, own: &Path, appimage: &Path) -> bool {
    let path = appimage.to_string_lossy();
    std::fs::read_dir(applications)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|file| file != own && file.extension().is_some_and(|e| e == "desktop"))
        .any(|file| std::fs::read_to_string(file).is_ok_and(|text| text.lines().any(|l| l.starts_with("Exec=") && l.contains(path.as_ref()))))
}

/// Writes `contents` to `path` unless it already holds them. `true` when the file changed.
fn write_if_changed(path: &Path, contents: &[u8]) -> std::io::Result<bool> {
    if std::fs::read(path).is_ok_and(|old| old == contents) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(true)
}

/// The files of `dir` and below whose name starts with the app id (the app's icons), relative to `dir`.
fn own_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().is_some_and(|n| n.to_string_lossy().starts_with(APP_ID)) {
                found.extend(path.strip_prefix(dir).ok().map(Path::to_path_buf));
            }
        }
    }
    found
}

/// What changed, so the menu's, the icons' and the file types' caches are renewed only then.
#[derive(Debug, Default, PartialEq, Eq)]
struct Changed {
    launcher: bool,
    icons: bool,
    file_types: bool,
}

/// Puts the AppImage `appimage`, unpacked at `appdir`, in the menu of `data_home` (`~/.local/share`).
fn integrate(appimage: &Path, appdir: &Path, data_home: &Path) -> std::io::Result<Changed> {
    let share = appdir.join("usr/share");
    let applications = data_home.join("applications");
    let own = applications.join(format!("{APP_ID}.desktop"));
    let mut changed = Changed::default();
    if launched_elsewhere(&applications, &own, appimage) {
        // another tool's launcher already starts it: ours would show the app twice
        if own.exists() {
            std::fs::remove_file(&own)?;
            changed.launcher = true;
        }
        return Ok(changed);
    }
    let bundled = std::fs::read_to_string(share.join(format!("applications/{APP_ID}.desktop")))?;
    changed.launcher = write_if_changed(&own, launcher(&bundled, appimage).as_bytes())?;
    let icons = share.join("icons");
    for icon in own_files(&icons) {
        changed.icons |= write_if_changed(&data_home.join("icons").join(&icon), &std::fs::read(icons.join(&icon))?)?;
    }
    let types = share.join(format!("mime/packages/{APP_ID}.xml"));
    if let Ok(xml) = std::fs::read(&types) {
        changed.file_types = write_if_changed(&data_home.join(format!("mime/packages/{APP_ID}.xml")), &xml)?;
    }
    Ok(changed)
}

/// Takes the app's launcher, icons and file types out of `data_home`.
fn unintegrate(data_home: &Path) -> std::io::Result<Changed> {
    let own = data_home.join(format!("applications/{APP_ID}.desktop"));
    let types = data_home.join(format!("mime/packages/{APP_ID}.xml"));
    let icons = data_home.join("icons");
    let own_icons = own_files(&icons);
    let changed = Changed { launcher: own.exists(), icons: !own_icons.is_empty(), file_types: types.exists() };
    for file in [own, types] {
        if file.exists() {
            std::fs::remove_file(file)?;
        }
    }
    for icon in own_icons {
        std::fs::remove_file(icons.join(icon))?;
    }
    Ok(changed)
}

/// Runs a desktop cache tool when it is installed; its absence or failure only means the menu updates a bit later.
fn renew(tool: &str, args: &[&str], dir: &Path) {
    match std::process::Command::new(tool).args(args).arg(dir).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status() {
        Ok(_) => {}
        Err(error) => tracing::debug!(%error, tool, "a desktop cache was not renewed"),
    }
}

/// The AppImage this run started from, where it is unpacked, and the user's data folder. `None` for any other build.
fn context() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let appimage = kuro_platform::self_update::appimage_file()?;
    let appdir = std::env::var_os("APPDIR").map(PathBuf::from)?;
    Some((appimage, appdir, gtk::glib::user_data_dir()))
}

/// Whether this run is an AppImage, so the menu question and the About switch apply.
pub fn available() -> bool {
    context().is_some()
}

/// Whether the app's own launcher is in the menu.
pub fn in_menu() -> bool {
    gtk::glib::user_data_dir().join(format!("applications/{APP_ID}.desktop")).exists()
}

/// Whether another tool's launcher already starts this AppImage (the About switch then says so instead).
pub fn managed_elsewhere() -> bool {
    context().is_some_and(|(appimage, _, data_home)| {
        let applications = data_home.join("applications");
        launched_elsewhere(&applications, &applications.join(format!("{APP_ID}.desktop")), &appimage)
    })
}

/// Renews the menu's and the file types' caches after a change.
fn renew_after(changed: &Changed, data_home: &Path) {
    if changed.launcher {
        renew("update-desktop-database", &[], &data_home.join("applications"));
    }
    if changed.icons {
        // a stale icon cache would hide the new icon; without a cache the folder is read directly
        let hicolor = data_home.join("icons/hicolor");
        if hicolor.join("icon-theme.cache").exists() {
            renew("gtk-update-icon-cache", &["-f", "-t"], &hicolor);
        }
    }
    if changed.file_types {
        renew("update-mime-database", &[], &data_home.join("mime"));
    }
}

/// Adds the AppImage to the applications menu (or brings its launcher up to date), off the GTK thread.
pub fn add() {
    let Some((appimage, appdir, data_home)) = context() else { return };
    std::thread::spawn(move || match integrate(&appimage, &appdir, &data_home) {
        Ok(changed) => renew_after(&changed, &data_home),
        Err(error) => tracing::warn!(%error, "the AppImage could not put itself in the applications menu"),
    });
}

/// Takes the AppImage out of the applications menu, off the GTK thread.
pub fn remove() {
    let data_home = gtk::glib::user_data_dir();
    std::thread::spawn(move || match unintegrate(&data_home) {
        Ok(changed) => renew_after(&changed, &data_home),
        Err(error) => tracing::warn!(%error, "the AppImage could not take itself out of the applications menu"),
    });
}

/// Whether the user took the AppImage out of the applications menu.
fn removed_by_user() -> bool {
    crate::data::is_installed() && crate::data::app().base.appimage_menu_removed.get()
}

/// Remembers About's switch: off keeps the AppImage out of the menu on later starts.
pub fn set_removed_by_user(removed: bool) {
    if crate::data::is_installed()
        && let Err(error) = crate::data::app().base.appimage_menu_removed.set(removed)
    {
        tracing::warn!(%error, "the applications menu choice could not be kept");
    }
}

/// The start of an AppImage: it adds itself to the applications menu, or brings its launcher up to date (a moved
/// AppImage). Any other build, an AppImage another tool manages, or one the user took out of the menu does nothing.
pub fn startup() {
    if available() && !managed_elsewhere() && !removed_by_user() {
        add();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = "[Desktop Entry]\nType=Application\nName=Kurobayomi\nExec=kurobayomi %U\nIcon=x\n\n[Desktop Action show-library]\nName=Manga library\nExec=kurobayomi --action=show-library\n";

    #[test]
    fn the_launcher_starts_the_appimage_where_it_is_and_hides_once_it_is_gone() {
        let text = launcher(BUNDLED, Path::new("/home/u/Apps/Kurobayomi.AppImage"));
        assert!(text.contains("Exec=/home/u/Apps/Kurobayomi.AppImage %U\n"));
        assert!(text.contains("Exec=/home/u/Apps/Kurobayomi.AppImage --action=show-library\n"), "the menu's actions too");
        assert!(text.starts_with("[Desktop Entry]\nTryExec=/home/u/Apps/Kurobayomi.AppImage\n"));
        assert!(!text.contains("Exec=kurobayomi"));
    }

    #[test]
    fn a_path_with_spaces_or_specials_is_quoted_and_escaped() {
        assert_eq!(exec_quote("/home/u/x.AppImage"), "/home/u/x.AppImage");
        assert_eq!(exec_quote("/home/u/My Apps/x.AppImage"), "\"/home/u/My Apps/x.AppImage\"");
        assert_eq!(exec_quote("/home/u/$x\"y"), "\"/home/u/\\$x\\\"y\"");
    }

    /// The whole job on folders: the launcher, the icons and the file types land in the data folder, a second run
    /// changes nothing, and another tool's launcher for the same AppImage makes ours go away.
    #[test]
    fn the_appimage_puts_itself_in_the_menu_once_and_steps_back_for_another_tools_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let (appdir, data) = (dir.path().join("AppDir"), dir.path().join("share"));
        let appimage = dir.path().join("Kurobayomi.AppImage");
        std::fs::write(&appimage, b"\x7fELF").unwrap();
        let share = appdir.join("usr/share");
        std::fs::create_dir_all(share.join("applications")).unwrap();
        std::fs::write(share.join(format!("applications/{APP_ID}.desktop")), BUNDLED).unwrap();
        let icon = format!("hicolor/256x256/apps/{APP_ID}.png");
        std::fs::create_dir_all(share.join("icons/hicolor/256x256/apps")).unwrap();
        std::fs::write(share.join("icons").join(&icon), b"png").unwrap();
        std::fs::write(share.join("icons/hicolor/256x256/apps/other.png"), b"png").unwrap();
        std::fs::create_dir_all(share.join("mime/packages")).unwrap();
        std::fs::write(share.join(format!("mime/packages/{APP_ID}.xml")), b"<mime/>").unwrap();

        assert_eq!(integrate(&appimage, &appdir, &data).unwrap(), Changed { launcher: true, icons: true, file_types: true });
        let own = data.join(format!("applications/{APP_ID}.desktop"));
        assert!(std::fs::read_to_string(&own).unwrap().contains(&format!("Exec={} %U", appimage.display())));
        assert!(data.join("icons").join(&icon).exists() && !data.join("icons/hicolor/256x256/apps/other.png").exists(), "only the app's icons");
        assert_eq!(integrate(&appimage, &appdir, &data).unwrap(), Changed::default(), "nothing changed, nothing written");

        std::fs::write(data.join("applications/appimagekit-kurobayomi.desktop"), format!("[Desktop Entry]\nExec={} %U\n", appimage.display())).unwrap();
        assert_eq!(integrate(&appimage, &appdir, &data).unwrap().launcher, true);
        assert!(!own.exists(), "the other tool's launcher stays alone");

        std::fs::remove_file(data.join("applications/appimagekit-kurobayomi.desktop")).unwrap();
        integrate(&appimage, &appdir, &data).unwrap();
        assert_eq!(unintegrate(&data).unwrap(), Changed { launcher: true, icons: true, file_types: true });
        assert!(!own.exists() && !data.join("icons").join(&icon).exists(), "taken out of the menu");
        assert!(!data.join(format!("mime/packages/{APP_ID}.xml")).exists());
        assert_eq!(unintegrate(&data).unwrap(), Changed::default(), "nothing left to remove");
    }
}
