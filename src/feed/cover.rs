//! Cover thumbnails for the widgets (T-50a-013, T-50a-014). The app renders a rounded 58:87 thumbnail per cover into
//! the widget cover cache; the CLI only looks the file up and never downloads, decodes or writes (T-50a-016).

use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
use image::{DynamicImage, RgbaImage, imageops::FilterType};

use super::UpdateRow;

#[cfg(test)]
pub const COVER_W_DP: f64 = 58.0;
#[cfg(test)]
pub const COVER_H_DP: f64 = 87.0;
#[cfg(test)]
pub const INNER_RADIUS_DP: f64 = 12.0;

/// File stem of a cover's thumbnail: SHA-256 of `<thumbnail url>|<cover last modified>`, so a changed cover is
/// rendered again.
pub fn cache_key(url: &str, last_modified: i64) -> String {
    kuro_core::util::hash::sha256_hex(format!("{url}|{last_modified}").as_bytes())
}

fn file_name(key: &str) -> String {
    format!("{key}.png")
}

/// Path of the cached thumbnail of a row, or `None` (placeholder) when it has no cover URL or no file yet.
pub fn cached_path(cache_dir: &Path, row: &UpdateRow) -> Option<String> {
    let url = row.thumbnail_url.as_deref().filter(|u| !u.is_empty())?;
    let path = cache_dir.join(file_name(&cache_key(url, row.cover_last_modified)));
    path.is_file().then(|| path.to_str().map(str::to_owned)).flatten()
}

#[cfg(test)]
/// Pixel size of a thumbnail at a device scale.
pub fn thumb_size(scale: u32) -> (u32, u32) {
    ((COVER_W_DP * f64::from(scale)).round() as u32, (COVER_H_DP * f64::from(scale)).round() as u32)
}

#[cfg(test)]
/// Centre-crops to the cover aspect, resizes, and bakes a rounded alpha mask of radius 12 x `scale`.
pub fn render(src: &DynamicImage, scale: u32) -> RgbaImage {
    let (w, h) = thumb_size(scale);
    let mut out = src.resize_to_fill(w, h, FilterType::Lanczos3).to_rgba8();
    let r = INNER_RADIUS_DP * f64::from(scale);
    for (x, y, px) in out.enumerate_pixels_mut() {
        let (cx, cy) = (
            if f64::from(x) + 0.5 < r { r } else if f64::from(x) + 0.5 > f64::from(w) - r { f64::from(w) - r } else { f64::from(x) + 0.5 },
            if f64::from(y) + 0.5 < r { r } else if f64::from(y) + 0.5 > f64::from(h) - r { f64::from(h) - r } else { f64::from(y) + 0.5 },
        );
        let dist = ((f64::from(x) + 0.5 - cx).powi(2) + (f64::from(y) + 0.5 - cy).powi(2)).sqrt();
        let coverage = (r - dist + 0.5).clamp(0.0, 1.0);
        px.0[3] = (f64::from(px.0[3]) * coverage).round() as u8;
    }
    out
}

#[cfg(test)]
/// Where a local cover lives: a `file://` URL. Other schemes (`content://`, `http`) have no local file for the
/// renderer, so those rows keep the placeholder (E-27).
pub fn local_source(url: &str) -> Option<PathBuf> {
    url.strip_prefix("file://").filter(|p| p.starts_with('/')).map(PathBuf::from)
}

#[cfg(test)]
/// Why a thumbnail was not written.
#[derive(Debug)]
pub struct ThumbError(pub String);

#[cfg(test)]
/// Renders one cover into `cache_dir` through a temp file and a rename. `Ok(None)` when the URL has no local source.
pub fn render_one(cache_dir: &Path, url: &str, last_modified: i64, scale: u32) -> Result<Option<PathBuf>, ThumbError> {
    let Some(src_path) = local_source(url) else { return Ok(None) };
    let err = |what: &str, e: &dyn std::fmt::Display| ThumbError(format!("cover thumbnail for {url}: {what}: {e}"));
    let src = image::open(&src_path).map_err(|e| err("cannot decode", &e))?;
    let thumb = render(&src, scale);
    std::fs::create_dir_all(cache_dir).map_err(|e| err("cannot create the cache", &e))?;
    let name = file_name(&cache_key(url, last_modified));
    let (tmp, dest) = (cache_dir.join(format!(".{name}.tmp")), cache_dir.join(&name));
    thumb.save_with_format(&tmp, image::ImageFormat::Png).map_err(|e| err("cannot write", &e))?;
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        err("cannot move into place", &e)
    })?;
    Ok(Some(dest))
}

#[cfg(test)]
/// Renders every `(url, last_modified)` job. Each failure is passed to `log` as exactly one line; returns how many
/// thumbnails were written.
pub fn render_all(cache_dir: &Path, jobs: &[(String, i64)], scale: u32, log: &mut dyn FnMut(&str)) -> usize {
    let mut written = 0;
    for (url, last_modified) in jobs {
        match render_one(cache_dir, url, *last_modified, scale) {
            Ok(Some(_)) => written += 1,
            Ok(None) => {}
            Err(e) => log(&e.0.replace('\n', " ")),
        }
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImage, GenericImageView, Rgba};

    fn solid(w: u32, h: u32) -> DynamicImage {
        let mut img = DynamicImage::new_rgba8(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba([200, 30, 30, 255]));
            }
        }
        img
    }

    fn row(url: Option<&str>, lm: i64) -> UpdateRow {
        UpdateRow { entry_id: 1, title: "t".into(), update_id: 1, thumbnail_url: url.map(str::to_owned), cover_last_modified: lm }
    }

    #[test]
    fn geometry_at_scale_one_and_two() {
        assert_eq!(thumb_size(1), (58, 87));
        assert_eq!(thumb_size(2), (116, 174));
        let one = render(&solid(300, 300), 1);
        assert_eq!(one.dimensions(), (58, 87));
        let two = render(&solid(300, 300), 2);
        assert_eq!(two.dimensions(), (116, 174));
        assert_eq!(two.get_pixel(0, 0).0[3], 0, "corner is transparent");
        assert_eq!(two.get_pixel(115, 173).0[3], 0);
        assert_eq!(two.get_pixel(58, 87).0[3], 255, "centre is opaque");
        assert_eq!(two.get_pixel(58, 0).0[3], 255, "straight edges are not rounded");
    }

    #[test]
    fn key_changes_with_the_modified_time() {
        assert_ne!(cache_key("u", 1), cache_key("u", 2));
        assert_eq!(cache_key("file:///covers/a.jpg", 5), "3fcfa9fb9e6a59bf28fe1380e51adb69b792d4b092d57a9c91ff7aba6d1c45e7");
    }

    #[test]
    fn lookup_is_cache_only() {
        let dir = tempfile::tempdir().unwrap();
        let r = row(Some("file:///c.jpg"), 3);
        assert_eq!(cached_path(dir.path(), &r), None);
        let p = dir.path().join(file_name(&cache_key("file:///c.jpg", 3)));
        std::fs::write(&p, b"x").unwrap();
        assert_eq!(cached_path(dir.path(), &r).as_deref(), p.to_str());
        assert_eq!(cached_path(dir.path(), &row(None, 3)), None);
    }

    #[test]
    fn a_file_url_renders_and_a_content_url_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("c.png");
        solid(40, 40).save(&src).unwrap();
        let cache = dir.path().join("cache");
        let url = format!("file://{}", src.display());
        let out = render_one(&cache, &url, 9, 2).unwrap().unwrap();
        assert_eq!(image::open(&out).unwrap().dimensions(), (116, 174));
        assert!(!std::fs::read_dir(&cache).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
        assert!(render_one(&cache, "content://media/1", 0, 2).unwrap().is_none());
    }

    #[test]
    fn one_log_line_per_failed_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("ok.png");
        solid(10, 10).save(&ok).unwrap();
        let jobs = vec![
            ("file:///no/such/file.png".to_owned(), 0),
            (format!("file://{}", ok.display()), 0),
            ("file:///also/missing.png".to_owned(), 1),
            ("content://x".to_owned(), 0),
        ];
        let mut lines = Vec::new();
        let written = render_all(&dir.path().join("cache"), &jobs, 1, &mut |l| lines.push(l.to_owned()));
        assert_eq!(written, 1);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| !l.contains('\n')));
    }
}
