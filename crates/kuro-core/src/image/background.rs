//! Automatic reader page background. Port of `ImageUtil.chooseBackground`
//! (`core/common/.../util/system/ImageUtil.kt:338-580`), behaviour 5.12.
//!
//! The Kotlin code indexes the decoded bitmap directly, so a sample column outside the image
//! throws (widths 100 to 109 put `rightOffsetX` at `W`). Deviation D-6: an out-of-range sample
//! reads as a fully transparent pixel, which is neither dark nor white.

use image::RgbaImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

pub const WHITE: Rgba = Rgba(255, 255, 255, 255);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Background {
    Solid(Rgba),
    /// Android `GradientDrawable` TOP_BOTTOM with four stops at 0, 1/3, 2/3 and 1: `top` for the
    /// first two stops, `bottom` for the last two.
    VerticalBands { top: Rgba, bottom: Rgba },
}

impl Background {
    /// The four gradient stops as `(offset, colour)`; a solid colour is one stop pair.
    pub fn stops(&self) -> [(f32, Rgba); 4] {
        match self {
            Background::Solid(c) => [(0.0, *c), (1.0 / 3.0, *c), (2.0 / 3.0, *c), (1.0, *c)],
            Background::VerticalBands { top, bottom } => {
                [(0.0, *top), (1.0 / 3.0, *top), (2.0 / 3.0, *bottom), (1.0, *bottom)]
            }
        }
    }
}

fn is_dark(p: Rgba) -> bool {
    p.0 < 40 && p.1 < 40 && p.2 < 40 && p.3 > 200
}

fn is_white(p: Rgba) -> bool {
    p.0 as u32 + p.1 as u32 + p.2 as u32 > 740
}

fn is_close_to(p: Rgba, o: Rgba) -> bool {
    p.0.abs_diff(o.0) < 30 && p.1.abs_diff(o.1) < 30 && p.2.abs_diff(o.2) < 30
}

/// Pure port of `chooseBackground` after decoding. `landscape` is the viewport orientation.
pub fn choose(img: &RgbaImage, landscape: bool) -> Background {
    choose_by(img.width(), img.height(), landscape, |x, y| {
        let p = img.get_pixel(x, y).0;
        Rgba(p[0], p[1], p[2], p[3])
    })
}

/// [`choose`] over any pixel source of `width` x `height`, so a caller holding raw RGBA bytes
/// does not copy them into an image first. `get` is only called inside the image.
pub fn choose_by(width: u32, height: u32, landscape: bool, get: impl Fn(u32, u32) -> Rgba) -> Background {
    let px = |x: i64, y: i64| -> Rgba {
        if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
            return Rgba(0, 0, 0, 0);
        }
        get(x as u32, y as u32)
    };
    let (w, h) = (width as i64, height as i64);
    if w < 50 || h < 50 {
        return Background::Solid(WHITE);
    }

    let top = 5;
    let bot = h - 5;
    let left = (w as f64 * 0.0275) as i64;
    let right = w - left;
    let mid_x = w / 2;
    let mid_y = h / 2;
    let offset_x = (w as f64 * 0.01) as i64;
    let left_offset_x = left - offset_x;
    let right_offset_x = right + offset_x;

    let top_left = px(left, top);
    let top_right = px(right, top);
    let mid_left = px(left, mid_y);
    let mid_right = px(right, mid_y);
    let top_center = px(mid_x, top);
    let bot_left = px(left, bot);
    let bottom_center = px(mid_x, bot);
    let bot_right = px(right, bot);

    let top_left_dark = is_dark(top_left);
    let top_right_dark = is_dark(top_right);
    let mid_left_dark = is_dark(mid_left);
    let mid_right_dark = is_dark(mid_right);
    let top_mid_dark = is_dark(top_center);
    let bot_left_dark = is_dark(bot_left);
    let bot_right_dark = is_dark(bot_right);

    let mut dark_bg = (top_left_dark
        && (bot_left_dark || bot_right_dark || top_right_dark || mid_left_dark || top_mid_dark))
        || (top_right_dark && (bot_right_dark || bot_left_dark || mid_right_dark || top_mid_dark));

    let border = [top_left, top_center, top_right, bot_right, bottom_center, bot_left];
    let uniform = (0..border.len()).all(|i| {
        let c = border[i];
        !is_white(c) && is_close_to(c, border[(i + 1) % border.len()])
    });
    if uniform {
        return Background::Solid(top_left);
    }

    let white_corners =
        [top_left, top_right, bot_left, bot_right].into_iter().filter(|c| is_white(*c)).count();
    if white_corners > 2 {
        dark_bg = false;
    }

    let mut black = if top_left_dark {
        top_left
    } else if top_right_dark {
        top_right
    } else if bot_left_dark {
        bot_left
    } else if bot_right_dark {
        bot_right
    } else {
        WHITE
    };
    let right_black = |black: Rgba| {
        if top_right_dark {
            top_right
        } else if bot_right_dark {
            bot_right
        } else {
            black
        }
    };

    let mut overall_white = 0;
    let mut overall_black = 0;
    let (mut top_black_streak, mut top_white_streak) = (0, 0);
    let (mut bot_black_streak, mut bot_white_streak) = (0, 0);
    let step = (h / 25) as usize;

    for x in [left, right, left_offset_x, right_offset_x] {
        let (mut white_streak_len, mut white_pixels) = (0, 0);
        let (mut black_streak_len, mut black_pixels) = (0, 0);
        let (mut black_streak, mut white_streak) = (false, false);
        let not_offset = x == left || x == right;
        for (index, y) in (0..h).step_by(step).enumerate() {
            let index = index as i64;
            let pixel = px(x, y);
            let pixel_off = px(x + if x < w / 2 { -offset_x } else { offset_x }, y);
            if is_white(pixel) {
                white_streak_len += 1;
                white_pixels += 1;
                if not_offset {
                    overall_white += 1;
                }
                if white_streak_len > 14 {
                    white_streak = true;
                }
                if white_streak_len > 6 && white_streak_len >= index - 1 {
                    top_white_streak = white_streak_len;
                }
            } else {
                white_streak_len = 0;
                if is_dark(pixel) && is_dark(pixel_off) {
                    black_pixels += 1;
                    if not_offset {
                        overall_black += 1;
                    }
                    black_streak_len += 1;
                    if black_streak_len >= 14 {
                        black_streak = true;
                    }
                    continue;
                }
            }
            if black_streak_len > 6 && black_streak_len >= index - 1 {
                top_black_streak = black_streak_len;
            }
            black_streak_len = 0;
        }
        if black_streak_len > 6 {
            bot_black_streak = black_streak_len;
        } else if white_streak_len > 6 {
            bot_white_streak = white_streak_len;
        }

        if black_pixels > 22 {
            if x == right || x == right_offset_x {
                black = right_black(black);
            }
            dark_bg = true;
            overall_white = 0;
            break;
        } else if black_streak {
            dark_bg = true;
            if x == right || x == right_offset_x {
                black = right_black(black);
            }
            if black_pixels > 18 {
                overall_white = 0;
                break;
            }
        } else if white_streak || white_pixels > 22 {
            dark_bg = false;
        }
    }

    let top_is_black_streak = top_black_streak > top_white_streak;
    let bottom_is_black_streak = bot_black_streak > bot_white_streak;
    if overall_white > 9 && overall_white > overall_black {
        dark_bg = false;
    }
    if top_is_black_streak && bottom_is_black_streak {
        dark_bg = true;
    }

    if landscape {
        return Background::Solid(if dark_bg { black } else { WHITE });
    }

    let bot_corners_white = is_white(bot_left) && is_white(bot_right);
    let top_corners_white = is_white(top_left) && is_white(top_right);
    let top_corners_dark = top_left_dark && top_right_dark;
    let bot_corners_dark = bot_left_dark && bot_right_dark;
    let top_offset_corners_dark =
        is_dark(px(left_offset_x, top)) && is_dark(px(right_offset_x, top));
    let bot_offset_corners_dark =
        is_dark(px(left_offset_x, bot)) && is_dark(px(right_offset_x, bot));

    if dark_bg && bot_corners_white {
        Background::VerticalBands { top: black, bottom: WHITE }
    } else if dark_bg && top_corners_white {
        Background::VerticalBands { top: WHITE, bottom: black }
    } else if dark_bg {
        Background::Solid(black)
    } else if top_is_black_streak
        || (top_corners_dark && top_offset_corners_dark && (top_mid_dark || overall_black > 9))
    {
        Background::VerticalBands { top: black, bottom: WHITE }
    } else if bottom_is_black_streak
        || (bot_corners_dark
            && bot_offset_corners_dark
            && (is_dark(bottom_center) || overall_black > 9))
    {
        Background::VerticalBands { top: WHITE, bottom: black }
    } else {
        Background::Solid(WHITE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    const BLACK: Rgba = Rgba(0, 0, 0, 255);

    fn fill(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba(c))
    }

    fn rect(img: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 4]) {
        for y in y0..y1.min(img.height()) {
            for x in x0..x1.min(img.width()) {
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }

    #[test]
    fn tiny_images_are_white() {
        assert_eq!(choose(&fill(49, 300, [0, 0, 0, 255]), false), Background::Solid(WHITE));
        assert_eq!(choose(&fill(300, 49, [0, 0, 0, 255]), false), Background::Solid(WHITE));
    }

    #[test]
    fn uniform_border_returns_the_corner_colour() {
        assert_eq!(choose(&fill(200, 300, [0, 0, 0, 255]), false), Background::Solid(BLACK));
        assert_eq!(choose(&fill(200, 300, [255, 255, 255, 255]), false), Background::Solid(WHITE));
    }

    #[test]
    fn widths_100_to_109_do_not_panic() {
        for w in 100..=109 {
            let mut img = fill(w, 200, [255, 255, 255, 255]);
            rect(&mut img, w - 4, 0, w, 200, [0, 0, 0, 255]);
            let _ = choose(&img, false);
            let _ = choose(&img, true);
        }
    }

    #[test]
    fn stops_are_at_thirds() {
        let b = Background::VerticalBands { top: BLACK, bottom: WHITE };
        let s = b.stops();
        assert_eq!(s.map(|(o, _)| o), [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]);
        assert_eq!(s.map(|(_, c)| c), [BLACK, BLACK, WHITE, WHITE]);
        let s = Background::Solid(WHITE).stops();
        assert!(s.iter().all(|(_, c)| *c == WHITE));
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        width: u32,
        height: u32,
        fill: [u8; 4],
        /// `[x0, y0, x1, y1, r, g, b, a]`, painted in order.
        rects: Vec<[u32; 8]>,
        landscape: bool,
        /// `{"solid": [r,g,b,a]}` or `{"bands": {"top": [..], "bottom": [..]}}`
        expected: serde_json::Value,
    }

    fn rgba(v: &serde_json::Value) -> Rgba {
        let a: Vec<u8> = serde_json::from_value(v.clone()).unwrap();
        Rgba(a[0], a[1], a[2], a[3])
    }

    /// Recorded by `tests/data/gen_background_reference.py`, a line-by-line Python port of the
    /// Kotlin function (OQ in area 13 overview: no JVM harness here).
    #[test]
    fn reproduces_background_reference_json() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/background_reference.json"
        ))
        .unwrap();
        let cases: Vec<Case> = serde_json::from_str(&text).unwrap();
        assert!(cases.len() >= 20);
        for c in cases {
            let mut img = fill(c.width, c.height, c.fill);
            for r in &c.rects {
                rect(&mut img, r[0], r[1], r[2], r[3], [r[4] as u8, r[5] as u8, r[6] as u8, r[7] as u8]);
            }
            let want = if let Some(s) = c.expected.get("solid") {
                Background::Solid(rgba(s))
            } else {
                let b = &c.expected["bands"];
                Background::VerticalBands { top: rgba(&b["top"]), bottom: rgba(&b["bottom"]) }
            };
            assert_eq!(choose(&img, c.landscape), want, "case {}", c.name);
        }
    }
}
