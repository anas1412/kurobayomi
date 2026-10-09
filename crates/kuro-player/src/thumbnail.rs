//! Seek preview thumbnails (area 45 `PlayerViewModel.updateSeekPos` and `createThumbnail`, T-45b-025;
//! area 46 `ThumbnailPreview.kt`, T-46a-013). Pure: the GTK layer fetches tile images, hands them in
//! with `SeekPreview::loaded`, and draws the card at the position from `thumb_x` and `card_size`.
//!
//! Rules kept from the Kotlin: a tile cache of three entries that evicts the least recently used one
//! (E-46-042); a 2 s fetch throttle that answers `Retry` instead of dropping the request, and a
//! pending seek that is fetched once the window ends (E-20, E-46-041); a result tagged with an
//! older episode generation is ignored (race 5); crops are clamped to the tile and an empty crop
//! gives no thumbnail (E-28); tiles that point at a missing image are dropped (E-30).

use crate::chapters::Segment;
use crate::types::{ThumbnailInfo, TileInfo};

/// Card height in dp (`124.dp`).
pub const CARD_HEIGHT: f32 = 124.0;
/// Card width for 16:9 and wider images in dp (`220.dp`).
pub const CARD_WIDTH: f32 = 220.0;
/// Gap kept between the card and the window edges in dp (`8.dp`).
pub const EDGE_GAP: f32 = 8.0;

/// Fixed-size LRU of at most `CAPACITY` tiles, keyed by image index. The most recently used entry
/// is last.
#[derive(Debug)]
pub struct TileLru<V> {
    entries: Vec<(i32, V)>,
}

impl<V> TileLru<V> {
    pub const CAPACITY: usize = 3;

    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn contains(&self, key: i32) -> bool {
        self.entries.iter().any(|(k, _)| *k == key)
    }

    /// Looks a tile up and makes it the most recently used one.
    pub fn get(&mut self, key: i32) -> Option<&V> {
        let at = self.entries.iter().position(|(k, _)| *k == key)?;
        let entry = self.entries.remove(at);
        self.entries.push(entry);
        self.entries.last().map(|(_, v)| v)
    }

    /// Stores a tile as the most recently used one, evicting the least recently used when full.
    pub fn insert(&mut self, key: i32, value: V) {
        self.entries.retain(|(k, _)| *k != key);
        self.entries.push((key, value));
        if self.entries.len() > Self::CAPACITY {
            self.entries.remove(0);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<V> Default for TileLru<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Answer of the throttle for one fetch request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// The request may go out now; the window restarts.
    Fetch,
    /// Within the window. The request may go out at `at_ms`.
    Retry { at_ms: u64 },
}

/// The 2 second fetch throttle (`lastThumbnailFetch`). Time is passed in as milliseconds so tests
/// can drive it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Throttle {
    last_ms: Option<u64>,
}

impl Throttle {
    pub const WINDOW_MS: u64 = 2000;

    pub fn check(&mut self, now_ms: u64) -> Gate {
        match self.last_ms {
            Some(last) if now_ms.saturating_sub(last) < Self::WINDOW_MS => Gate::Retry {
                at_ms: last + Self::WINDOW_MS,
            },
            _ => {
                self.last_ms = Some(now_ms);
                Gate::Fetch
            }
        }
    }
}

/// A rectangle inside a tile image, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The crop of one tile out of its image. The rectangle is clamped to the image, and `None` when
/// nothing is left (E-28).
pub fn crop(tile: &TileInfo, image_width: i32, image_height: i32) -> Option<Rect> {
    let (iw, ih) = (image_width.max(0), image_height.max(0));
    let x = tile.x.clamp(0, iw);
    let y = tile.y.clamp(0, ih);
    let width = tile.width.min(iw - x);
    let height = tile.height.min(ih - y);
    (width > 0 && height > 0).then_some(Rect {
        x,
        y,
        width,
        height,
    })
}

/// Card size in dp for the cropped image: 220 by 124, or 124 times the aspect ratio by 124 when the
/// image is narrower than 16:9.
pub fn card_size(image: Option<(i32, i32)>) -> (f32, f32) {
    match image {
        Some((w, h)) if w > 0 && h > 0 && (w as f32 / h as f32) < 16.0 / 9.0 => {
            (CARD_HEIGHT * (w as f32 / h as f32), CARD_HEIGHT)
        }
        _ => (CARD_WIDTH, CARD_HEIGHT),
    }
}

/// Left edge of the card so that it is centred on `target`, kept `EDGE_GAP` from each edge. A
/// window narrower than the card keeps the card at the start edge.
pub fn thumb_x(target: f32, width: f32, card_w: f32, start_inset: f32, end_inset: f32) -> f32 {
    let min_x = start_inset + EDGE_GAP;
    let max_x = width - card_w - end_inset - EDGE_GAP;
    let half = card_w / 2.0;
    let x = if target - half < min_x {
        min_x
    } else if target + half > width - end_inset - EDGE_GAP {
        max_x
    } else {
        target - half
    };
    let upper = (width - card_w - EDGE_GAP).max(min_x);
    x.max(min_x).min(upper)
}

/// The chapter name shown on the card: the last chapter in list order that starts at or before the
/// position (`lastOrNull`), and only when its name is not empty.
pub fn chapter_label_at(chapters: &[Segment], position: f32) -> Option<&str> {
    chapters
        .iter()
        .rev()
        .find(|c| c.start <= position)
        .map(|c| c.name.as_str())
        .filter(|name| !name.is_empty())
}

/// Result of a seek or a completed fetch.
#[derive(Clone, Debug, PartialEq)]
pub enum Preview<T> {
    /// Show `rect` cropped out of `tile`.
    Show { info: TileInfo, rect: Rect, tile: T },
    /// Fetch the tile image at `url` (image index `image_index`) for episode `generation`, then
    /// call `SeekPreview::loaded`.
    Fetch {
        generation: u64,
        image_index: i32,
        url: String,
    },
    /// Throttled. Call `SeekPreview::poll` at `at_ms` to send the pending request.
    Retry { at_ms: u64 },
    /// Nothing to show.
    Idle,
}

/// Seek preview state for one episode.
#[derive(Debug)]
pub struct SeekPreview<T> {
    /// Valid tiles sorted by time.
    tiles: Vec<TileInfo>,
    urls: Vec<String>,
    /// Tile image and its pixel size, keyed by image index.
    cache: TileLru<(T, i32, i32)>,
    generation: u64,
    throttle: Throttle,
    /// The tile for the position the user is on now.
    wanted: Option<TileInfo>,
    /// A tile whose fetch was throttled.
    pending: Option<TileInfo>,
}

impl<T: Clone> SeekPreview<T> {
    /// Builds the state for one episode. Tiles that point outside the URL list are dropped (E-30).
    pub fn new(info: ThumbnailInfo, generation: u64) -> Self {
        let urls = info.image_tile_urls;
        let mut tiles: Vec<TileInfo> = info
            .tile_info
            .into_iter()
            .filter(|t| t.image_index >= 0 && (t.image_index as usize) < urls.len())
            .collect();
        tiles.sort_by_key(|t| t.time_ms);
        Self {
            tiles,
            urls,
            cache: TileLru::new(),
            generation,
            throttle: Throttle::default(),
            wanted: None,
            pending: None,
        }
    }

    /// The first two tile images, fetched when the episode loads. Each is `(image_index, url)`.
    pub fn preload_targets(&self) -> Vec<(i32, String)> {
        self.urls
            .iter()
            .take(2)
            .enumerate()
            .map(|(i, url)| (i as i32, url.clone()))
            .collect()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Stores a fetched tile. A result from another generation is ignored (race 5). When it is the
    /// tile for the current position, the crop is returned.
    pub fn loaded(
        &mut self,
        generation: u64,
        image_index: i32,
        tile: T,
        image_width: i32,
        image_height: i32,
    ) -> Preview<T> {
        if generation != self.generation {
            return Preview::Idle;
        }
        self.cache
            .insert(image_index, (tile, image_width, image_height));
        match self.wanted {
            Some(info) if info.image_index == image_index => self.cached(info),
            _ => Preview::Idle,
        }
    }

    /// The preview for a seek position in seconds. Tiles come from the cache when possible.
    /// Otherwise the fetch goes out now, or is queued behind the throttle and reported as `Retry`.
    pub fn seek(&mut self, position_secs: f64, now_ms: u64) -> Preview<T> {
        let Some(info) = self
            .tiles
            .iter()
            .rev()
            .find(|t| t.time_ms as f64 <= position_secs * 1000.0)
            .copied()
        else {
            self.wanted = None;
            self.pending = None;
            return Preview::Idle;
        };
        self.wanted = Some(info);
        if self.cache.contains(info.image_index) {
            self.pending = None;
            return self.cached(info);
        }
        match self.throttle.check(now_ms) {
            Gate::Fetch => {
                self.pending = None;
                self.fetch(info)
            }
            Gate::Retry { at_ms } => {
                self.pending = Some(info);
                Preview::Retry { at_ms }
            }
        }
    }

    /// Sends the queued fetch once the throttle window has ended. Call at the `at_ms` of a `Retry`,
    /// and when the drag stops (E-46-041).
    pub fn poll(&mut self, now_ms: u64) -> Preview<T> {
        let Some(info) = self.pending else {
            return Preview::Idle;
        };
        if self.cache.contains(info.image_index) {
            self.pending = None;
            return self.cached(info);
        }
        match self.throttle.check(now_ms) {
            Gate::Fetch => {
                self.pending = None;
                self.fetch(info)
            }
            Gate::Retry { at_ms } => Preview::Retry { at_ms },
        }
    }

    /// The seek ended (`updateIsSeeking(false)`). Nothing is wanted any more, but a fetch that the
    /// throttle held back stays queued for the last position: call `poll` next, which sends it at
    /// once when the window has passed and answers `Retry` otherwise (E-46-041).
    pub fn end_seek(&mut self) {
        self.wanted = None;
    }

    fn cached(&mut self, info: TileInfo) -> Preview<T> {
        let Some((tile, w, h)) = self.cache.get(info.image_index).cloned() else {
            return Preview::Idle;
        };
        match crop(&info, w, h) {
            Some(rect) => Preview::Show { info, rect, tile },
            None => Preview::Idle,
        }
    }

    fn fetch(&self, info: TileInfo) -> Preview<T> {
        Preview::Fetch {
            generation: self.generation,
            image_index: info.image_index,
            url: self.urls[info.image_index as usize].clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ChapterType;

    fn tile(image_index: i32, time_ms: i64) -> TileInfo {
        TileInfo {
            image_index,
            time_ms,
            x: 0,
            y: 0,
            width: 160,
            height: 90,
        }
    }

    fn info(tiles: Vec<TileInfo>, urls: usize) -> ThumbnailInfo {
        ThumbnailInfo {
            tile_info: tiles,
            image_tile_urls: (0..urls).map(|i| format!("tile-{i}")).collect(),
        }
    }

    fn segment(name: &str, start: f32) -> Segment {
        Segment {
            index: 0,
            name: name.to_owned(),
            start,
            kind: ChapterType::Other,
        }
    }

    #[test]
    fn lru_keeps_three_and_evicts_the_least_recently_used() {
        let mut lru = TileLru::new();
        lru.insert(0, "a");
        lru.insert(1, "b");
        lru.insert(2, "c");
        assert_eq!(lru.get(0), Some(&"a"));
        lru.insert(3, "d");
        assert_eq!(lru.len(), TileLru::<&str>::CAPACITY);
        assert!(!lru.contains(1));
        assert!(lru.contains(0) && lru.contains(2) && lru.contains(3));
    }

    #[test]
    fn throttle_retries_inside_the_window_and_fetches_after() {
        let mut throttle = Throttle::default();
        assert_eq!(throttle.check(1000), Gate::Fetch);
        assert_eq!(throttle.check(2500), Gate::Retry { at_ms: 3000 });
        assert_eq!(throttle.check(3000), Gate::Fetch);
    }

    #[test]
    fn crop_is_clamped_and_empty_crops_give_none() {
        let mut t = tile(0, 0);
        t.x = 150;
        t.width = 40;
        assert_eq!(
            crop(&t, 160, 90),
            Some(Rect {
                x: 150,
                y: 0,
                width: 10,
                height: 90
            })
        );
        t.x = 400;
        assert_eq!(crop(&t, 160, 90), None);
        t.x = -5;
        t.width = -1;
        assert_eq!(crop(&t, 160, 90), None);
    }

    #[test]
    fn card_is_220_wide_except_for_narrow_images() {
        assert_eq!(card_size(Some((320, 180))), (220.0, 124.0));
        assert_eq!(card_size(None), (220.0, 124.0));
        let (w, h) = card_size(Some((100, 100)));
        assert!((w - 124.0).abs() < 1e-4);
        assert_eq!(h, 124.0);
    }

    #[test]
    fn thumb_x_is_centred_and_clamped_to_the_window() {
        assert_eq!(thumb_x(500.0, 1000.0, 220.0, 0.0, 0.0), 390.0);
        assert_eq!(thumb_x(10.0, 1000.0, 220.0, 0.0, 0.0), 8.0);
        assert_eq!(thumb_x(995.0, 1000.0, 220.0, 0.0, 0.0), 772.0);
        assert_eq!(thumb_x(50.0, 200.0, 220.0, 0.0, 0.0), 8.0);
    }

    #[test]
    fn chapter_label_uses_list_order_and_skips_empty_names() {
        let chapters = vec![
            segment("Intro", 0.0),
            segment("", 100.0),
            segment("End", 200.0),
        ];
        assert_eq!(chapter_label_at(&chapters, 150.0), None);
        assert_eq!(chapter_label_at(&chapters, 250.0), Some("End"));
        assert_eq!(chapter_label_at(&chapters, 50.0), Some("Intro"));
        assert_eq!(chapter_label_at(&[], 10.0), None);
    }

    #[test]
    fn invalid_tile_indexes_are_dropped_and_tiles_sorted() {
        let state: SeekPreview<u8> = SeekPreview::new(
            info(
                vec![
                    tile(1, 20_000),
                    tile(9, 10_000),
                    tile(0, 0),
                    tile(0, 10_000),
                ],
                2,
            ),
            1,
        );
        assert_eq!(state.tiles.len(), 3);
        assert_eq!(state.tiles[0].time_ms, 0);
        assert_eq!(state.tiles[2].time_ms, 20_000);
    }

    #[test]
    fn seek_fetches_then_shows_from_cache() {
        let mut state: SeekPreview<u8> =
            SeekPreview::new(info(vec![tile(0, 0), tile(1, 10_000)], 2), 1);
        assert_eq!(
            state.seek(12.0, 0),
            Preview::Fetch {
                generation: 1,
                image_index: 1,
                url: "tile-1".to_owned()
            }
        );
        let shown = state.loaded(1, 1, 7, 160, 90);
        assert!(matches!(shown, Preview::Show { tile: 7, .. }));
        assert!(matches!(
            state.seek(15.0, 100),
            Preview::Show { tile: 7, .. }
        ));
    }

    #[test]
    fn throttled_seek_is_queued_and_sent_when_the_window_ends() {
        let mut state: SeekPreview<u8> = SeekPreview::new(
            info(vec![tile(0, 0), tile(1, 10_000), tile(2, 20_000)], 3),
            1,
        );
        assert!(matches!(
            state.seek(1.0, 0),
            Preview::Fetch { image_index: 0, .. }
        ));
        assert_eq!(state.seek(11.0, 500), Preview::Retry { at_ms: 2000 });
        assert_eq!(state.seek(21.0, 1000), Preview::Retry { at_ms: 2000 });
        assert!(matches!(
            state.poll(2000),
            Preview::Fetch { image_index: 2, .. }
        ));
    }

    #[test]
    fn result_from_an_older_episode_is_ignored() {
        let mut state: SeekPreview<u8> = SeekPreview::new(info(vec![tile(0, 0)], 1), 2);
        let _ = state.seek(1.0, 0);
        assert_eq!(state.loaded(1, 0, 9, 160, 90), Preview::Idle);
        assert!(state.cache.is_empty());
    }

    #[test]
    fn preload_takes_the_first_two_urls() {
        let state: SeekPreview<u8> = SeekPreview::new(info(vec![], 3), 1);
        assert_eq!(
            state.preload_targets(),
            vec![(0, "tile-0".to_owned()), (1, "tile-1".to_owned())]
        );
    }

    #[test]
    fn end_seek_keeps_the_throttled_fetch_for_the_last_position() {
        let mut state: SeekPreview<u8> = SeekPreview::new(
            info(vec![tile(0, 0), tile(1, 10_000), tile(2, 20_000)], 3),
            1,
        );
        let _ = state.seek(1.0, 0);
        assert_eq!(state.seek(11.0, 10), Preview::Retry { at_ms: 2000 });
        assert_eq!(state.seek(21.0, 20), Preview::Retry { at_ms: 2000 });
        state.end_seek();
        assert_eq!(state.poll(500), Preview::Retry { at_ms: 2000 });
        assert!(matches!(
            state.poll(3000),
            Preview::Fetch { image_index: 2, .. }
        ));
        assert_eq!(state.poll(4000), Preview::Idle);
    }

    #[test]
    fn end_seek_without_a_throttled_fetch_has_nothing_to_send() {
        let mut state: SeekPreview<u8> = SeekPreview::new(info(vec![tile(0, 0)], 1), 1);
        let _ = state.seek(1.0, 0);
        state.end_seek();
        assert_eq!(state.poll(5000), Preview::Idle);
    }
}
