//! Source icon resolution (T-40a-006). The choice is cached per source id, and a missing icon never blocks a row.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Icons are 40 px high and square (`height(40) aspectRatio(1)`).
pub const ICON_HEIGHT_PX: i32 = 40;

/// The icon of the details page header is 112 px square (T-40a-021, T-40a-027).
pub const DETAILS_ICON_PX: i32 = 112;

/// Where the picture of an extension comes from: an available extension names the url of its icon, an installed one
/// the package or image file it ships. Both travel in `Ext::icon_file`; the scheme tells them apart (T-40a-027).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconRef<'a> {
    Url(&'a str),
    File(&'a str),
}

pub fn icon_ref(value: &str) -> IconRef<'_> {
    if value.starts_with("https://") || value.starts_with("http://") { IconRef::Url(value) } else { IconRef::File(value) }
}

/// The file an icon url is cached in: the image cache's own name for a key, the md5 of the url (T-34a-068).
pub fn icon_cache_file(root: &Path, url: &str) -> PathBuf {
    root.join(kuro_images::keys::hash_key(url))
}

/// The image cache folder, resolved from the platform data directory. `None` when there is no home to put it in.
pub fn icon_cache_root() -> Option<PathBuf> {
    let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
    Some(kuro_images::platform::image_cache_dir(&paths))
}

/// The picture of an available extension: the cached file when the url was fetched before, else what `get` downloads,
/// which is then cached. A fetch that fails, or bytes that are not a picture, are `None` and the row shows its error
/// glyph. Blocking: callers run it off the GTK thread.
pub fn fetch_icon(root: Option<&Path>, url: &str, get: impl FnOnce(&str) -> Option<Vec<u8>>) -> Option<DecodedIcon> {
    let cached = root.map(|root| icon_cache_file(root, url));
    if let Some(bytes) = cached.as_ref().and_then(|file| std::fs::read(file).ok()) {
        if let Some(icon) = decode_icon(&bytes) {
            return Some(icon);
        }
    }
    let bytes = get(url)?;
    let icon = decode_icon(&bytes)?;
    if let Some(file) = cached {
        // written beside the entry and renamed in, so a reader never sees half a file
        let part = file.with_extension("part");
        let stored = std::fs::create_dir_all(file.parent().unwrap_or(Path::new("."))).and_then(|()| std::fs::write(&part, &bytes)).and_then(|()| std::fs::rename(&part, &file));
        if let Err(error) = stored {
            tracing::debug!(%error, "an extension icon could not be cached");
        }
    }
    Some(icon)
}

/// The glyph a failed icon leaves: a url that could not be fetched shows the error glyph in the error colour (the bundled
/// error placeholder of D7); a file that could not be read shows the default source glyph.
pub fn fallback_glyph(from_url: bool) -> (&'static str, bool) {
    if from_url { ("dialog-error-symbolic", true) } else { (SourceIcon::Default.icon_name().unwrap_or("image-missing"), false) }
}

/// Downloads an icon with a short timeout.
pub fn http_get(url: &str) -> Option<Vec<u8>> {
    let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(15)).build().ok()?;
    let response = client.get(url).send().ok()?.error_for_status().ok()?;
    response.bytes().ok().map(|bytes| bytes.to_vec())
}

/// Which icon a source row shows. T-10a-034 decision: the provider is this enum plus the loader in `view::icon_image_sized`
/// (a custom icon is decoded off the GTK thread and set as a texture there); the built-in kinds are named theme icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceIcon {
    /// A stub with no icon: the warning glyph in the error colour.
    Warning,
    /// The icon the extension ships with, loaded from its file.
    Custom,
    /// The local source shows the application icon (`ic_local_source` in Aniyomi).
    Local,
    /// The generic source icon.
    Default,
    /// A source with no icon of its own: a tile with the first letter of its name (`look::row_cover`).
    Letter,
    /// An untrusted extension: the dangerous glyph in the error colour.
    Untrusted,
}

impl SourceIcon {
    /// The warning and dangerous glyphs are tinted with the error colour (T-41a-004).
    pub fn is_error(self) -> bool {
        matches!(self, SourceIcon::Warning | SourceIcon::Untrusted)
    }

    /// The GTK symbolic name for the built-in icons. A custom icon comes from its file, so it has none.
    pub fn icon_name(self) -> Option<&'static str> {
        match self {
            SourceIcon::Warning => Some("dialog-warning-symbolic"),
            SourceIcon::Custom => None,
            SourceIcon::Local => Some(crate::theme::icons::APP_ID),
            SourceIcon::Default => Some("application-x-addon-symbolic"),
            SourceIcon::Letter => None,
            SourceIcon::Untrusted => Some("security-low-symbolic"),
        }
    }
}

/// An untrusted extension shows the dangerous glyph before anything else (T-41a-004).
pub fn resolve_for(untrusted: bool, is_stub: bool, has_custom_icon: bool, is_local: bool) -> SourceIcon {
    if untrusted { SourceIcon::Untrusted } else { resolve(is_stub, has_custom_icon, is_local) }
}

/// The order: a stub without an icon, then a custom icon, then the local source, then the letter tile.
pub fn resolve(is_stub: bool, has_custom_icon: bool, is_local: bool) -> SourceIcon {
    if is_stub && !has_custom_icon {
        SourceIcon::Warning
    } else if has_custom_icon {
        SourceIcon::Custom
    } else if is_local {
        SourceIcon::Local
    } else {
        SourceIcon::Letter
    }
}

/// The fill shown while an installed icon decodes, and when it cannot be read (`0x1F888888`, ARGB).
pub const PLACEHOLDER_ARGB: u32 = 0x1F88_8888;

/// The placeholder as the bytes of one RGBA pixel.
pub fn placeholder_rgba() -> [u8; 4] {
    let [a, r, g, b] = PLACEHOLDER_ARGB.to_be_bytes();
    [r, g, b, a]
}

/// A decoded icon: straight RGBA, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Reads the icon of an installed extension and decodes it. `path` is the extension package (its `icon.png` entry is
/// read) or a plain image file. Any failure is `None`, and the row then keeps the default icon. Blocking: callers run
/// it off the GTK thread.
pub fn load_icon(path: &Path) -> Option<DecodedIcon> {
    let bytes = kuro_extensions::anime::package::read_icon(path).or_else(|| std::fs::read(path).ok())?;
    decode_icon(&bytes)
}

/// Decodes PNG, JPEG, GIF or WebP bytes to RGBA. `None` for anything else.
pub fn decode_icon(bytes: &[u8]) -> Option<DecodedIcon> {
    let rgba = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = rgba.dimensions();
    (width > 0 && height > 0).then(|| DecodedIcon { width, height, rgba: rgba.into_raw() })
}

/// The cached `icon.png` of a manga package, as the path the list loads. `None` when the package has none or has vanished,
/// which the row draws as the generic glyph (T-18a-011).
pub fn cached_icon_file(icons: &Path, pkg: &str) -> Option<String> {
    kuro_extensions::icon::lookup(icons, pkg).map(|path| path.to_string_lossy().into_owned())
}

/// Icon choices keyed by source id.
#[derive(Debug, Default)]
pub struct IconCache {
    by_id: HashMap<i64, SourceIcon>,
}

impl IconCache {
    /// The icon of a source, resolved once per id. An untrusted extension gets the dangerous glyph first.
    pub fn get(&mut self, id: i64, untrusted: bool, is_stub: bool, has_custom_icon: bool, is_local: bool) -> SourceIcon {
        *self.by_id.entry(id).or_insert_with(|| resolve_for(untrusted, is_stub, has_custom_icon, is_local))
    }

    #[cfg(test)]
    /// Forgets one source's icon, for example after its extension was updated.
    pub fn invalidate(&mut self, id: i64) {
        self.by_id.remove(&id);
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stub_without_an_icon_shows_the_warning_glyph() {
        assert_eq!(resolve(true, false, false), SourceIcon::Warning);
        assert_eq!(resolve(true, false, true), SourceIcon::Warning);
    }

    #[test]
    fn a_custom_icon_wins_over_local_and_default() {
        assert_eq!(resolve(false, true, true), SourceIcon::Custom);
        assert_eq!(resolve(true, true, false), SourceIcon::Custom, "a stub with an icon shows it");
    }

    #[test]
    fn local_then_letter_when_nothing_else_applies() {
        assert_eq!(resolve(false, false, true), SourceIcon::Local);
        assert_eq!(resolve(false, false, false), SourceIcon::Letter, "a source with no icon of its own gets a letter tile");
    }

    #[test]
    fn built_in_icons_have_names_and_custom_icons_do_not() {
        assert_eq!(SourceIcon::Custom.icon_name(), None);
        assert!(SourceIcon::Warning.icon_name().is_some());
        assert_eq!(SourceIcon::Local.icon_name(), Some(crate::theme::icons::APP_ID), "the local source shows the app icon");
        assert!(SourceIcon::Default.icon_name().is_some());
        assert_eq!(SourceIcon::Letter.icon_name(), None, "the letter tile is drawn, not looked up");
        assert_eq!(ICON_HEIGHT_PX, 40);
    }

    #[test]
    fn an_untrusted_extension_shows_the_dangerous_glyph_before_anything_else() {
        assert_eq!(resolve_for(true, false, true, false), SourceIcon::Untrusted);
        assert_eq!(resolve_for(true, true, false, true), SourceIcon::Untrusted);
        assert_eq!(resolve_for(false, true, false, false), SourceIcon::Warning);
        assert_eq!(SourceIcon::Untrusted.icon_name(), Some("security-low-symbolic"));
    }

    #[test]
    fn the_cache_resolves_once_per_id_and_can_be_invalidated() {
        let mut cache = IconCache::default();
        assert_eq!(cache.get(7, false, false, false, false), SourceIcon::Letter);
        // A second lookup with different inputs keeps the first answer until the id is invalidated.
        assert_eq!(cache.get(7, false, true, false, false), SourceIcon::Letter);
        assert_eq!(cache.len(), 1);
        cache.invalidate(7);
        assert!(cache.is_empty());
        assert_eq!(cache.get(7, false, true, false, false), SourceIcon::Warning);
        assert_eq!(cache.get(8, true, false, true, false), SourceIcon::Untrusted, "the cache goes through resolve_for");
    }

    #[test]
    fn the_placeholder_is_a_faint_grey_and_unreadable_icons_decode_to_nothing() {
        assert_eq!(placeholder_rgba(), [0x88, 0x88, 0x88, 0x1F]);
        assert_eq!(decode_icon(b"not an image"), None);
        assert_eq!(load_icon(Path::new("/nonexistent/icon.png")), None);
    }

    #[test]
    fn a_png_file_and_a_package_icon_decode_to_rgba() {
        use std::io::Write;
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode");
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("icon.png");
        std::fs::File::create(&file).expect("file").write_all(&png).expect("write");
        let icon = load_icon(&file).expect("decoded");
        assert_eq!((icon.width, icon.height), (2, 3));
        assert_eq!(&icon.rgba[..4], &[1, 2, 3, 255]);
    }

    #[test]
    fn a_source_without_a_saved_repo_icon_has_no_picture_and_keeps_the_letter_tile() {
        assert_eq!(crate::recipe_sources::icon_path(-1), None, "no repo source has this id");
        assert_eq!(resolve(false, false, false), SourceIcon::Letter);
    }

    #[test]
    fn an_http_value_is_a_url_and_anything_else_is_a_file() {
        assert_eq!(icon_ref("https://x.test/icon.png"), IconRef::Url("https://x.test/icon.png"));
        assert_eq!(icon_ref("http://x.test/icon.png"), IconRef::Url("http://x.test/icon.png"));
        assert_eq!(icon_ref("/cache/icons/pkg.png"), IconRef::File("/cache/icons/pkg.png"));
        assert_eq!(DETAILS_ICON_PX, 112);
    }

    fn png() -> Vec<u8> {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 2, image::Rgba([9, 8, 7, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode");
        png
    }

    #[test]
    fn an_icon_url_is_fetched_once_and_then_read_from_the_cache() {
        let dir = tempfile::tempdir().expect("dir");
        let url = "https://x.test/icon.png";
        let bytes = png();
        let first = fetch_icon(Some(dir.path()), url, |_| Some(bytes.clone())).expect("fetched");
        assert_eq!((first.width, first.height), (2, 2));
        assert!(icon_cache_file(dir.path(), url).exists(), "the picture is cached under the md5 of its url");
        let second = fetch_icon(Some(dir.path()), url, |_| panic!("a cached icon is not fetched again")).expect("cached");
        assert_eq!(second, first);
    }

    #[test]
    fn a_failed_fetch_or_a_bad_picture_is_none_and_caches_nothing() {
        let dir = tempfile::tempdir().expect("dir");
        assert_eq!(fetch_icon(Some(dir.path()), "https://x.test/a.png", |_| None), None, "the network failed");
        assert_eq!(fetch_icon(Some(dir.path()), "https://x.test/b.png", |_| Some(b"<html>".to_vec())), None, "not a picture");
        assert_eq!(std::fs::read_dir(dir.path()).expect("dir").count(), 0);
        assert_eq!(fetch_icon(None, "https://x.test/c.png", |_| Some(png())).map(|i| i.width), Some(2), "with no cache folder it still shows");
    }
}
