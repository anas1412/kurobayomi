//! Episodes of an online anime source (KickAssAnime and the recipe sources). The source's best video is resolved off
//! the GTK thread through the `kuro-source` loader (the source's own hoster and video order, then the first preferred
//! or playable video), and the window plays its URL with the request headers, subtitles and audio tracks it returned.
//! Progress is saved by episode row, as for a local episode. A failure is a toast.

use std::sync::Arc;

use gtk::glib;
use kuro_data::anime::AnimeRepo;
use kuro_data::episode::EpisodeRepo;
use kuro_domain::source::remote_update::to_s_episode;
use kuro_i18n::{MsgKey, tr};
use kuro_player::load::http_header_fields;
use kuro_player::tracks::external_track_commands;
use kuro_source::anime::{AnimeSource, Hoster, SEpisode, Track, Video, best_video};

use super::Media;

thread_local! {
    /// An episode of a source is being resolved: further clicks wait for it instead of opening more players.
    static RESOLVING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Plays episode row `episode_id` when its entry belongs to one of `sources`. `false` when it does not (or the row is
/// missing), so the caller keeps its own player. The video is resolved in the background; a "Loading…" toast shows
/// meanwhile and the window opens when it is ready.
pub fn open_source_episode(sources: &[Arc<dyn AnimeSource>], episode_id: i64) -> bool {
    if !crate::data::is_installed() {
        return false;
    }
    let db = &crate::data::app().anime_db;
    let found = EpisodeRepo::new(Arc::clone(db))
        .by_id(episode_id)
        .and_then(|episode| episode.map(|e| AnimeRepo::new(Arc::clone(db)).by_id(e.anime_id).map(|a| (a, e))).transpose());
    let (anime, episode) = match found {
        Ok(Some(found)) => found,
        Ok(None) => return false,
        Err(e) => {
            tracing::error!(error = %e, "player: the episode lookup failed");
            return false;
        }
    };
    let Some(source) = sources.iter().find(|s| s.id() == anime.source).cloned() else {
        return false;
    };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::error!("player: no async runtime on this thread; the source episode is not opened");
        return false;
    };
    // a second click while the first still loads, or while a player is open, does nothing: the first answer opens it
    if super::is_open() || RESOLVING.with(|r| r.replace(true)) {
        return true;
    }
    let (tx, rx) = async_channel::bounded(1);
    let s_episode = to_s_episode(&episode);
    handle.spawn(async move {
        let _ = tx.send(best_stream(source.as_ref(), &s_episode).await).await;
    });
    let busy = crate::toast::busy(&tr(MsgKey::loading));
    let title = format!("{} - {}", anime.title, episode.name);
    glib::spawn_future_local(async move {
        // a closed channel means the source task panicked
        let answer = rx.recv().await.unwrap_or_else(|_| Err(tr(MsgKey::video_list_empty_error)));
        drop(busy);
        RESOLVING.with(|r| r.set(false));
        match answer {
            Ok(video) => {
                super::open_media(media_of(video, title), episode_id, 0);
            }
            Err(text) => crate::toast::notify(text),
        }
    });
    true
}

/// The video to play: the source's hosters (or its plain video list, as one hoster) through `best_video`. The error
/// is the text of the toast.
async fn best_stream(source: &dyn AnimeSource, episode: &SEpisode) -> Result<Video, String> {
    let hosters = if source.provides_hosters() {
        source.sort_hosters(source.hosters(episode).await.map_err(|e| e.to_string())?)
    } else {
        vec![Hoster::synthetic(source.videos_for_episode(episode).await.map_err(|e| e.to_string())?)]
    };
    if hosters.is_empty() {
        return Err(tr(MsgKey::no_hosters));
    }
    best_video(source, &hosters)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| tr(MsgKey::video_list_empty_error))
}

/// Side tracks are added with the `auto` flag, which selects a track only when it matches `alang` or `slang`; the source's
/// tracks carry a display name and no language code, so nothing matched and the video played without sound or subtitles.
/// The first audio track (the source puts its default first) and the English subtitle (else the first one) are selected.
fn select_defaults(mut commands: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let first_audio = commands.iter().position(|c| c[0] == "audio-add");
    let subs: Vec<usize> = commands.iter().enumerate().filter(|(_, c)| c[0] == "sub-add").map(|(i, _)| i).collect();
    let english = subs.iter().copied().find(|&i| commands[i].get(3).is_some_and(|name| name.to_lowercase().starts_with("english") || name.to_lowercase().starts_with("en")));
    for i in first_audio.into_iter().chain(english.or_else(|| subs.first().copied())) {
        commands[i][2] = "select".to_owned();
    }
    commands
}

fn media_of(video: Video, title: String) -> Media {
    let tracks = |list: &[Track]| list.iter().map(|t| kuro_player::types::Track { url: t.url.clone(), lang: t.lang.clone() }).collect::<Vec<_>>();
    let headers = http_header_fields(true, video.headers.as_ref().map(|h| h.0.as_slice()), &[]).filter(|h| !h.is_empty());
    Media {
        tracks: select_defaults(external_track_commands(&tracks(&video.audio_tracks), &tracks(&video.subtitle_tracks)).commands),
        target: video.video_url,
        title,
        headers,
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use kuro_source::SourceError;
    use kuro_source::anime::{AnimesPage, Headers};
    use kuro_source::filter_list::FilterList;

    use super::super::playback::load_commands;
    use super::*;

    /// A source over fixed hosters. `fail` makes `hosters` fail; `sort_videos` puts the highest resolution first, as
    /// KickAssAnime's quality order does.
    struct Fake {
        hosters: Vec<Hoster>,
        fail: bool,
    }

    #[async_trait]
    impl AnimeSource for Fake {
        fn id(&self) -> i64 {
            1
        }
        fn name(&self) -> &str {
            "Fake"
        }
        fn supports_latest(&self) -> bool {
            false
        }
        async fn popular(&self, _: i32) -> Result<AnimesPage, SourceError> {
            unreachable!()
        }
        async fn latest(&self, _: i32) -> Result<AnimesPage, SourceError> {
            unreachable!()
        }
        async fn search(&self, _: i32, _: &str, _: &FilterList) -> Result<AnimesPage, SourceError> {
            unreachable!()
        }
        fn provides_hosters(&self) -> bool {
            true
        }
        async fn hosters(&self, _: &SEpisode) -> Result<Vec<Hoster>, SourceError> {
            if self.fail { Err(SourceError::Parse("HTTP 503".into())) } else { Ok(self.hosters.clone()) }
        }
        fn sort_videos(&self, mut videos: Vec<Video>) -> Vec<Video> {
            videos.sort_by_key(|v| std::cmp::Reverse(v.resolution));
            videos
        }
    }

    fn video(url: &str, resolution: i32) -> Video {
        Video {
            resolution: Some(resolution),
            headers: Some(Headers(vec![
                ("Origin".into(), "https://krussdomi.com".into()),
                ("User-Agent".into(), "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko)".into()),
            ])),
            subtitle_tracks: vec![Track { url: "https://cdn.test/en.vtt".into(), lang: "English".into() }],
            audio_tracks: vec![Track { url: "https://cdn.test/ja.m3u8".into(), lang: "Japanese".into() }],
            ..Video::new(url, format!("{resolution}p"))
        }
    }

    fn resolve(fake: &Fake) -> Result<Video, String> {
        let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        rt.block_on(best_stream(fake, &SEpisode::default()))
    }

    #[test]
    fn the_first_audio_and_the_english_subtitle_are_selected_the_rest_stay_available() {
        let strings = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let cmds = vec![
            strings(&["audio-add", "a1", "auto", "Japanese"]),
            strings(&["audio-add", "a2", "auto", "English"]),
            strings(&["sub-add", "s1", "auto", "French (fr)"]),
            strings(&["sub-add", "s2", "auto", "English (en)"]),
        ];
        let out = select_defaults(cmds);
        let flags: Vec<&str> = out.iter().map(|c| c[2].as_str()).collect();
        assert_eq!(flags, ["select", "auto", "auto", "select"]);
        // no English: the first subtitle is selected; no side tracks: nothing changes
        let only_fr = select_defaults(vec![strings(&["sub-add", "s1", "auto", "French (fr)"])]);
        assert_eq!(only_fr[0][2], "select");
        assert!(select_defaults(Vec::new()).is_empty());
    }

    #[test]
    fn the_best_quality_plays_with_the_source_headers_and_side_tracks() {
        let hoster = |name: &str, videos| Hoster { hoster_name: name.into(), video_list: Some(videos), ..Hoster::default() };
        let fake = Fake {
            hosters: vec![hoster("VidStreaming", vec![video("https://cdn.test/480.m3u8", 480), video("https://cdn.test/1080.m3u8", 1080)])],
            fail: false,
        };
        let media = media_of(resolve(&fake).expect("a video"), "Show - 1".into());
        assert_eq!(media.target, "https://cdn.test/1080.m3u8", "the source's best quality is the default");

        let headers = "Origin: https://krussdomi.com,User-Agent: Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML\\, like Gecko)";
        assert_eq!(media.headers.as_deref(), Some(headers), "commas inside a value are escaped for the mpv list");
        assert_eq!(
            load_commands(&media.target, media.headers.as_deref(), 0),
            vec![
                kuro_player::load::start_command_ms(0),
                vec!["set".to_owned(), "http-header-fields".to_owned(), headers.to_owned()],
                vec!["loadfile".to_owned(), "https://cdn.test/1080.m3u8".to_owned(), "replace".to_owned()],
            ],
            "the headers are set before loadfile, which has no index argument (mpv before 0.38)"
        );
        let strings = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            media.tracks,
            vec![
                strings(&["audio-add", "https://cdn.test/ja.m3u8", "select", "Japanese"]),
                strings(&["sub-add", "https://cdn.test/en.vtt", "select", "English"]),
            ]
        );
    }

    #[test]
    fn a_video_without_headers_sets_none() {
        let mut plain = video("https://cdn.test/a.m3u8", 720);
        plain.headers = None;
        assert_eq!(media_of(plain, String::new()).headers, None);
    }

    #[test]
    fn a_failed_or_empty_video_list_is_the_toast_text() {
        assert_eq!(resolve(&Fake { hosters: Vec::new(), fail: true }).unwrap_err(), SourceError::Parse("HTTP 503".into()).to_string());
        assert_eq!(resolve(&Fake { hosters: Vec::new(), fail: false }).unwrap_err(), tr(MsgKey::no_hosters));
        let empty = Fake { hosters: vec![Hoster { hoster_name: "A".into(), video_list: Some(Vec::new()), ..Hoster::default() }], fail: false };
        assert_eq!(resolve(&empty).unwrap_err(), tr(MsgKey::video_list_empty_error));
    }
}
