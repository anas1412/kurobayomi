//! The WASM source host: compiles a package, runs guest calls behind a mutex and an epoch deadline, and presents each
//! source as a `CatalogueSource` (T-18a-006, T-18a-008).
//!
//! Each source owns one guest instance. Calls to that source are serialised by its mutex (R5). A call that traps drops
//! the instance, and the next call starts a fresh one, so a trapped instance is never entered again.

use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use kuro_net::NetworkClient;
use kuro_prefs::store::PrefStore;
use kuro_source::anime::{AnimeSource, AnimesPage, Hoster, SAnime, SEpisode, Track, Video};
use kuro_source::error::SourceError;
use kuro_source::filter_list::FilterList;
use kuro_source::model::{MangasPage, Page, SChapter, SManga};
use kuro_source::source::{CatalogueSource, MangaSource};
use serde::de::DeserializeOwned;
use serde_json::json;
use wasmtime::Store;
use wasmtime::component::{Component, HasSelf, Linker};

use crate::anime_wire::{WireAnime, WireAnimesPage, WireEpisode, WireHoster, WireTrack, WireVideo};
use crate::bindings::Source;
use crate::bridge::HostState;
use crate::error::WasmError;
use crate::limits::{self, CALL_DEADLINE_TICKS, MAX_WASM_BYTES};
use crate::wire::{SourceDescriptor, WireChapter, WireManga, WireMangasPage, WirePage};

/// What the host gives every guest: the client behind `http` and the preference store behind `pref-get`.
#[derive(Clone)]
pub struct HostEnv {
    pub client: NetworkClient,
    pub prefs: PrefStore,
}

/// Loads one package and returns its sources. `factory` picks the source list: `create-sources` returns a list, and
/// `info` returns the one source of a single-source package.
///
/// Compilation and the first calls run on a blocking thread. A failure, or a panic, in one package is an `Err` for that
/// package only, and the caller loads the others (T-18a-008).
pub async fn instantiate(
    bytes: Vec<u8>,
    hosts: Vec<String>,
    factory: bool,
    env: HostEnv,
) -> Result<Vec<Arc<dyn CatalogueSource>>, WasmError> {
    instantiate_with_deadline(bytes, hosts, factory, env, CALL_DEADLINE_TICKS).await
}

/// [`instantiate`] with the call deadline in epoch ticks, so a test can stop a spinning guest quickly.
#[doc(hidden)]
pub async fn instantiate_with_deadline(
    bytes: Vec<u8>,
    hosts: Vec<String>,
    factory: bool,
    env: HostEnv,
    deadline_ticks: u64,
) -> Result<Vec<Arc<dyn CatalogueSource>>, WasmError> {
    let sources = tokio::task::spawn_blocking(move || load(&bytes, hosts, factory, env, deadline_ticks))
        .await
        .map_err(|e| WasmError::Instantiate(format!("load task failed: {e}")))??;
    Ok(sources.into_iter().map(|source| Arc::new(source) as Arc<dyn CatalogueSource>).collect())
}

/// Loads an anime package: the same guest, read through the anime ops of `wit/kuro-source.wit`. The caller picks this for
/// an anime package and [`instantiate`] for a manga one, because the descriptor does not name the kind.
pub async fn instantiate_anime(
    bytes: Vec<u8>,
    hosts: Vec<String>,
    factory: bool,
    env: HostEnv,
) -> Result<Vec<Arc<dyn AnimeSource>>, WasmError> {
    let sources = tokio::task::spawn_blocking(move || load(&bytes, hosts, factory, env, CALL_DEADLINE_TICKS))
        .await
        .map_err(|e| WasmError::Instantiate(format!("load task failed: {e}")))??;
    Ok(sources
        .into_iter()
        .map(|source| Arc::new(WasmAnimeSource(source)) as Arc<dyn AnimeSource>)
        .collect())
}

/// Everything the sources of one package share: the compiled component and the host settings.
struct Package {
    component: Component,
    hosts: Vec<String>,
    env: HostEnv,
    deadline_ticks: u64,
}

fn load(
    bytes: &[u8],
    hosts: Vec<String>,
    factory: bool,
    env: HostEnv,
    deadline_ticks: u64,
) -> Result<Vec<WasmSource>, WasmError> {
    if bytes.len() > MAX_WASM_BYTES {
        return Err(WasmError::TooLarge {
            size: bytes.len(),
            limit: MAX_WASM_BYTES,
        });
    }
    let engine = limits::engine()?;
    let component = Component::new(engine, bytes).map_err(|e| WasmError::Compile(format!("{e:#}")))?;
    if component.get_export_index(None, "invoke").is_none() {
        return Err(WasmError::Instantiate("missing export invoke".to_owned()));
    }
    let package = Arc::new(Package {
        component,
        hosts,
        env,
        deadline_ticks,
    });

    let probe = Channel::new(Arc::clone(&package), 0);
    let op = if factory { "create-sources" } else { "info" };
    let text = probe.call(op, "{}").map_err(WasmError::Instantiate)?;
    let descriptors: Vec<SourceDescriptor> = if factory {
        serde_json::from_str(&text).map_err(|e| WasmError::Instantiate(format!("{op}: {e}")))?
    } else {
        vec![serde_json::from_str(&text).map_err(|e| WasmError::Instantiate(format!("{op}: {e}")))?]
    };

    reject_duplicate_ids(&descriptors)?;
    Ok(descriptors
        .into_iter()
        .map(|info| {
            let channel = Channel::new(Arc::clone(&package), info.id);
            WasmSource {
                info,
                channel: Arc::new(channel),
            }
        })
        .collect())
}

/// Two sources of one package may not share an id, so a lookup by id has one answer (T-18a-007).
fn reject_duplicate_ids(descriptors: &[SourceDescriptor]) -> Result<(), WasmError> {
    let mut seen = std::collections::HashSet::new();
    match descriptors.iter().find(|info| !seen.insert(info.id)) {
        Some(info) => Err(WasmError::Manifest(format!("duplicate source id {}", info.id))),
        None => Ok(()),
    }
}

/// One source's call channel. The mutex serialises calls, and holds the instance between them.
struct Channel {
    package: Arc<Package>,
    source_id: i64,
    guest: Mutex<Option<Guest>>,
}

impl Channel {
    fn new(package: Arc<Package>, source_id: i64) -> Self {
        Self {
            package,
            source_id,
            guest: Mutex::new(None),
        }
    }

    /// Runs one op. The `Err` text is the guest's error, or the trap text when the call trapped.
    fn call(&self, op: &str, input: &str) -> Result<String, String> {
        let mut slot = self.guest.lock().unwrap_or_else(PoisonError::into_inner);
        let mut guest = match slot.take() {
            Some(guest) => guest,
            None => Guest::new(&self.package, self.source_id)?,
        };
        match guest.invoke(op, input, self.package.deadline_ticks) {
            Ok(result) => {
                *slot = Some(guest);
                result
            }
            Err(trap) => Err(format!("{trap:#}")),
        }
    }
}

/// A live guest instance and its store.
struct Guest {
    store: Store<HostState>,
    bindings: Source,
}

impl Guest {
    fn new(package: &Package, source_id: i64) -> Result<Self, String> {
        let engine = limits::engine().map_err(|e| e.to_string())?;
        let mut linker = Linker::new(engine);
        Source::add_to_linker::<_, HasSelf<_>>(&mut linker, |state: &mut HostState| state)
            .map_err(|e| format!("{e:#}"))?;
        let state = HostState {
            source_id,
            hosts: package.hosts.clone(),
            client: package.env.client.clone(),
            prefs: package.env.prefs.clone(),
            limits: limits::store_limits(),
        };
        let mut store = Store::new(engine, state);
        store.limiter(|state| &mut state.limits);
        let bindings = Source::instantiate(&mut store, &package.component, &linker).map_err(|e| format!("{e:#}"))?;
        Ok(Self { store, bindings })
    }

    /// The outer `Err` is a trap. The inner `Err` is an error the guest returned.
    fn invoke(&mut self, op: &str, input: &str, deadline_ticks: u64) -> wasmtime::Result<Result<String, String>> {
        self.store.set_epoch_deadline(deadline_ticks);
        self.bindings.call_invoke(&mut self.store, op, input)
    }
}

/// A source of a loaded package. Identity comes from the descriptor, and every listing or detail call goes to the guest.
pub struct WasmSource {
    info: SourceDescriptor,
    channel: Arc<Channel>,
}

impl WasmSource {
    /// Sends `args` to `op`, inside the `{"sourceId", "args"}` envelope, on a blocking thread.
    async fn call(&self, op: &'static str, args: serde_json::Value) -> Result<String, SourceError> {
        let input = json!({ "sourceId": self.info.id, "args": args }).to_string();
        let channel = Arc::clone(&self.channel);
        let outcome = tokio::task::spawn_blocking(move || channel.call(op, &input))
            .await
            .map_err(|e| SourceError::Transport(format!("source call failed: {e}")))?;
        outcome.map_err(SourceError::Transport)
    }

    async fn typed<T: DeserializeOwned>(&self, op: &'static str, args: serde_json::Value) -> Result<T, SourceError> {
        let text = self.call(op, args).await?;
        serde_json::from_str(&text).map_err(|e| SourceError::Parse(e.to_string()))
    }

    async fn listing(&self, op: &'static str, args: serde_json::Value) -> Result<MangasPage, SourceError> {
        let page: WireMangasPage = self.typed(op, args).await?;
        Ok(MangasPage::from(page))
    }
}

/// A source of an anime package: a [`WasmSource`] read through the anime ops, with the same guest, mutex and deadline. It
/// is a wrapper, not a second trait impl on `WasmSource`, so the manga calls keep one meaning.
pub struct WasmAnimeSource(WasmSource);

impl WasmAnimeSource {
    /// The subtitle tracks of one video (the `subtitles` op). Not in `AnimeSource`: the app reads tracks from `Video`.
    pub async fn subtitles(&self, video: &Video) -> Result<Vec<Track>, SourceError> {
        let wire: Vec<WireTrack> = self.0.typed("subtitles", json!({ "video": WireVideo::from(video) })).await?;
        Ok(wire.into_iter().map(Track::from).collect())
    }
}

impl WasmSource {
    async fn anime_page(&self, op: &'static str, args: serde_json::Value) -> Result<AnimesPage, SourceError> {
        let page: WireAnimesPage = self.typed(op, args).await?;
        Ok(AnimesPage::from(page))
    }

    async fn videos(&self, args: serde_json::Value) -> Result<Vec<Video>, SourceError> {
        let wire: Vec<WireVideo> = self.typed("videos", args).await?;
        Ok(wire.into_iter().map(Video::from).collect())
    }
}

#[async_trait]
impl AnimeSource for WasmAnimeSource {
    fn id(&self) -> i64 {
        self.0.info.id
    }

    fn name(&self) -> &str {
        &self.0.info.name
    }

    fn lang(&self) -> &str {
        &self.0.info.lang
    }

    fn supports_latest(&self) -> bool {
        self.0.info.supports_latest
    }

    /// The package has no `details` or `seasons` op, so a combined update must not call them.
    fn supports_details(&self) -> bool {
        false
    }

    fn supports_season_list(&self) -> bool {
        false
    }

    /// Always true: an anime package answers the `hosters` op. A guest without it fails that call alone.
    fn provides_hosters(&self) -> bool {
        true
    }

    async fn popular(&self, page: i32) -> Result<AnimesPage, SourceError> {
        self.0.anime_page("popular", json!({ "page": page })).await
    }

    async fn latest(&self, page: i32) -> Result<AnimesPage, SourceError> {
        self.0.anime_page("latest", json!({ "page": page })).await
    }

    async fn search(&self, page: i32, query: &str, _filters: &FilterList) -> Result<AnimesPage, SourceError> {
        self.0
            .anime_page("search", json!({ "page": page, "query": query, "filters": [] }))
            .await
    }

    async fn episode_list(&self, anime: &SAnime) -> Result<Vec<SEpisode>, SourceError> {
        let wire: Vec<WireEpisode> = self.0.typed("episodes", json!({ "anime": WireAnime::from(anime) })).await?;
        Ok(wire.into_iter().map(SEpisode::from).collect())
    }

    async fn hosters(&self, episode: &SEpisode) -> Result<Vec<Hoster>, SourceError> {
        let wire: Vec<WireHoster> = self.0.typed("hosters", json!({ "episode": WireEpisode::from(episode) })).await?;
        Ok(wire.into_iter().map(Hoster::from).collect())
    }

    async fn videos_for_hoster(&self, hoster: &Hoster) -> Result<Vec<Video>, SourceError> {
        self.0.videos(json!({ "hoster": WireHoster::from(hoster) })).await
    }

    async fn videos_for_episode(&self, episode: &SEpisode) -> Result<Vec<Video>, SourceError> {
        self.0.videos(json!({ "episode": WireEpisode::from(episode) })).await
    }
}

#[async_trait]
impl MangaSource for WasmSource {
    fn id(&self) -> i64 {
        self.info.id
    }

    fn name(&self) -> &str {
        &self.info.name
    }

    fn lang(&self) -> &str {
        &self.info.lang
    }

    async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
        let wire: WireManga = self
            .typed("details", json!({ "manga": WireManga::from(manga) }))
            .await?;
        Ok(SManga::from(wire))
    }

    async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
        let chapters: Vec<WireChapter> = self
            .typed("chapters", json!({ "manga": WireManga::from(manga) }))
            .await?;
        Ok(chapters.into_iter().map(SChapter::from).collect())
    }

    async fn get_page_list(&self, chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
        let pages: Vec<WirePage> = self
            .typed("pages", json!({ "chapter": WireChapter::from(chapter) }))
            .await?;
        Ok(pages
            .into_iter()
            .enumerate()
            .map(|(index, page)| page.into_page(i32::try_from(index).unwrap_or(i32::MAX)))
            .collect())
    }
}

#[async_trait]
impl CatalogueSource for WasmSource {
    fn supports_latest(&self) -> bool {
        self.info.supports_latest
    }

    /// Empty: ABI 1.4 has no filter op, so a guest declares no filters. The `filters` of a search are sent as `[]`.
    fn filter_list(&self) -> FilterList {
        FilterList::default()
    }

    async fn popular(&self, page: i32) -> Result<MangasPage, SourceError> {
        self.listing("popular", json!({ "page": page })).await
    }

    async fn search(&self, page: i32, query: &str, _filters: &FilterList) -> Result<MangasPage, SourceError> {
        self.listing("search", json!({ "page": page, "query": query, "filters": [] }))
            .await
    }

    async fn latest(&self, page: i32) -> Result<MangasPage, SourceError> {
        self.listing("latest", json!({ "page": page })).await
    }
}

#[cfg(test)]
mod tests {
    use kuro_source::error::SourceError;
    use serde_json::json;

    use super::*;
    use crate::test_guest::{Behaviour, env, guest};

    /// One source of a single-guest package, built without the loader, so each test can pick the guest's answer.
    fn direct(bytes: Vec<u8>, id: i64, deadline_ticks: u64) -> WasmSource {
        let (host_env, _dir) = env();
        let engine = limits::engine().expect("engine");
        let component = Component::new(engine, &bytes).expect("component");
        let package = Arc::new(Package {
            component,
            hosts: Vec::new(),
            env: host_env,
            deadline_ticks,
        });
        let info = SourceDescriptor {
            id,
            name: "Fixture".into(),
            lang: "en".into(),
            supports_latest: true,
        };
        WasmSource {
            info,
            channel: Arc::new(Channel::new(package, id)),
        }
    }

    const INFO_AND_LISTING: &str = r#"{"id":42,"name":"Fixture","lang":"en","supportsLatest":true,"url":"/m/1","title":"One","mangas":[{"url":"/m/1","title":"One"}],"hasNextPage":true}"#;

    #[tokio::test(flavor = "multi_thread")]
    async fn a_single_source_package_reports_its_identity() {
        let (host_env, _dir) = env();
        let sources = instantiate(
            guest(Behaviour::Returns(INFO_AND_LISTING.into())),
            Vec::new(),
            false,
            host_env,
        )
        .await
        .expect("loads");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id(), 42);
        assert_eq!(sources[0].name(), "Fixture");
        assert_eq!(sources[0].lang(), "en");
        assert!(sources[0].supports_latest());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_factory_package_gives_one_source_per_entry_in_order() {
        let list = r#"[{"id":1,"name":"A","lang":"en"},{"id":2,"name":"B","lang":"ja"}]"#;
        let (host_env, _dir) = env();
        let sources = instantiate(guest(Behaviour::Returns(list.into())), Vec::new(), true, host_env)
            .await
            .expect("loads");
        let ids: Vec<i64> = sources.iter().map(|s| s.id()).collect();
        assert_eq!(ids, vec![1, 2]);
        assert_eq!(sources[1].lang(), "ja");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_popular_listing_comes_back_as_mangas() {
        let source = direct(
            guest(Behaviour::Returns(INFO_AND_LISTING.into())),
            42,
            CALL_DEADLINE_TICKS,
        );
        let page = source.popular(1).await.expect("listing");
        assert_eq!(page.mangas.len(), 1);
        assert_eq!(page.mangas[0].title, "One");
        assert!(page.has_next_page);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn every_call_sends_the_source_id_and_its_arguments_in_the_envelope() {
        let source = direct(guest(Behaviour::Echoes), 7, CALL_DEADLINE_TICKS);
        let sent = source.call("popular", json!({ "page": 2 })).await.expect("echo");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&sent).unwrap(),
            json!({ "args": { "page": 2 }, "sourceId": 7 })
        );
        let channel = Arc::clone(&source.channel);
        let text = tokio::task::spawn_blocking(move || channel.call("pages", "{}"))
            .await
            .unwrap();
        assert_eq!(text, Ok("{}".to_owned()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn details_chapters_and_pages_read_their_wire_shapes() {
        let details = direct(
            guest(Behaviour::Returns(INFO_AND_LISTING.into())),
            42,
            CALL_DEADLINE_TICKS,
        );
        let manga = details
            .get_manga_details(&SManga::new("/m/1", "One"))
            .await
            .expect("details");
        assert_eq!(manga.title, "One");

        let chapters_text = r#"[{"url":"/c/1","name":"Ch 1","chapterNumber":1.5}]"#;
        let chapters = direct(guest(Behaviour::Returns(chapters_text.into())), 1, CALL_DEADLINE_TICKS);
        let list = chapters
            .get_chapter_list(&SManga::new("/m/1", "One"))
            .await
            .expect("chapters");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].chapter_number, 1.5);

        let pages_text = r#"[{"url":"/p/1","imageUrl":"https://img.test/1.png"},{"url":"/p/2"}]"#;
        let pages = direct(guest(Behaviour::Returns(pages_text.into())), 1, CALL_DEADLINE_TICKS);
        let list = pages
            .get_page_list(&SChapter::new("/c/1", "Ch 1"))
            .await
            .expect("pages");
        assert_eq!(list.iter().map(|p| p.index).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(list[0].image_url.as_deref(), Some("https://img.test/1.png"));
        assert_eq!(list[1].image_url, None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_trapping_call_is_a_source_error_and_the_source_still_answers() {
        let source = direct(guest(Behaviour::Traps), 1, CALL_DEADLINE_TICKS);
        let first = source.popular(1).await.expect_err("traps");
        assert!(
            matches!(&first, SourceError::Transport(text) if text.contains("unreachable")),
            "{first:?}"
        );
        let second = source.popular(1).await.expect_err("traps again, without a panic");
        assert!(matches!(second, SourceError::Transport(_)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_guest_error_text_is_the_source_error_text() {
        let source = direct(
            guest(Behaviour::Fails("unknown op: popular".into())),
            1,
            CALL_DEADLINE_TICKS,
        );
        assert_eq!(
            source.popular(1).await,
            Err(SourceError::Transport("unknown op: popular".into()))
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_listing_that_is_not_json_is_a_parse_error() {
        let source = direct(guest(Behaviour::Returns("not json".into())), 1, CALL_DEADLINE_TICKS);
        assert!(matches!(source.popular(1).await, Err(SourceError::Parse(_))));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_factory_with_a_duplicate_source_id_is_a_manifest_error() {
        let list = r#"[{"id":1,"name":"A","lang":"en"},{"id":2,"name":"B","lang":"ja"},{"id":1,"name":"C","lang":"en"}]"#;
        // The duplicate is the third entry, so the first two are still a valid list.
        let (host_env, _dir) = env();
        let outcome = instantiate(guest(Behaviour::Returns(list.into())), Vec::new(), true, host_env)
            .await
            .map(|sources| sources.len());
        assert_eq!(outcome.err(), Some(WasmError::Manifest("duplicate source id 1".into())));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_factory_with_no_entries_loads_zero_sources() {
        let (host_env, _dir) = env();
        let sources = instantiate(guest(Behaviour::Returns("[]".into())), Vec::new(), true, host_env)
            .await
            .expect("loads");
        assert!(sources.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bytes_that_are_not_a_component_fail_to_compile() {
        let (host_env, _dir) = env();
        let outcome = instantiate(b"\0asm-but-not-really".to_vec(), Vec::new(), false, host_env)
            .await
            .map(|sources| sources.len());
        assert!(matches!(outcome, Err(WasmError::Compile(_))), "{outcome:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_oversized_package_is_refused_before_compiling() {
        let bytes = vec![0_u8; MAX_WASM_BYTES + 1];
        let (host_env, _dir) = env();
        let outcome = instantiate(bytes, Vec::new(), false, host_env)
            .await
            .map(|sources| sources.len());
        assert!(matches!(outcome, Err(WasmError::TooLarge { .. })), "{outcome:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_bad_package_does_not_stop_a_good_one_from_loading() {
        let (host_env, _dir) = env();
        let good = instantiate(
            guest(Behaviour::Returns(INFO_AND_LISTING.into())),
            Vec::new(),
            false,
            host_env,
        );
        let (host_env, _dir) = env();
        let bad = instantiate(guest(Behaviour::Traps), Vec::new(), false, host_env);
        let (good, bad) = tokio::join!(good, bad);
        assert_eq!(good.expect("good package loads").len(), 1);
        assert!(bad.is_err());
    }
}

#[cfg(test)]
mod anime_tests {
    use kuro_source::error::SourceError;
    use std::sync::Arc;

    use kuro_source::anime::{AnimeSource, SAnime, SEpisode, Video};
    use wasmtime::component::Component;

    use super::{Channel, Package, WasmAnimeSource, WasmSource};
    use crate::limits::{self, CALL_DEADLINE_TICKS};
    use crate::test_guest::{Behaviour, env, guest};
    use crate::wire::SourceDescriptor;

    /// One anime source over a guest whose every op answers `text`.
    fn anime(text: &str) -> WasmAnimeSource {
        anime_with(Behaviour::Returns(text.into()))
    }

    /// One anime source over a guest with the given behaviour.
    fn anime_with(behaviour: Behaviour) -> WasmAnimeSource {
        let (host_env, _dir) = env();
        let engine = limits::engine().expect("engine");
        let component = Component::new(engine, &guest(behaviour)).expect("component");
        let package = Arc::new(Package {
            component,
            hosts: Vec::new(),
            env: host_env,
            deadline_ticks: CALL_DEADLINE_TICKS,
        });
        WasmAnimeSource(WasmSource {
            info: SourceDescriptor {
                id: 42,
                name: "Fixture".into(),
                lang: "en".into(),
                supports_latest: true,
            },
            channel: Arc::new(Channel::new(package, 42)),
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_anime_source_reports_its_identity_and_a_listing() {
        let source = anime(r#"{"animes":[{"url":"/a/1","title":"One"}],"hasNextPage":true}"#);
        assert_eq!(source.id(), 42);
        assert!(source.supports_latest());
        assert!(source.provides_hosters());
        assert!(!source.supports_details());
        assert!(!source.supports_season_list());
        let page = source.popular(1).await.expect("popular");
        assert_eq!(page.animes[0].title, "One");
        assert!(page.has_next_page);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn episodes_read_the_episode_wire_shape() {
        let source = anime(r#"[{"url":"/e/1","name":"Ep 1","episodeNumber":1.0,"dateUpload":5}]"#);
        let list: Vec<SEpisode> = source.episode_list(&SAnime::default()).await.expect("episodes");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].episode_number, 1.0);
        assert_eq!(list[0].date_upload, 5);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hosters_carry_their_videos() {
        let source = anime(
            r#"[{"hosterUrl":"https://h/1","hosterName":"Main","videoList":[{"videoUrl":"https://v/1.m3u8","videoTitle":"1080p","resolution":1080}]}]"#,
        );
        let hosters = source.hosters(&SEpisode::default()).await.expect("hosters");
        assert_eq!(hosters[0].hoster_name, "Main");
        let videos = hosters[0].video_list.clone().expect("videos");
        assert_eq!(videos[0].resolution, Some(1080));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn videos_for_an_episode_read_the_video_shape() {
        let source = anime(r#"[{"videoUrl":"https://v/1.m3u8","videoTitle":"720p","preferred":true}]"#);
        let videos: Vec<Video> = source.videos_for_episode(&SEpisode::default()).await.expect("videos");
        assert_eq!(videos[0].video_url, "https://v/1.m3u8");
        assert!(videos[0].preferred);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subtitles_read_the_track_shape() {
        let source = anime(r#"[{"url":"https://s/en.vtt","lang":"en"}]"#);
        let tracks = source
            .subtitles(&Video::new("https://v/1.m3u8", "720p"))
            .await
            .expect("subtitles");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].lang, "en");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_guest_without_the_anime_ops_fails_only_that_call() {
        let source = anime_with(Behaviour::Fails("unknown op: episodes".into()));
        assert_eq!(
            source.episode_list(&SAnime::default()).await,
            Err(SourceError::Transport("unknown op: episodes".into()))
        );
        assert_eq!(source.id(), 42, "identity does not call the guest");
    }
}
