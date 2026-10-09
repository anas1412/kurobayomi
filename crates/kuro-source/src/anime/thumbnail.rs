//! Seek-preview thumbnails (`ThumbnailInfo.kt`, behaviour 9; T-16a-032).

/// One preview frame inside an image tile. Pixel units for `x`, `y`, `width` and `height`; `time_ms` is the start of the
/// preview in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileInfo {
    pub image_index: usize,
    pub time_ms: i64,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// The preview sheet of a video: frames and the image tiles that hold them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThumbnailInfo {
    pub tile_info: Vec<TileInfo>,
    /// `TileInfo::image_index` points into this list.
    pub image_tile_urls: Vec<String>,
}

impl ThumbnailInfo {
    /// Sorts the frames by time, ascending and stable (`tileInfo` is sorted by `timeMs` before use).
    pub fn sort_tiles(&mut self) {
        self.tile_info.sort_by_key(|tile| tile.time_ms);
    }

    /// The image tiles fetched before playback: the first two only (behaviour 9.2).
    pub fn preload_urls(&self) -> &[String] {
        &self.image_tile_urls[..self.image_tile_urls.len().min(2)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(time_ms: i64, image_index: usize) -> TileInfo {
        TileInfo { image_index, time_ms, x: 0, y: 0, width: 160, height: 90 }
    }

    #[test]
    fn tiles_sort_ascending_by_time_and_keep_equal_times_in_order() {
        let mut info = ThumbnailInfo {
            tile_info: vec![tile(3000, 0), tile(1000, 1), tile(1000, 2)],
            image_tile_urls: vec!["a".into()],
        };
        info.sort_tiles();
        let order: Vec<(i64, usize)> = info.tile_info.iter().map(|t| (t.time_ms, t.image_index)).collect();
        assert_eq!(order, vec![(1000, 1), (1000, 2), (3000, 0)]);
    }

    #[test]
    fn only_the_first_two_image_tiles_are_preloaded() {
        let info = ThumbnailInfo { tile_info: vec![], image_tile_urls: vec!["a".into(), "b".into(), "c".into()] };
        assert_eq!(info.preload_urls(), ["a".to_owned(), "b".to_owned()]);
        let single = ThumbnailInfo { tile_info: vec![], image_tile_urls: vec!["a".into()] };
        assert_eq!(single.preload_urls().len(), 1);
        assert!(ThumbnailInfo::default().preload_urls().is_empty());
    }
}
