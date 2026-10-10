//! Tiled rendering of large pages (T-44a-049, topic zoom-and-image section 4.3, B§10.1). A page whose longer edge is
//! above [`MAX_TILE_EDGE`] cannot be uploaded as one texture, so it is cut into tiles. The pyramid is built on the
//! decode worker with a 2x2 box filter: level 0 is the full page and each next level halves it, until the whole level
//! fits in one tile. A frame draws the level whose texels are not finer than the screen pixels, and only the tiles that
//! overlap the view. Pure: no GTK types. The page view uploads the tiles it is told to draw and drops the rest.
//!
//! Texture budget (T-43b-021). Uploaded textures are only the tiles that overlap the view of a page that is on screen;
//! `PageView::draw_page` drops every other tile on the next frame. The frame draws a level whose texels are more than
//! half a screen pixel, so the window it covers is under twice the view in level pixels, and [`max_tiles_for_view`]
//! bounds the tiles of one page by that. The reader keeps the current page and two on each side ([`WINDOW_PAGES`]), so
//! [`texture_budget_bytes`] is the peak the uploaded textures can reach, even if every page of the window were drawn at
//! once. It is separate from the 512 MiB of decoded pixels that `decoded_lru` keeps.

use crate::reader::page_decode::{DEFAULT_TILE_THRESHOLD, DecodedPage, needs_tiling};

/// The largest tile edge in pixels of a level (`hardwareBitmapThreshold`, B§10.1).
pub const MAX_TILE_EDGE: u32 = DEFAULT_TILE_THRESHOLD;

/// The pages whose textures can be alive together: the current page and two on each side (T-43b-021).
pub const WINDOW_PAGES: u64 = 5;

/// The bytes of one full tile texture, RGBA8.
pub const TILE_TEXTURE_BYTES: u64 = MAX_TILE_EDGE as u64 * MAX_TILE_EDGE as u64 * 4;

/// The most tiles one page draws for a view of `view` screen pixels. A window of `length` level pixels overlaps at most
/// `ceil(length / edge) + 1` tiles along an axis, and the window is under twice the view (see the module notes).
pub fn max_tiles_for_view(view: (f64, f64)) -> u64 {
    let edge = f64::from(MAX_TILE_EDGE);
    let along = |extent: f64| (2.0 * extent.max(0.0) / edge).ceil() as u64 + 1;
    along(view.0) * along(view.1)
}

/// The peak bytes of uploaded tile textures for the pages of the window, for a view of `view` screen pixels.
pub fn texture_budget_bytes(view: (f64, f64)) -> u64 {
    WINDOW_PAGES * max_tiles_for_view(view) * TILE_TEXTURE_BYTES
}

/// A rectangle of one pyramid level, in that level's pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
    pub level: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The levels of a page. `levels[0]` is the full page. Each next level is half the size of the one before (rounded up).
#[derive(Debug)]
pub struct Pyramid {
    levels: Vec<DecodedPage>,
}

impl Pyramid {
    /// Builds the pyramid of `page` with tiles of at most `max_edge`. Pages that already fit have one level.
    pub fn new(page: DecodedPage, max_edge: u32) -> Self {
        let max_edge = max_edge.max(1);
        let mut levels = vec![page];
        while let Some(last) = levels.last() {
            if !needs_tiling(last.width, last.height, max_edge) {
                break;
            }
            let next = halve(last);
            levels.push(next);
        }
        Self { levels }
    }

    /// The bytes of decoded pixels the pyramid holds, all levels.
    pub fn byte_size(&self) -> usize {
        self.levels.iter().map(|l| l.rgba.len()).sum()
    }

    /// The background `chooseBackground` picks for the full page (level 0), read in place.
    pub fn choose_background(&self, landscape: bool) -> kuro_core::image::background::Background {
        use kuro_core::image::background::{Rgba, choose_by};
        let page = &self.levels[0];
        choose_by(page.width, page.height, landscape, |x, y| {
            let i = (y as usize * page.width as usize + x as usize) * 4;
            Rgba(page.rgba[i], page.rgba[i + 1], page.rgba[i + 2], page.rgba[i + 3])
        })
    }

    /// The full page's size (level 0).
    pub fn size(&self) -> (u32, u32) {
        self.levels.first().map_or((0, 0), |p| (p.width, p.height))
    }

    pub fn level_count(&self) -> u32 {
        self.levels.len() as u32
    }

    /// The pixels of one level, or `None` past the last level.
    pub fn level(&self, level: u32) -> Option<&DecodedPage> {
        self.levels.get(level as usize)
    }

    /// The coarsest level whose texels are still at least one screen pixel across at `scale` (screen pixels per source
    /// pixel). Level `k` covers `2^k` source pixels per texel, so it is used while `scale * 2^k <= 1`.
    pub fn level_for_scale(&self, scale: f64) -> u32 {
        let last = self.level_count().saturating_sub(1);
        if scale <= 0.0 || !scale.is_finite() {
            return 0;
        }
        let mut level = 0;
        while level < last && scale * f64::from(1u32 << (level + 1).min(31)) <= 1.0 {
            level += 1;
        }
        level
    }

    /// The tiles of `level` that overlap the view. `center` is in source pixels of level 0, `view` in screen pixels.
    /// Returns the level used and its visible tiles; empty when the level does not exist or the view is empty.
    pub fn visible_tiles(&self, scale: f64, center: (f64, f64), view: (f64, f64)) -> (u32, Vec<Tile>) {
        let level = self.level_for_scale(scale);
        let Some(page) = self.level(level) else { return (level, Vec::new()) };
        if scale <= 0.0 || view.0 <= 0.0 || view.1 <= 0.0 {
            return (level, Vec::new());
        }
        let factor = f64::from(1u32 << level.min(31));
        // The visible window in level pixels: the view, mapped back through the scale and the centre.
        let (half_w, half_h) = (view.0 / (2.0 * scale) / factor, view.1 / (2.0 * scale) / factor);
        let (cx, cy) = (center.0 / factor, center.1 / factor);
        let (x0, x1) = (cx - half_w, cx + half_w);
        let (y0, y1) = (cy - half_h, cy + half_h);
        let tiles = tiles_of(level, page.width, page.height, MAX_TILE_EDGE)
            .into_iter()
            .filter(|t| f64::from(t.x) < x1 && f64::from(t.x + t.w) > x0 && f64::from(t.y) < y1 && f64::from(t.y + t.h) > y0)
            .collect();
        (level, tiles)
    }

    /// The screen rectangle `(x, y, w, h)` of a tile for the given scale, centre and view size.
    pub fn screen_rect(tile: Tile, scale: f64, center: (f64, f64), view: (f64, f64)) -> (f64, f64, f64, f64) {
        let factor = f64::from(1u32 << tile.level.min(31));
        let x = view.0 / 2.0 + (f64::from(tile.x) * factor - center.0) * scale;
        let y = view.1 / 2.0 + (f64::from(tile.y) * factor - center.1) * scale;
        (x, y, f64::from(tile.w) * factor * scale, f64::from(tile.h) * factor * scale)
    }

    /// The RGBA bytes of a tile, rows of `tile.w * 4` bytes. Empty for a tile outside its level.
    pub fn tile_bytes(&self, tile: Tile) -> Vec<u8> {
        let Some(page) = self.level(tile.level) else { return Vec::new() };
        if tile.x + tile.w > page.width || tile.y + tile.h > page.height {
            return Vec::new();
        }
        let row = page.width as usize * 4;
        let (x0, w) = (tile.x as usize * 4, tile.w as usize * 4);
        let mut out = Vec::with_capacity(w * tile.h as usize);
        for y in tile.y as usize..(tile.y + tile.h) as usize {
            let start = y * row + x0;
            out.extend_from_slice(&page.rgba[start..start + w]);
        }
        out
    }
}

/// The tiles of a `width` by `height` level, row by row, each at most `max_edge` on a side.
pub fn tiles_of(level: u32, width: u32, height: u32, max_edge: u32) -> Vec<Tile> {
    let max_edge = max_edge.max(1);
    let mut tiles = Vec::new();
    let mut y = 0;
    while y < height {
        let h = (height - y).min(max_edge);
        let mut x = 0;
        while x < width {
            let w = (width - x).min(max_edge);
            tiles.push(Tile { level, x, y, w, h });
            x += w;
        }
        y += h;
    }
    tiles
}

/// Halves a page with a 2x2 box filter. An odd edge keeps its last row or column, which is averaged on its own.
pub fn halve(page: &DecodedPage) -> DecodedPage {
    let width = page.width.div_ceil(2).max(1);
    let height = page.height.div_ceil(2).max(1);
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    let src_row = page.width as usize * 4;
    for oy in 0..height {
        for ox in 0..width {
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let sx = (ox * 2 + dx).min(page.width.saturating_sub(1)) as usize;
                    let sy = (oy * 2 + dy).min(page.height.saturating_sub(1)) as usize;
                    let at = sy * src_row + sx * 4;
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += u32::from(page.rgba[at + channel]);
                    }
                    count += 1;
                }
            }
            rgba.extend(sum.iter().map(|total| (total / count) as u8));
        }
    }
    DecodedPage { width, height, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(width: u32, height: u32, value: u8) -> DecodedPage {
        DecodedPage { width, height, rgba: vec![value; width as usize * height as usize * 4] }
    }

    #[test]
    fn the_byte_size_counts_every_level_for_the_cache_budget() {
        let one = Pyramid::new(page(10, 10, 1), MAX_TILE_EDGE);
        assert_eq!(one.byte_size(), 10 * 10 * 4);
        let two = Pyramid::new(page(8, 8, 1), 4);
        assert!(two.level_count() > 1);
        assert!(two.byte_size() > 8 * 8 * 4, "the smaller levels count too");
    }

    #[test]
    fn the_uploaded_tiles_of_a_page_stay_within_the_documented_bound_at_every_zoom() {
        // pages that need two and three levels, in a full HD view, over the whole range of zoom and centres; the bound does
        // not depend on the page, so a page at the 16384 px limit holds it the same way
        let view = (1920.0, 1080.0);
        let bound = max_tiles_for_view(view) as usize;
        assert_eq!(bound, 9);
        for (w, h) in [(4100, 4100), (2100, 6000), (6000, 2100)] {
            let pyramid = Pyramid::new(page(w, h, 7), MAX_TILE_EDGE);
            for step in 0..60 {
                let scale = 0.02 * 1.12f64.powi(step);
                for (cx, cy) in [(0.0, 0.0), (f64::from(w) / 2.0, f64::from(h) / 2.0), (f64::from(w) * 0.37, f64::from(h) * 0.81)] {
                    let (_, tiles) = pyramid.visible_tiles(scale, (cx, cy), view);
                    assert!(tiles.len() <= bound, "{} tiles at scale {scale} on {w}x{h}", tiles.len());
                }
            }
        }
    }

    #[test]
    fn the_texture_budget_is_five_pages_of_the_bounded_tiles() {
        assert_eq!(TILE_TEXTURE_BYTES, 16 * 1024 * 1024);
        assert_eq!(texture_budget_bytes((1920.0, 1080.0)), 5 * 9 * 16 * 1024 * 1024);
        assert!(texture_budget_bytes((800.0, 600.0)) < texture_budget_bytes((3840.0, 2160.0)), "a bigger view may draw more tiles");
    }

    #[test]
    fn a_page_that_fits_one_tile_has_one_level_and_one_tile() {
        let pyramid = Pyramid::new(page(300, 900, 1), MAX_TILE_EDGE);
        assert_eq!(pyramid.level_count(), 1);
        assert_eq!(tiles_of(0, 300, 900, MAX_TILE_EDGE).len(), 1);
    }

    #[test]
    fn a_strip_of_twenty_thousand_pixels_is_tiled_without_a_tile_above_the_cap() {
        let pyramid = Pyramid::new(page(1, 20_000, 9), MAX_TILE_EDGE);
        let (level, tiles) = pyramid.visible_tiles(1.0, (0.5, 10_000.0), (1.0, 800.0));
        assert_eq!(level, 0);
        assert!(!tiles.is_empty());
        assert!(tiles.iter().all(|t| t.w <= MAX_TILE_EDGE && t.h <= MAX_TILE_EDGE));
        let all = tiles_of(0, 1, 20_000, MAX_TILE_EDGE);
        assert_eq!(all.iter().map(|t| t.h).sum::<u32>(), 20_000, "tiles cover the strip");
        assert_eq!(all.len(), 10);
    }

    #[test]
    fn only_the_tiles_that_overlap_the_view_are_drawn() {
        let pyramid = Pyramid::new(page(1, 20_000, 9), MAX_TILE_EDGE);
        // A 600 px view at scale 1 centred on row 10000 sees rows 9700..10300: tiles starting at 8192 and 10240.
        let (_, tiles) = pyramid.visible_tiles(1.0, (0.5, 10_000.0), (1.0, 600.0));
        let rows: Vec<u32> = tiles.iter().map(|t| t.y).collect();
        assert_eq!(rows, vec![8192, 10240]);
    }

    #[test]
    fn the_level_is_the_coarsest_that_keeps_one_texel_per_screen_pixel() {
        let pyramid = Pyramid::new(page(4096, 4096, 0), MAX_TILE_EDGE);
        assert_eq!(pyramid.level_count(), 2, "4096 -> 2048 is one halving; 2048 fits");
        assert_eq!(pyramid.level_for_scale(1.0), 0);
        assert_eq!(pyramid.level_for_scale(0.5), 1, "two source pixels per screen pixel");
        assert_eq!(pyramid.level_for_scale(0.3), 1);
        assert_eq!(pyramid.level_for_scale(0.1), 1, "clamped to the last level");
        assert_eq!(pyramid.level_for_scale(0.0), 0);
    }

    #[test]
    fn tiles_of_a_large_page_stay_within_the_cap_and_cover_it() {
        let tiles = tiles_of(0, 5000, 3000, MAX_TILE_EDGE);
        assert!(tiles.iter().all(|t| t.w <= MAX_TILE_EDGE && t.h <= MAX_TILE_EDGE));
        let area: u64 = tiles.iter().map(|t| u64::from(t.w) * u64::from(t.h)).sum();
        assert_eq!(area, 5000 * 3000);
    }

    #[test]
    fn a_screen_rectangle_scales_the_tile_by_its_level() {
        let tile = Tile { level: 1, x: 10, y: 20, w: 30, h: 40 };
        // Level 1 pixels are two source pixels. At scale 0.5 a tile is 30 px wide on screen and centred on (0, 0).
        let (x, y, w, h) = Pyramid::screen_rect(tile, 0.5, (0.0, 0.0), (100.0, 100.0));
        assert_eq!((x, y), (50.0 + 10.0, 50.0 + 20.0));
        assert_eq!((w, h), (30.0, 40.0));
    }

    #[test]
    fn a_tile_is_cropped_from_its_level() {
        // A 4x2 page where each pixel's red channel is its column.
        let mut rgba = Vec::new();
        for _row in 0..2 {
            for col in 0..4u8 {
                rgba.extend_from_slice(&[col, 0, 0, 255]);
            }
        }
        let pyramid = Pyramid::new(DecodedPage { width: 4, height: 2, rgba }, 2);
        let bytes = pyramid.tile_bytes(Tile { level: 0, x: 1, y: 0, w: 2, h: 2 });
        assert_eq!(bytes.len(), 2 * 2 * 4);
        assert_eq!(bytes[0], 1, "first pixel of the tile is column 1");
        assert_eq!(bytes[4], 2, "second pixel is column 2");
        assert!(pyramid.tile_bytes(Tile { level: 0, x: 3, y: 0, w: 2, h: 2 }).is_empty(), "outside the page");
    }

    #[test]
    fn halving_averages_two_by_two_and_keeps_odd_edges() {
        let mut rgba = Vec::new();
        for v in [0u8, 100, 200] {
            rgba.extend_from_slice(&[v, v, v, 255]);
        }
        let halved = halve(&DecodedPage { width: 3, height: 1, rgba });
        assert_eq!((halved.width, halved.height), (2, 1));
        assert_eq!(halved.rgba[0], 50, "(0 + 100) / 2 across the first pair");
        assert_eq!(halved.rgba[4], 200, "the odd last column stands alone");
    }
}
