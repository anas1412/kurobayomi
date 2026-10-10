//! Border cropping at decode time (T-44a-053, TZ §6, E-55 to E-57). Port of `findBorders` in the Kotlin decoder's
//! `library/src/main/cpp/borders.cpp`. Pure: RGBA in, RGBA out, no GTK.
//!
//! Pixels are reduced to 8-bit luminance (Rec. 601 integer luma, the choice TZ §6 documents). Each side is found from
//! the outside in: the outer line is tallied for black and white samples (every other pixel, the source's stride 2).
//! A mixed outer line leaves that side alone. Otherwise the first line that is filled with the opposite tone to the
//! outer line, by more than the side's fill limit, is where the content starts. A page with no such line, or a crop
//! that would leave nothing, comes back as it was.

use crate::reader::page_decode::DecodedPage;

/// The share of a line's length that may be detected before the line counts (`filledRatioLimit`, halved in use).
const FILLED_RATIO_LIMIT: f32 = 0.0025;
/// `isBlackPixel`: a luminance below 75 percent of 255 (`thresholdForBlack` = 191).
const BLACK_BELOW: u8 = 191;
/// `isWhitePixel`: a luminance above 25 percent of 255 (`thresholdForWhite` = 63).
const WHITE_ABOVE: u8 = 63;

fn is_black(p: u8) -> bool {
    p < BLACK_BELOW
}

fn is_white(p: u8) -> bool {
    p > WHITE_ABOVE
}

/// The fill limit of a line of `length` samples: `round(length * 0.0025 / 2)` in single precision, as the source has it.
fn filled_limit(length: usize) -> u32 {
    (length as f32 * FILLED_RATIO_LIMIT / 2.0).round() as u32
}

/// Rec. 601 integer luminance of one RGBA pixel. Alpha is ignored.
fn luma(px: &[u8]) -> u8 {
    ((u32::from(px[0]) * 299 + u32::from(px[1]) * 587 + u32::from(px[2]) * 114) / 1000) as u8
}

/// The content start of one side. `outer` is the outer line; `candidates` are the lines to scan, in order, each with its
/// index. `None` when the outer line is mixed (black and white both over the limit, the source's "mixed fill") or no
/// candidate is filled with the tone the outer line lacks.
fn content_line(outer: &[u8], limit: u32, mut candidates: impl Iterator<Item = (usize, Vec<u8>)>) -> Option<usize> {
    let (black, white) = outer.iter().fold((0u32, 0u32), |(b, w), &p| {
        // the first line counts a pixel once, black before white
        if is_black(p) {
            (b + 1, w)
        } else if is_white(p) {
            (b, w + 1)
        } else {
            (b, w)
        }
    });
    if white > limit && black > limit {
        return None;
    }
    let look_for_white = black > limit;
    candidates.find_map(|(index, line)| {
        let count = line.iter().filter(|&&p| if look_for_white { is_white(p) } else { is_black(p) }).count() as u32;
        (count > limit).then_some(index)
    })
}

/// The kept rectangle: columns `x0..x1` and rows `y0..y1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Keep {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

/// `findBorderTop`, `findBorderBottom`, `findBorderLeft` and `findBorderRight`, in that order: the top and bottom use the
/// width's fill limit, the left and right the height's, and the columns are sampled only inside the kept rows.
fn find_borders(plane: &[u8], w: usize, h: usize) -> Keep {
    let row = |y: usize| -> Vec<u8> { (0..w).step_by(2).map(|x| plane[y * w + x]).collect() };
    let limit_w = filled_limit(w);
    let top = content_line(&row(0), limit_w, (1..h).map(|y| (y, row(y)))).unwrap_or(0);
    let bottom = content_line(&row(h - 1), limit_w, (1..h.saturating_sub(1)).rev().map(|y| (y, row(y))))
        .map_or(h, |y| y + 1);
    let limit_h = filled_limit(h);
    let col = |x: usize| -> Vec<u8> { (top..bottom).step_by(2).map(|y| plane[y * w + x]).collect() };
    let left = content_line(&col(0), limit_h, (1..w).map(|x| (x, col(x)))).unwrap_or(0);
    let right = content_line(&col(w - 1), limit_h, (1..w.saturating_sub(1)).rev().map(|x| (x, col(x))))
        .map_or(w, |x| x + 1);
    Keep { x0: left, x1: right, y0: top, y1: bottom }
}

/// Cuts the uniform border off `page` (`crop_borders` and `crop_borders_webtoon`). A page with no border, a border that
/// would leave no area, and a page whose buffer does not match its size come back unchanged (E-55, E-57).
pub fn crop_borders(page: DecodedPage) -> DecodedPage {
    let (w, h) = (page.width as usize, page.height as usize);
    if w == 0 || h == 0 || page.rgba.len() != w * h * 4 {
        return page;
    }
    let plane: Vec<u8> = page.rgba.chunks_exact(4).map(luma).collect();
    let keep = find_borders(&plane, w, h);
    if keep == (Keep { x0: 0, x1: w, y0: 0, y1: h }) {
        return page;
    }
    if keep.x1 <= keep.x0 || keep.y1 <= keep.y0 {
        tracing::warn!("the border crop would leave no area; the page is kept as it is");
        return page;
    }
    let (width, height) = (keep.x1 - keep.x0, keep.y1 - keep.y0);
    let (stride, from, len) = (w * 4, keep.x0 * 4, width * 4);
    let mut rgba = Vec::with_capacity(len * height);
    for y in keep.y0..keep.y1 {
        rgba.extend_from_slice(&page.rgba[y * stride + from..y * stride + from + len]);
    }
    DecodedPage { width: width as u32, height: height as u32, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const BLACK: [u8; 4] = [0, 0, 0, 255];
    const INK: [u8; 4] = [30, 40, 50, 255];
    const GREY: [u8; 4] = [128, 128, 128, 255];

    /// A `width` by `height` page of `fill` with a block of `ink` `margin` px from every edge.
    fn framed(width: u32, height: u32, margin: u32, fill: [u8; 4], ink: [u8; 4]) -> DecodedPage {
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let inside = x >= margin && x < width - margin && y >= margin && y < height - margin;
                rgba.extend_from_slice(if inside { &ink } else { &fill });
            }
        }
        DecodedPage { width, height, rgba }
    }

    #[test]
    fn e55_a_white_margin_is_cut_to_the_content_and_the_page_shrinks_by_forty_each_way() {
        let page = framed(120, 100, 20, WHITE, INK);
        let cropped = crop_borders(page.clone());
        assert_eq!((page.width, page.height), (120, 100), "uncropped size");
        assert_eq!((cropped.width, cropped.height), (80, 60), "cropped size");
        assert_eq!(cropped.rgba.len(), 80 * 60 * 4);
        assert!(cropped.rgba.chunks(4).all(|px| px == INK), "only the inside is left");
    }

    #[test]
    fn e55_a_black_margin_around_grey_content_is_cut() {
        let cropped = crop_borders(framed(60, 60, 5, BLACK, GREY));
        assert_eq!((cropped.width, cropped.height), (50, 50));
    }

    #[test]
    fn e55_a_page_that_is_all_margin_has_no_content_line_and_is_kept() {
        let blank = framed(10, 10, 0, WHITE, WHITE);
        assert_eq!(crop_borders(blank.clone()), blank);
    }

    #[test]
    fn e56_a_page_whose_outer_lines_are_all_mixed_is_unchanged() {
        // every outer line alternates black and white on its sampled pixels, so no side is cut
        let (w, h) = (50usize, 40usize);
        let mut page = framed(w as u32, h as u32, 0, WHITE, WHITE);
        for y in 0..h {
            for x in 0..w {
                if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                    let px = if (x + y) % 4 < 2 { BLACK } else { WHITE };
                    page.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&px);
                }
            }
        }
        assert_eq!(crop_borders(page.clone()), page);
    }

    #[test]
    fn e56_a_mixed_top_line_keeps_every_row() {
        // the top row alternates black and white on the sampled pixels, so the top is not cut
        let mut page = framed(8, 8, 0, INK, INK);
        for x in 0..8usize {
            let px = if x % 4 == 0 { BLACK } else { WHITE };
            page.rgba[x * 4..x * 4 + 4].copy_from_slice(&px);
        }
        let cropped = crop_borders(page);
        assert_eq!(cropped.height, 8, "the mixed top keeps every row");
    }

    #[test]
    fn borders_of_different_widths_are_each_cut_by_their_own_extent() {
        let mut page = framed(40, 30, 0, INK, INK);
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
    }

    #[test]
    fn a_transparent_page_has_no_content_line_and_an_empty_page_is_returned_as_is() {
        let clear = DecodedPage { width: 4, height: 4, rgba: vec![0; 64] };
        assert_eq!(crop_borders(clear.clone()), clear);
        let empty = DecodedPage { width: 0, height: 0, rgba: Vec::new() };
        assert_eq!(crop_borders(empty.clone()), empty);
    }

    #[test]
    fn the_luma_thresholds_are_the_source_ones() {
        assert!(is_black(190) && !is_black(191), "black is under 191");
        assert!(is_white(64) && !is_white(63), "white is over 63");
        assert!(is_black(100) && is_white(100), "a grey between the thresholds counts as both");
    }

    #[test]
    fn the_fill_limit_is_the_source_rounding_of_half_a_percent_of_the_length() {
        assert_eq!(filled_limit(300), 0);
        assert_eq!(filled_limit(1000), 1);
    }

    #[test]
    fn the_luma_of_a_pixel_is_rec_601_integer() {
        assert_eq!(luma(&WHITE), 255);
        assert_eq!(luma(&[255, 0, 0, 255]), 76);
        assert_eq!(luma(&[0, 255, 0, 255]), 149);
    }
}
