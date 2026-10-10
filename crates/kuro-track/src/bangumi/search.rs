//! Bangumi subject to search result (T-31a-024, `dto/BGMSearch.kt`, `BangumiApi.kt:152`). A subject with no rating has
//! no score (`None`, shown as "-"). The source uses -1.0 for that.

use super::dto::BgmSubject;
use crate::model::TrackerId;
use crate::model::search::{AnimeTrackSearch, MangaTrackSearch};

/// The page of a subject on the Bangumi website (`BGMSearch.kt`, `tracking_url`).
fn page_url(id: i64) -> String {
    format!("https://bangumi.tv/subject/{id}")
}

/// Manga search keeps only subjects whose platform is null or `漫画`. Novels and other platforms are dropped.
pub fn is_manga(s: &BgmSubject) -> bool {
    s.platform.as_deref().is_none_or(|p| p == "漫画")
}

/// The Chinese name when it is not blank, otherwise the original name (`ifBlank`, EC-B-25).
pub fn title(s: &BgmSubject) -> String {
    if s.name_cn.trim().is_empty() {
        s.name.clone()
    } else {
        s.name_cn.clone()
    }
}

/// With a Chinese name, the original name is written first (`作品原名：`) and the trimmed summary follows on a new line
/// (EC-B-26, EC-B-27). Without one, the trimmed summary alone (EC-B-25).
pub fn summary(s: &BgmSubject) -> String {
    let trimmed = s.summary.as_deref().map(str::trim);
    if s.name_cn.trim().is_empty() {
        return trimmed.unwrap_or_default().to_owned();
    }
    let tail = trimmed.map(|t| format!("\n{t}")).unwrap_or_default();
    format!("作品原名：{}{tail}", s.name)
}

fn cover(s: &BgmSubject) -> String {
    s.images
        .as_ref()
        .and_then(|i| i.common.clone())
        .unwrap_or_default()
}

fn score(s: &BgmSubject) -> Option<f64> {
    s.rating.as_ref().and_then(|r| r.score)
}

/// The manga search result of a subject.
pub fn to_manga_search(s: &BgmSubject) -> MangaTrackSearch {
    let mut out = MangaTrackSearch::new(TrackerId::BANGUMI, s.id, title(s), page_url(s.id));
    out.total_chapters = s.eps;
    out.score = score(s);
    out.cover_url = cover(s);
    out.summary = summary(s);
    out.start_date = s.date.clone().unwrap_or_default();
    out
}

/// The anime search result of a subject.
pub fn to_anime_search(s: &BgmSubject) -> AnimeTrackSearch {
    let mut out = AnimeTrackSearch::new(TrackerId::BANGUMI, s.id, title(s), page_url(s.id));
    out.total_episodes = s.eps;
    out.score = score(s);
    out.cover_url = cover(s);
    out.summary = summary(s);
    out.start_date = s.date.clone().unwrap_or_default();
    out
}
