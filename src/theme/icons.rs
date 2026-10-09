//! Icon names (`Icon`), the bundled icon theme registration, and the state-to-icon mappings for the
//! anime library tab, the entry caret, the reader modes and the download indicator (T-49ba-009,
//! T-49ba-012, T-49ba-015, T-49ba-019, T-49ba-025, T-49ba-026).
//!
//! Standard names come from the user's icon theme. The `kb-*` names are symbolic SVGs exported by
//! `gen/export_icons.py` into `icons/` and bundled as a GResource.

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

/// The application id, also the icon name of the full-colour app icon.
pub const APP_ID: &str = "io.github.blackbox.Kurobayomi";

/// The bundled icon theme root: `scalable/actions/`, `scalable/apps/` and `symbolic/apps/` below it.
pub const ICON_RESOURCE_PATH: &str = "/io/github/blackbox/Kurobayomi/icons";

/// One icon the app shows. `name()` is the icon-theme name looked up at run time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Close,
    Play,
    Pause,
    SkipNext,
    SkipPrevious,
    Refresh,
    Share,
    Done,
    Info,
    Warning,
    Book,
    Folder,
    Extension,
    Photo,
    Server,
    UpdateAvailable,
    Calendar,
    DragHandle,
    CaretDown,
    CaretUp,
    MoreHorizontal,
    GoBack,
    HistoryTab,
    Cover,
    AnimeLibraryOutline,
    AnimeLibraryFilled,
    LibraryManga,
    UpdatesTab,
    Compass,
    DownloadCircle,
    DoneMarkPrevious,
    OverflowVertical,
    Glasses,
    ReaderDefault,
    ReaderLtr,
    ReaderRtl,
    ReaderVertical,
    ReaderContinuousVertical,
    ReaderWebtoon,
    CropOn,
    CropOff,
    SeekTriangle,
    Forward10,
    TextShadow,
    FontFamily,
    Logo,
    LogoSymbolic,
}

impl Icon {
    /// Every variant, in declaration order.
    pub const ALL: [Icon; 47] = [
        Icon::Close,
        Icon::Play,
        Icon::Pause,
        Icon::SkipNext,
        Icon::SkipPrevious,
        Icon::Refresh,
        Icon::Share,
        Icon::Done,
        Icon::Info,
        Icon::Warning,
        Icon::Book,
        Icon::Folder,
        Icon::Extension,
        Icon::Photo,
        Icon::Server,
        Icon::UpdateAvailable,
        Icon::Calendar,
        Icon::DragHandle,
        Icon::CaretDown,
        Icon::CaretUp,
        Icon::MoreHorizontal,
        Icon::GoBack,
        Icon::HistoryTab,
        Icon::Cover,
        Icon::AnimeLibraryOutline,
        Icon::AnimeLibraryFilled,
        Icon::LibraryManga,
        Icon::UpdatesTab,
        Icon::Compass,
        Icon::DownloadCircle,
        Icon::DoneMarkPrevious,
        Icon::OverflowVertical,
        Icon::Glasses,
        Icon::ReaderDefault,
        Icon::ReaderLtr,
        Icon::ReaderRtl,
        Icon::ReaderVertical,
        Icon::ReaderContinuousVertical,
        Icon::ReaderWebtoon,
        Icon::CropOn,
        Icon::CropOff,
        Icon::SeekTriangle,
        Icon::Forward10,
        Icon::TextShadow,
        Icon::FontFamily,
        Icon::Logo,
        Icon::LogoSymbolic,
    ];

    /// The icon-theme name of this icon.
    pub const fn name(self) -> &'static str {
        match self {
            Icon::Close => "window-close-symbolic",
            Icon::Play => "media-playback-start-symbolic",
            Icon::Pause => "media-playback-pause-symbolic",
            Icon::SkipNext => "media-skip-forward-symbolic",
            Icon::SkipPrevious => "media-skip-backward-symbolic",
            Icon::Refresh => "view-refresh-symbolic",
            Icon::Share => "document-send-symbolic",
            Icon::Done => "object-select-symbolic",
            Icon::Info => "dialog-information-symbolic",
            Icon::Warning => "dialog-warning-symbolic",
            Icon::Book => "accessories-dictionary-symbolic",
            Icon::Folder => "folder-symbolic",
            Icon::Extension => "application-x-addon-symbolic",
            Icon::Photo => "image-x-generic-symbolic",
            Icon::Server => "network-server-symbolic",
            Icon::UpdateAvailable => "software-update-available-symbolic",
            Icon::Calendar => "x-office-calendar-symbolic",
            Icon::DragHandle => "list-drag-handle-symbolic",
            Icon::CaretDown => "pan-down-symbolic",
            Icon::CaretUp => "pan-up-symbolic",
            Icon::MoreHorizontal => "view-more-horizontal-symbolic",
            Icon::GoBack => "go-previous-symbolic",
            Icon::HistoryTab => "document-open-recent-symbolic",
            Icon::Cover => "image-x-generic-symbolic",
            Icon::AnimeLibraryOutline => "kb-animelibrary-outline-symbolic",
            Icon::AnimeLibraryFilled => "kb-animelibrary-filled-symbolic",
            Icon::LibraryManga => "kb-library-symbolic",
            Icon::UpdatesTab => "kb-updates-outline-symbolic",
            Icon::Compass => "kb-compass-symbolic",
            Icon::DownloadCircle => "kb-download-circle-symbolic",
            Icon::DoneMarkPrevious => "kb-done-previous-symbolic",
            Icon::OverflowVertical => "kb-overflow-vertical-symbolic",
            Icon::Glasses => "kb-glasses-symbolic",
            Icon::ReaderDefault => "kb-reader-default-symbolic",
            Icon::ReaderLtr => "kb-reader-ltr-symbolic",
            Icon::ReaderRtl => "kb-reader-rtl-symbolic",
            Icon::ReaderVertical => "kb-reader-vertical-symbolic",
            Icon::ReaderContinuousVertical => "kb-reader-continuous-vertical-symbolic",
            Icon::ReaderWebtoon => "kb-reader-webtoon-symbolic",
            Icon::CropOn => "kb-crop-symbolic",
            Icon::CropOff => "kb-crop-off-symbolic",
            Icon::SeekTriangle => "kb-seek-triangle-symbolic",
            Icon::Forward10 => "kb-forward-10-symbolic",
            Icon::TextShadow => "kb-text-shadow-symbolic",
            Icon::FontFamily => "kb-font-family-symbolic",
            Icon::Logo => APP_ID,
            Icon::LogoSymbolic => "io.github.blackbox.Kurobayomi-symbolic",
        }
    }

    /// The file this icon is bundled as, relative to the icon theme root, or `None` for a standard
    /// name that the user's icon theme provides.
    pub const fn bundled_path(self) -> Option<&'static str> {
        Some(match self {
            Icon::Logo => "scalable/apps/io.github.blackbox.Kurobayomi.svg",
            Icon::LogoSymbolic => "symbolic/apps/io.github.blackbox.Kurobayomi-symbolic.svg",
            Icon::AnimeLibraryOutline => "scalable/actions/kb-animelibrary-outline-symbolic.svg",
            Icon::AnimeLibraryFilled => "scalable/actions/kb-animelibrary-filled-symbolic.svg",
            Icon::LibraryManga => "scalable/actions/kb-library-symbolic.svg",
            Icon::UpdatesTab => "scalable/actions/kb-updates-outline-symbolic.svg",
            Icon::Compass => "scalable/actions/kb-compass-symbolic.svg",
            Icon::DownloadCircle => "scalable/actions/kb-download-circle-symbolic.svg",
            Icon::DoneMarkPrevious => "scalable/actions/kb-done-previous-symbolic.svg",
            Icon::OverflowVertical => "scalable/actions/kb-overflow-vertical-symbolic.svg",
            Icon::Glasses => "scalable/actions/kb-glasses-symbolic.svg",
            Icon::ReaderDefault => "scalable/actions/kb-reader-default-symbolic.svg",
            Icon::ReaderLtr => "scalable/actions/kb-reader-ltr-symbolic.svg",
            Icon::ReaderRtl => "scalable/actions/kb-reader-rtl-symbolic.svg",
            Icon::ReaderVertical => "scalable/actions/kb-reader-vertical-symbolic.svg",
            Icon::ReaderContinuousVertical => "scalable/actions/kb-reader-continuous-vertical-symbolic.svg",
            Icon::ReaderWebtoon => "scalable/actions/kb-reader-webtoon-symbolic.svg",
            Icon::CropOn => "scalable/actions/kb-crop-symbolic.svg",
            Icon::CropOff => "scalable/actions/kb-crop-off-symbolic.svg",
            Icon::SeekTriangle => "scalable/actions/kb-seek-triangle-symbolic.svg",
            Icon::Forward10 => "scalable/actions/kb-forward-10-symbolic.svg",
            Icon::TextShadow => "scalable/actions/kb-text-shadow-symbolic.svg",
            Icon::FontFamily => "scalable/actions/kb-font-family-symbolic.svg",
            _ => return None,
        })
    }
}

/// The icon bundle compiled by `build.rs`.
static BUNDLE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kurobayomi.gresource"));

/// Adds the bundled icon theme to the default display, so the `kb-*` names resolve. Must run before
/// the first window is shown. A missing bundle logs one error and the app keeps the user's icons.
pub fn register(display: &gtk::gdk::Display) {
    match gtk::gio::Resource::from_data(&gtk::glib::Bytes::from_static(BUNDLE)) {
        Ok(bundle) => gtk::gio::resources_register(&bundle),
        Err(e) => tracing::error!(error = %e, "the icon bundle is unreadable"),
    }
    let sentinel = format!("{ICON_RESOURCE_PATH}/scalable/actions/kb-library-symbolic.svg");
    if gtk::gio::resources_lookup_data(&sentinel, gtk::gio::ResourceLookupFlags::NONE).is_err() {
        tracing::error!(path = ICON_RESOURCE_PATH, "bundled icons are missing; the app keeps the system icons");
        return;
    }
    gtk::IconTheme::for_display(display).add_resource_path(ICON_RESOURCE_PATH);
}

/// The anime library tab icon: outline when not selected, filled when selected (T-49ba-015).
pub fn anime_library_icon(selected: bool) -> Icon {
    if selected { Icon::AnimeLibraryFilled } else { Icon::AnimeLibraryOutline }
}

/// The entry info header caret: up when expanded, down when collapsed (T-49ba-019).
pub fn caret_icon(expanded: bool) -> Icon {
    if expanded { Icon::CaretUp } else { Icon::CaretDown }
}

/// The caret tooltip: the collapse label when expanded, the expand label otherwise.
pub fn caret_tooltip(expanded: bool) -> String {
    tr(if expanded { MsgKey::manga_info_collapse } else { MsgKey::manga_info_expand })
}

/// The reading-mode icon for a `ReadingMode` constant name, as in `ReadingMode.kt`. An unknown
/// name is `DEFAULT`, as `ReadingMode.fromPreference` does (T-49ba-025).
pub fn reading_mode_icon(mode: &str) -> Icon {
    match mode {
        "LEFT_TO_RIGHT" => Icon::ReaderLtr,
        "RIGHT_TO_LEFT" => Icon::ReaderRtl,
        "VERTICAL" => Icon::ReaderVertical,
        "WEBTOON" => Icon::ReaderWebtoon,
        "CONTINUOUS_VERTICAL" => Icon::ReaderContinuousVertical,
        _ => Icon::ReaderDefault,
    }
}

/// The download indicator icon: shown for a downloaded row, nothing otherwise (T-49ba-026).
pub fn download_indicator_icon(downloaded: bool) -> Option<Icon> {
    downloaded.then_some(Icon::DownloadCircle)
}

/// The download indicator widget. It is not clickable and carries the "Downloaded" tooltip.
pub fn download_indicator(downloaded: bool) -> Option<gtk::Image> {
    let icon = download_indicator_icon(downloaded)?;
    let image = gtk::Image::from_icon_name(icon.name());
    image.set_tooltip_text(Some(&tr(MsgKey::label_downloaded)));
    image.set_can_target(false);
    Some(image)
}

/// The reader mode image. LTR and RTL icons are never mirrored, so the widget is set to LTR (I3).
pub fn reading_mode_image(mode: &str) -> gtk::Image {
    let icon = reading_mode_icon(mode);
    let image = gtk::Image::from_icon_name(icon.name());
    image.set_direction(gtk::TextDirection::Ltr);
    image
}

/// Exposes the selected state of a toggle widget to assistive technology.
pub fn set_toggle_state(widget: &impl IsA<gtk::Accessible>, selected: bool) {
    let tristate = if selected { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False };
    widget.update_state(&[gtk::accessible::State::Checked(tristate)]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A file under the bundled icon theme, by its path relative to the theme root.
    fn icon_file(relative: &str) -> PathBuf {
        PathBuf::from(format!("{}/src/theme/icons/{relative}", env!("CARGO_MANIFEST_DIR")))
    }

    #[test]
    fn names_match_the_table() {
        assert_eq!(Icon::Close.name(), "window-close-symbolic");
        assert_eq!(Icon::Pause.name(), "media-playback-pause-symbolic");
        assert_eq!(Icon::SkipPrevious.name(), "media-skip-backward-symbolic");
        assert_eq!(Icon::Warning.name(), "dialog-warning-symbolic");
        assert_eq!(Icon::CaretUp.name(), "pan-up-symbolic");
        assert_eq!(Icon::DownloadCircle.name(), "kb-download-circle-symbolic");
        assert_eq!(Icon::ReaderContinuousVertical.name(), "kb-reader-continuous-vertical-symbolic");
        assert_eq!(Icon::Logo.name(), APP_ID);
        assert_eq!(Icon::LogoSymbolic.name(), "io.github.blackbox.Kurobayomi-symbolic");
    }

    #[test]
    fn every_variant_is_listed_once() {
        for (i, icon) in Icon::ALL.iter().enumerate() {
            assert!(!Icon::ALL[..i].contains(icon), "{icon:?} listed twice");
        }
        assert_eq!(Icon::ALL.len(), 47);
    }

    #[test]
    fn every_bundled_icon_has_its_file() {
        for icon in Icon::ALL {
            if let Some(path) = icon.bundled_path() {
                assert!(icon_file(path).is_file(), "{icon:?} missing {path}");
            }
        }
    }

    #[test]
    fn every_kb_name_is_bundled() {
        for icon in Icon::ALL {
            let name = icon.name();
            if name.starts_with("kb-") {
                assert!(icon.bundled_path().is_some(), "{name} has no bundled file");
            }
        }
    }

    /// T-49ba-009 (T2): every icon resolves after `register`. Needs a display and the GResource
    /// bundle: `cargo test -p kurobayomi every_icon_resolves -- --ignored`.
    #[test]
    #[ignore = "needs a display and the icon bundle; run alone: cargo test -p kurobayomi every_icon_resolves -- --ignored"]
    fn every_icon_resolves_after_register() {
        if gtk::init().is_err() {
            eprintln!("no display: skipping");
            return;
        }
        let display = gtk::gdk::Display::default().expect("a display");
        register(&display);
        let theme = gtk::IconTheme::for_display(&display);
        for icon in Icon::ALL {
            assert!(theme.has_icon(icon.name()), "{icon:?} ({}) does not resolve", icon.name());
        }
    }

    #[test]
    fn standard_names_are_not_bundled() {
        for icon in [Icon::Close, Icon::Play, Icon::Warning, Icon::CaretDown, Icon::Cover] {
            assert_eq!(icon.bundled_path(), None);
        }
    }

    #[test]
    fn anime_library_follows_the_selection() {
        assert_eq!(anime_library_icon(false), Icon::AnimeLibraryOutline);
        assert_eq!(anime_library_icon(true), Icon::AnimeLibraryFilled);
    }

    #[test]
    fn caret_follows_the_expansion() {
        assert_eq!(caret_icon(true), Icon::CaretUp);
        assert_eq!(caret_icon(false), Icon::CaretDown);
        assert_eq!(caret_tooltip(true), "Less");
        assert_eq!(caret_tooltip(false), "More");
    }

    #[test]
    fn reading_modes_map_to_their_icons() {
        assert_eq!(reading_mode_icon("DEFAULT"), Icon::ReaderDefault);
        assert_eq!(reading_mode_icon("LEFT_TO_RIGHT"), Icon::ReaderLtr);
        assert_eq!(reading_mode_icon("RIGHT_TO_LEFT"), Icon::ReaderRtl);
        assert_eq!(reading_mode_icon("VERTICAL"), Icon::ReaderVertical);
        assert_eq!(reading_mode_icon("WEBTOON"), Icon::ReaderWebtoon);
        assert_eq!(reading_mode_icon("CONTINUOUS_VERTICAL"), Icon::ReaderContinuousVertical);
        assert_eq!(reading_mode_icon("nonsense"), Icon::ReaderDefault);
    }

    #[test]
    fn download_indicator_only_for_downloaded_rows() {
        assert_eq!(download_indicator_icon(true), Some(Icon::DownloadCircle));
        assert_eq!(download_indicator_icon(false), None);
    }

    #[test]
    fn app_icon_is_full_colour_and_symbolic_is_separate() {
        assert_eq!(Icon::Logo.bundled_path(), Some("scalable/apps/io.github.blackbox.Kurobayomi.svg"));
        assert_eq!(
            Icon::LogoSymbolic.bundled_path(),
            Some("symbolic/apps/io.github.blackbox.Kurobayomi-symbolic.svg")
        );
    }

    fn svg_files(relative: &str) -> Vec<std::path::PathBuf> {
        let mut files: Vec<_> = std::fs::read_dir(icon_file(relative))
            .expect("icon directory")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "svg"))
            .collect();
        files.sort();
        files
    }

    /// T7: every symbolic export is filled with `#bebebe` and has no white fill.
    #[test]
    fn symbolic_exports_use_the_symbolic_fill_only() {
        let mut files = svg_files("scalable/actions");
        files.extend(svg_files("symbolic/apps"));
        assert!(files.len() >= 21, "found {} symbolic icons", files.len());
        for path in files {
            let text = std::fs::read_to_string(&path).expect("svg");
            assert!(text.contains("#bebebe"), "{} has no #bebebe fill", path.display());
            assert!(!text.contains("#FFF\""), "{} has a #FFF fill", path.display());
            assert!(!text.contains("#FFFFFF"), "{} has a #FFFFFF fill", path.display());
            assert!(text.contains("viewBox=\"0 0 "), "{} has no viewBox", path.display());
        }
    }

    /// T-49ba-017: the full-colour app icon keeps the launcher colours and has no background tile.
    #[test]
    fn app_icon_keeps_the_launcher_colours() {
        let text = std::fs::read_to_string(icon_file("scalable/apps/io.github.blackbox.Kurobayomi.svg")).expect("svg");
        for colour in ["#000", "#2E84BF", "#69A3CB", "#C92824", "#FFFFFF"] {
            assert!(text.contains(&format!("fill=\"{colour}\"")), "missing {colour}");
        }
        assert!(!text.contains("#bebebe"));
    }
}
