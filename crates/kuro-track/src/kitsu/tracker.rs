//! Kitsu behind the tracker traits (T-30a-019 to T-30a-023, T-30a-026). The registry and the track sheet hold it as
//! `Arc<dyn Tracker>`. Every call maps the fields of a track onto the rules of [`super::rules`] and then onto the REST
//! calls of [`KitsuApi`].
//!
//! Concurrency (T-30a-026): bind, update, refresh and delete on one library entry run one at a time, through a lock
//! per `(kind, id)`. The id is the track's `remote_id`, which is the media id before a bind and the library-entry id
//! after it. A bind cancelled mid-flight drops this future, so the caller runs bind on a spawned task when it must
//! finish (the GUI bridge, outside this crate).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use kuro_platform::secrets::SecretStore;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard, watch};

use super::api::{Endpoints, KitsuApi};
use super::error::KitsuError;
use super::rules::{self, Personal};
use super::score;
use super::search;
use super::{COMPLETED, NAME, PLAN_TO_READ, PLAN_TO_WATCH, READING, WATCHING};
use crate::base::{Field, SECRET_SERVICE, account};
use crate::error::TrackError;
use crate::messages::MsgKey;
use crate::model::search::{AnimeTrackSearch, MangaTrackSearch};
use crate::model::{AnimeTrack, Kind, MangaTrack, TrackerId};
use crate::tracker::{
    AnimeTracker, DeletableAnimeTracker, DeletableMangaTracker, LogoRef, MangaTracker, Tracker,
};

/// Kitsu brand colour `#F75239`, `0xAARRGGBB`.
pub const LOGO_COLOR: u32 = 0xFFF7_5239;

impl From<KitsuError> for TrackError {
    /// The error text is the toast, unchanged (T-30a-024). The stable key is kept for the localisation pass.
    fn from(e: KitsuError) -> Self {
        TrackError::Key(MsgKey::Raw(e.to_string()))
    }
}

pub struct KitsuTracker {
    api: KitsuApi,
    secrets: Arc<dyn SecretStore>,
    login: watch::Sender<bool>,
    labels: Vec<String>,
    locks: Mutex<HashMap<(Kind, i64), Arc<AsyncMutex<()>>>>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn anime_personal(t: &AnimeTrack) -> Personal {
    Personal {
        progress: t.last_episode_seen,
        score: t.score,
        status: t.status,
        start_ms: t.start_date,
        finish_ms: t.finish_date,
        private: t.private,
    }
}

fn set_anime(t: &mut AnimeTrack, p: &Personal) {
    t.last_episode_seen = p.progress;
    t.score = p.score;
    t.status = p.status;
    t.start_date = p.start_ms;
    t.finish_date = p.finish_ms;
    t.private = p.private;
}

fn manga_personal(t: &MangaTrack) -> Personal {
    Personal {
        progress: t.last_chapter_read,
        score: t.score,
        status: t.status,
        start_ms: t.start_date,
        finish_ms: t.finish_date,
        private: t.private,
    }
}

fn set_manga(t: &mut MangaTrack, p: &Personal) {
    t.last_chapter_read = p.progress;
    t.score = p.score;
    t.status = p.status;
    t.start_date = p.start_ms;
    t.finish_date = p.finish_ms;
    t.private = p.private;
}

impl KitsuTracker {
    pub fn new(endpoints: Endpoints, secrets: Arc<dyn SecretStore>) -> Result<Self, KitsuError> {
        let api = KitsuApi::new(endpoints, secrets.clone())?;
        let logged_in = api.has_session();
        Ok(Self {
            api,
            secrets,
            login: watch::channel(logged_in).0,
            labels: score::SCORE_LIST.iter().map(|s| (*s).to_owned()).collect(),
            locks: Mutex::new(HashMap::new()),
        })
    }

    pub fn api(&self) -> &KitsuApi {
        &self.api
    }

    /// The lock of one library entry. Waits for the entry's previous operation to finish.
    pub(super) async fn lock_entry(&self, kind: Kind, id: i64) -> OwnedMutexGuard<()> {
        let slot = self
            .locks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry((kind, id))
            .or_default()
            .clone();
        slot.lock_owned().await
    }

    /// Bind (section 7): a found entry is copied and pushed, a new one is added with the status of its progress.
    async fn bind_entry(
        &self,
        kind: Kind,
        media_id: i64,
        local: Personal,
        has_progress: bool,
        progress_status: (i64, i64),
    ) -> Result<(i64, Personal), TrackError> {
        let (current, planned) = progress_status;
        let found = self.api.find_entry(kind, media_id).await?;
        match found {
            Some(found) => {
                let remote = rules::remote_personal(kind, &found)?;
                let mut p = local;
                rules::bind_found(&mut p, remote, has_progress, current);
                let entry = found.entry.id;
                self.api
                    .update_entry(kind, entry, &p.entry_fields())
                    .await?;
                Ok((entry, p))
            }
            None => {
                let mut p = local;
                p.status = rules::bind_new_status(has_progress, current, planned);
                p.score = 0.0;
                let entry = self
                    .api
                    .add_entry(kind, media_id, &p.entry_fields())
                    .await?;
                Ok((entry, p))
            }
        }
    }
}

#[async_trait]
impl Tracker for KitsuTracker {
    fn id(&self) -> TrackerId {
        TrackerId::KITSU
    }

    fn name(&self) -> &str {
        NAME
    }

    fn logo(&self) -> LogoRef {
        LogoRef::IconName("kitsu")
    }

    fn logo_color(&self) -> u32 {
        LOGO_COLOR
    }

    fn supports_reading_dates(&self) -> bool {
        super::supports_reading_dates()
    }

    fn supports_private_tracking(&self) -> bool {
        super::supports_private_tracking()
    }

    fn completion_status(&self) -> i64 {
        COMPLETED
    }

    fn score_list(&self) -> &[String] {
        &self.labels
    }

    async fn login(&self, username: &str, password: &str) -> Result<(), TrackError> {
        self.api.login(username, password).await?;
        self.login.send_replace(true);
        Ok(())
    }

    fn logout(&self) -> Result<(), TrackError> {
        self.api.logout()?;
        self.login.send_replace(false);
        Ok(())
    }

    fn is_logged_in(&self) -> bool {
        self.api.has_session()
    }

    fn auth_expired(&self) -> bool {
        self.secrets
            .get(SECRET_SERVICE, &account(TrackerId::KITSU, Field::Expired))
            .ok()
            .flatten()
            .as_deref()
            == Some("true")
    }

    fn login_state(&self) -> watch::Receiver<bool> {
        self.login.subscribe()
    }

    /// The password is never stored (KT-23), so there is nothing to save here.
    fn save_credentials(&self, _username: &str, _password: &str) -> Result<(), TrackError> {
        Ok(())
    }

    fn username(&self) -> String {
        self.secrets
            .get(SECRET_SERVICE, &account(TrackerId::KITSU, Field::Username))
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    fn password(&self) -> String {
        String::new()
    }

    fn as_anime(&self) -> Option<&dyn AnimeTracker> {
        Some(self)
    }

    fn as_manga(&self) -> Option<&dyn MangaTracker> {
        Some(self)
    }

    fn removes_local_first(&self) -> bool {
        true
    }

    fn as_deletable_anime(&self) -> Option<&dyn DeletableAnimeTracker> {
        Some(self)
    }

    fn as_deletable_manga(&self) -> Option<&dyn DeletableMangaTracker> {
        Some(self)
    }
}

#[async_trait]
impl AnimeTracker for KitsuTracker {
    fn status_list(&self) -> Vec<i64> {
        super::status_list(Kind::Anime).to_vec()
    }

    fn watching_status(&self) -> i64 {
        WATCHING
    }

    /// Kitsu has no rewatching status. `-1` matches no real code.
    fn rewatching_status(&self) -> i64 {
        -1
    }

    fn status_label(&self, status: i64) -> Option<&'static str> {
        super::status_label(Kind::Anime, status)
    }

    fn index_to_score(&self, index: usize) -> f64 {
        score::index_to_score(index)
    }

    fn display_score(&self, t: &AnimeTrack) -> String {
        score::display_score(t.score)
    }

    async fn update(
        &self,
        t: &AnimeTrack,
        did_watch_episode: bool,
    ) -> Result<AnimeTrack, TrackError> {
        let _entry = self.lock_entry(Kind::Anime, t.remote_id).await;
        let mut p = anime_personal(t);
        rules::apply_progress_event(
            &mut p,
            t.total_episodes,
            did_watch_episode,
            WATCHING,
            now_ms(),
        );
        self.api
            .update_entry(Kind::Anime, t.remote_id, &p.entry_fields())
            .await?;
        let mut out = t.clone();
        set_anime(&mut out, &p);
        Ok(out)
    }

    async fn bind(&self, t: &mut AnimeTrack, has_seen: bool) -> Result<(), TrackError> {
        let _entry = self.lock_entry(Kind::Anime, t.remote_id).await;
        let (entry, p) = self
            .bind_entry(
                Kind::Anime,
                t.remote_id,
                anime_personal(t),
                has_seen,
                (WATCHING, PLAN_TO_WATCH),
            )
            .await?;
        t.remote_id = entry;
        set_anime(t, &p);
        Ok(())
    }

    async fn search(&self, query: &str) -> Result<Vec<AnimeTrackSearch>, TrackError> {
        let hits = self.api.search_media(Kind::Anime, query).await?;
        Ok(hits.iter().map(search::to_anime_search).collect())
    }

    /// Copies the remote personal fields, private included, and the remote episode count (null is 0). The title,
    /// tracking URL and ids stay (section 9).
    async fn refresh(&self, t: &AnimeTrack) -> Result<AnimeTrack, TrackError> {
        let _entry = self.lock_entry(Kind::Anime, t.remote_id).await;
        let found = self.api.get_entry(Kind::Anime, t.remote_id).await?;
        let remote = rules::remote_personal(Kind::Anime, &found)?;
        let mut out = t.clone();
        set_anime(&mut out, &remote);
        out.total_episodes = found.media.attributes.episode_count.unwrap_or(0);
        Ok(out)
    }
}

#[async_trait]
impl MangaTracker for KitsuTracker {
    fn status_list(&self) -> Vec<i64> {
        super::status_list(Kind::Manga).to_vec()
    }

    fn reading_status(&self) -> i64 {
        READING
    }

    /// Kitsu has no rereading status. `-1` matches no real code.
    fn rereading_status(&self) -> i64 {
        -1
    }

    fn status_label(&self, status: i64) -> Option<&'static str> {
        super::status_label(Kind::Manga, status)
    }

    fn index_to_score(&self, index: usize) -> f64 {
        score::index_to_score(index)
    }

    fn display_score(&self, t: &MangaTrack) -> String {
        score::display_score(t.score)
    }

    async fn update(
        &self,
        t: &MangaTrack,
        did_read_chapter: bool,
    ) -> Result<MangaTrack, TrackError> {
        let _entry = self.lock_entry(Kind::Manga, t.remote_id).await;
        let mut p = manga_personal(t);
        rules::apply_progress_event(
            &mut p,
            t.total_chapters,
            did_read_chapter,
            READING,
            now_ms(),
        );
        self.api
            .update_entry(Kind::Manga, t.remote_id, &p.entry_fields())
            .await?;
        let mut out = t.clone();
        set_manga(&mut out, &p);
        Ok(out)
    }

    async fn bind(&self, t: &mut MangaTrack, has_read: bool) -> Result<(), TrackError> {
        let _entry = self.lock_entry(Kind::Manga, t.remote_id).await;
        let (entry, p) = self
            .bind_entry(
                Kind::Manga,
                t.remote_id,
                manga_personal(t),
                has_read,
                (READING, PLAN_TO_READ),
            )
            .await?;
        t.remote_id = entry;
        set_manga(t, &p);
        Ok(())
    }

    async fn search(&self, query: &str) -> Result<Vec<MangaTrackSearch>, TrackError> {
        let hits = self.api.search_media(Kind::Manga, query).await?;
        Ok(hits.iter().map(search::to_manga_search).collect())
    }

    async fn refresh(&self, t: &MangaTrack) -> Result<MangaTrack, TrackError> {
        let _entry = self.lock_entry(Kind::Manga, t.remote_id).await;
        let found = self.api.get_entry(Kind::Manga, t.remote_id).await?;
        let remote = rules::remote_personal(Kind::Manga, &found)?;
        let mut out = t.clone();
        set_manga(&mut out, &remote);
        out.total_chapters = found.media.attributes.chapter_count.unwrap_or(0);
        Ok(out)
    }
}

#[async_trait]
impl DeletableAnimeTracker for KitsuTracker {
    async fn delete(&self, t: &AnimeTrack) -> Result<(), TrackError> {
        let _entry = self.lock_entry(Kind::Anime, t.remote_id).await;
        Ok(self.api.remove_entry(Kind::Anime, t.remote_id).await?)
    }
}

#[async_trait]
impl DeletableMangaTracker for KitsuTracker {
    async fn delete(&self, t: &MangaTrack) -> Result<(), TrackError> {
        let _entry = self.lock_entry(Kind::Manga, t.remote_id).await;
        Ok(self.api.remove_entry(Kind::Manga, t.remote_id).await?)
    }
}
