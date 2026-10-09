//! Info box rules of the anime entry (T-39a-023, T-39a-025, T-39a-063). Title, author and artist text reuse the manga
//! rules in `entry_core::text`, because they are identical. Upstream: `AnimeInfoHeader.kt:433-579` (text block),
//! `AnimeSourceExtensions.kt:18-31` (source name).

use kuro_data::anime::Anime;
use kuro_i18n::{MsgKey, tr};

use crate::entry_core::detail::SourceInfo;
use crate::entry_core::header::HeaderModel;

/// The icon and label of a status value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusView {
    /// Adwaita symbolic icon name. The widget checks it with `gtk::IconTheme::has_icon`.
    pub icon: &'static str,
    pub text: String,
}

/// `SAnime` status values 0..7 (behaviour 6.3). Any other value shows "Unknown" with the block icon.
pub fn status(value: i64) -> StatusView {
    let (icon, key) = match value {
        1 => ("document-open-recent-symbolic", MsgKey::ongoing),
        2 => ("emblem-ok-symbolic", MsgKey::completed),
        3 => ("money-symbolic", MsgKey::licensed),
        4 => ("object-select-symbolic", MsgKey::publishing_finished),
        5 => ("window-close-symbolic", MsgKey::cancelled),
        6 => ("media-playback-pause-symbolic", MsgKey::on_hiatus),
        7 => ("x-office-calendar-symbolic", MsgKey::upcoming),
        _ => ("action-unavailable-symbolic", MsgKey::unknown),
    };
    StatusView { icon, text: tr(key) }
}

/// The info box lines of an anime: title, author and artist with their placeholders (`entry_core::text`), the anime
/// status table, and the source line with its stub flag. The widget is `entry_core::header_view`, shared with manga.
pub fn header_model(anime: &Anime, source: &SourceInfo) -> HeaderModel {
    let shown = status(anime.status);
    HeaderModel::from_parts(
        &anime.title,
        anime.author.as_deref(),
        anime.artist.as_deref(),
        (shown.icon, shown.text),
        &source.name,
        source.stub,
    )
}

/// `getNameForAnimeInfo`. `enabled` is the enabled language list. The `all` and `other` pseudo-languages are ignored.
/// With exactly one enabled language, a source in that language shows only its name. Every other case shows
/// `"{name} ({LANG})"` with the language code upper-cased.
pub fn source_display_name(name: &str, lang: &str, enabled: &[&str]) -> String {
    let enabled: Vec<&str> = enabled
        .iter()
        .copied()
        .filter(|l| *l != "all" && *l != "other")
        .collect();
    if enabled.len() == 1 && enabled.contains(&lang) {
        name.to_owned()
    } else {
        format!("{name} ({})", lang.to_uppercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_values_map_to_the_spec_labels() {
        assert_eq!(status(1).text, tr(MsgKey::ongoing));
        assert_eq!(status(2).text, tr(MsgKey::completed));
        assert_eq!(status(7).text, tr(MsgKey::upcoming));
        assert_eq!(status(7).icon, "x-office-calendar-symbolic");
    }

    #[test]
    fn unknown_and_out_of_range_status_fall_back_to_unknown() {
        for v in [0, 8, -1, 999] {
            assert_eq!(status(v).text, tr(MsgKey::unknown), "value {v}");
            assert_eq!(status(v).icon, "action-unavailable-symbolic", "value {v}");
        }
    }

    #[test]
    fn the_info_box_uses_the_placeholders_the_anime_status_and_the_stub_flag() {
        let anime = Anime { status: 7, ..Anime::create() };
        let model = header_model(&anime, &SourceInfo { name: "Src".into(), stub: true });
        assert_eq!(model.title, tr(MsgKey::unknown_title));
        assert_eq!(model.author, tr(MsgKey::unknown_author));
        assert_eq!(model.artist, None);
        assert_eq!((model.status_icon, model.status.as_str()), ("x-office-calendar-symbolic", tr(MsgKey::upcoming).as_str()));
        assert!(model.stub);
        let known = Anime { title: "Frieren".into(), author: Some("A".into()), artist: Some("A".into()), ..Anime::create() };
        let model = header_model(&known, &SourceInfo::default());
        assert_eq!((model.title.as_str(), model.author.as_str(), model.artist), ("Frieren", "A", None));
        assert!(!model.stub);
    }

    #[test]
    fn source_name_table_t_sn1() {
        assert_eq!(source_display_name("Src", "en", &["en"]), "Src");
        assert_eq!(source_display_name("Src", "ja", &["en"]), "Src (JA)");
        assert_eq!(source_display_name("Src", "en", &["en", "ja"]), "Src (EN)");
        assert_eq!(source_display_name("Src", "en", &["all", "other"]), "Src (EN)");
    }

    #[test]
    fn single_enabled_language_ignores_the_pseudo_languages() {
        assert_eq!(source_display_name("Src", "en", &["all", "en", "other"]), "Src");
        assert_eq!(source_display_name("Src", "de", &["all", "en"]), "Src (DE)");
    }
}
