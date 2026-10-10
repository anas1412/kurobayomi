//! Cover frames of the entry screens (T-38a-039). Upstream: `ItemCover.kt`. A cover has one of three aspect ratios and is
//! cropped to fill its frame. While it loads it is a flat tile of 12 % grey, and a failed load shows the cover-error
//! icon. Pure: the widget reads these values.

/// The aspect ratio of a cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ratio {
    /// 1:1.
    Square,
    /// 2:3, the book cover.
    Book,
    /// 16:9.
    Thumb,
}

impl Ratio {
    /// The ratio as `(width, height)` parts.
    pub fn parts(self) -> (i32, i32) {
        match self {
            Self::Square => (1, 1),
            Self::Book => (2, 3),
            Self::Thumb => (16, 9),
        }
    }

    /// The height a frame of `width` needs at this ratio, rounded down.
    pub fn height_for(self, width: i32) -> i32 {
        let (w, h) = self.parts();
        width * h / w
    }
}

/// The placeholder tile colour, ARGB `0x1F888888`: 12 % grey.
pub const PLACEHOLDER_ARGB: u32 = 0x1F88_8888;

/// What a cover frame shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// Still loading: the flat tile.
    Placeholder,
    /// The picture is ready.
    Image,
    /// The load failed: the cover-error icon.
    Failed,
}

/// A failure shows the error icon even when a picture was attempted; otherwise a loaded picture shows, and anything else
/// is the placeholder.
pub fn frame(loaded: bool, failed: bool) -> Frame {
    if failed {
        Frame::Failed
    } else if loaded {
        Frame::Image
    } else {
        Frame::Placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_follow_the_three_ratios() {
        assert_eq!(Ratio::Square.height_for(120), 120);
        assert_eq!(Ratio::Book.height_for(120), 180);
        assert_eq!(Ratio::Thumb.height_for(160), 90);
    }

    #[test]
    fn the_placeholder_is_twelve_percent_grey() {
        assert_eq!(PLACEHOLDER_ARGB, 0x1F888888);
        assert_eq!(PLACEHOLDER_ARGB >> 24, 0x1F);
    }

    #[test]
    fn a_failure_wins_over_a_loaded_picture() {
        assert_eq!(frame(false, false), Frame::Placeholder);
        assert_eq!(frame(true, false), Frame::Image);
        assert_eq!(frame(true, true), Frame::Failed);
        assert_eq!(frame(false, true), Frame::Failed);
    }
}
