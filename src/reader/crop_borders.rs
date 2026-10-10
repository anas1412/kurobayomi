//! Border cropping at decode time (T-43b-020, `crop_borders` and `crop_borders_webtoon`). Pure: pixels in, pixels out, no
//! GTK. The page decoder of Aniyomi cuts the solid white or black margin that scans carry; this does the same on the
//! decoded RGBA, on the decode worker, before the pyramid is built.
//!
//! A border is a run of outer rows or columns in which every pixel is the border colour, white or black, give or take
//! [`TOLERANCE`] on each channel. The colour is whichever of the two removes something. A page with no such margin,
//! and a page that is nothing but margin, come back as they were.

use crate::reader::page_decode::DecodedPage;

/// How far a channel may be from pure white or pure black and still count as border. Scans and JPEG noise sit a few
/// steps off the pure colour, and a margin is not uniform without this slack. 16 of 255 is about 6 percent.
pub const TOLERANCE: u8 = 16;

/// The rows and columns to keep: `x0..x1` by `y0..y1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Keep {
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
}

/// Whether the pixel at byte offset `at` is the border colour `white` (or black) within the tolerance. Transparent
/// pixels have no colour of their own, so they never count as border.
fn is_border(rgba: &[u8], at: usize, white: bool) -> bool {
    let px = &rgba[at..at + 4];
    px[3] > 0
        && px[..3].iter().all(|&c| if white { c >= 255 - TOLERANCE } else { c <= TOLERANCE })
}

/// The part of `page` that is not border for the colour `white` (or black). `None` when the whole page is border.
fn keep_for(page: &DecodedPage, white: bool) -> Option<Keep> {
    let (w, h) = (page.width as usize, page.height as usize);
    let row_is_border = |y: usize, x0: usize, x1: usize| (x0..x1).all(|x| is_border(&page.rgba, (y * w + x) * 4, white));
    let col_is_border = |x: usize, y0: usize, y1: usize| (y0..y1).all(|y| is_border(&page.rgba, (y * w + x) * 4, white));
    let y0 = (0..h).find(|&y| !row_is_border(y, 0, w))?;
    let y1 = (y0..h).rev().find(|&y| !row_is_border(y, 0, w))? + 1;
    let x0 = (0..w).find(|&x| !col_is_border(x, y0, y1))?;
    let x1 = (x0..w).rev().find(|&x| !col_is_border(x, y0, y1))? + 1;
    Some(Keep { x0: x0 as u32, x1: x1 as u32, y0: y0 as u32, y1: y1 as u32 })
}

/// Cuts the uniform white or black border off `page`. Nothing is copied when there is nothing to cut.
pub fn crop_borders(page: DecodedPage) -> DecodedPage {
    let full = (page.width as usize) * (page.height as usize) * 4;
    if page.width == 0 || page.height == 0 || page.rgba.len() != full {
        return page;
    }
    let cut = |keep: &Keep| (keep.x1 - keep.x0) * (keep.y1 - keep.y0);
    // the colour that removes the most is the page's margin; ties and no margin keep the page
    let best = [true, false].into_iter().filter_map(|white| keep_for(&page, white)).min_by_key(cut);
    let Some(keep) = best.filter(|k| cut(k) < page.width * page.height) else { return page };
    let (width, height) = (keep.x1 - keep.x0, keep.y1 - keep.y0);
    let (row, from, len) = (page.width as usize * 4, keep.x0 as usize * 4, width as usize * 4);
    let mut rgba = Vec::with_capacity(len * height as usize);
    for y in keep.y0 as usize..keep.y1 as usize {
        rgba.extend_from_slice(&page.rgba[y * row + from..y * row + from + len]);
    }
    DecodedPage { width, height, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const INK: [u8; 4] = [30, 40, 50, 255];

    /// A `width` by `height` page of `fill` with a `core` by `core` ink block `margin` px from every edge.
    fn bordered(width: u32, height: u32, margin: u32, fill: [u8; 4]) -> DecodedPage {
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let inside = x >= margin && x < width - margin && y >= margin && y < height - margin;
                rgba.extend_from_slice(if inside { &INK } else { &fill });
            }
        }
        DecodedPage { width, height, rgba }
    }

    #[test]
    fn a_twenty_pixel_white_border_is_cut_and_the_page_shrinks_by_forty_each_way() {
        let page = bordered(120, 100, 20, WHITE);
        let cropped = crop_borders(page.clone());
        assert_eq!((page.width, page.height), (120, 100), "uncropped size");
        assert_eq!((cropped.width, cropped.height), (80, 60), "cropped size");
        assert_eq!(cropped.rgba.len(), 80 * 60 * 4);
        assert!(cropped.rgba.chunks(4).all(|px| px == INK), "only the inside is left");
    }

    #[test]
    fn a_black_border_is_cut_too() {
        let cropped = crop_borders(bordered(60, 60, 5, [0, 0, 0, 255]));
        assert_eq!((cropped.width, cropped.height), (50, 50));
    }

    #[test]
    fn a_border_within_the_tolerance_counts_and_one_past_it_does_not() {
        let near = [255 - TOLERANCE, 255, 255 - TOLERANCE, 255];
        assert_eq!(crop_borders(bordered(40, 40, 4, near)).width, 32);
        let off = [254 - TOLERANCE, 255, 255, 255];
        assert_eq!(crop_borders(bordered(40, 40, 4, off)).width, 40, "a step past the tolerance is page content");
    }

    #[test]
    fn a_page_with_no_uniform_border_is_unchanged() {
        let mut page = bordered(50, 40, 0, WHITE);
        page.rgba.chunks_mut(4).enumerate().for_each(|(i, px)| px[0] = (i % 200) as u8 + 20);
        assert_eq!(crop_borders(page.clone()), page);
    }

    #[test]
    fn borders_of_different_widths_are_each_cut_and_a_blank_page_is_kept() {
        let mut page = bordered(40, 30, 0, INK);
        // white margin: 3 px left, 7 px right, 2 px top, 5 px bottom
        for y in 0..30u32 {
            for x in 0..40u32 {
                if x < 3 || x >= 33 || y < 2 || y >= 25 {
                    let at = ((y * 40 + x) * 4) as usize;
                    page.rgba[at..at + 4].copy_from_slice(&WHITE);
                }
            }
        }
        let cropped = crop_borders(page);
        assert_eq!((cropped.width, cropped.height), (30, 23));
        let blank = bordered(10, 10, 0, WHITE);
        assert_eq!(crop_borders(blank.clone()), blank, "nothing but margin is not cropped to nothing");
    }

    #[test]
    fn a_transparent_page_has_no_border_and_an_empty_page_is_returned_as_is() {
        let clear = DecodedPage { width: 4, height: 4, rgba: vec![0; 64] };
        assert_eq!(crop_borders(clear.clone()), clear);
        let empty = DecodedPage { width: 0, height: 0, rgba: Vec::new() };
        assert_eq!(crop_borders(empty.clone()), empty);
    }
}
