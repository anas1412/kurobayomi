//! OAuth login plumbing (`p1-topic-tracker-oauth.md` sections 4 to 7, `p2-topic-oauth-callback.md`): authorise URLs,
//! PKCE, the `state` value, pending attempts with a TTL, and redirect parsing. The random source is the only
//! side effect. The caller owns the token exchange and the secret store.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use url::Url;

use crate::messages::MsgKey;
use crate::model::TrackerId;

/// A login attempt stays valid this long after its authorise URL was opened (`p1-topic-tracker-oauth.md` section 4).
pub const PENDING_TTL_MS: i64 = 5 * 60 * 1000;

/// Verifier bytes. 50 bytes encode to the 67-character PKCE verifier of `PkceUtil.kt`.
const VERIFIER_BYTES: usize = 50;

/// `state` bytes. 32 bytes encode to 43 URL-safe characters.
const STATE_BYTES: usize = 32;

const ANILIST_CLIENT_ID: &str = "5338";
const MYANIMELIST_CLIENT_ID: &str = "686b980ff4240fccce7f6a654cea07ce";
const SHIKIMORI_CLIENT_ID: &str = "aOAYRqOLwxpA8skpcQIXetNy4cw2rn2fRzScawlcQ5U";
const BANGUMI_CLIENT_ID: &str = "bgm293165b66d7e58156";
const SIMKL_CLIENT_ID: &str = "aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e";

/// `n` random bytes as URL-safe base64 without padding.
fn random_token(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    rand::fill(&mut bytes[..]);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The PKCE verifier of one MyAnimeList attempt. The challenge is the verifier itself (plain method, as in source).
fn new_verifier() -> String {
    random_token(VERIFIER_BYTES)
}

/// The authorise URL of a browser login, with the source's query order and `state` last. `verifier` is the MyAnimeList
/// PKCE challenge. `None` for trackers without a browser login (Kitsu, MangaUpdates, enhanced trackers), or when
/// MyAnimeList has no verifier.
pub fn authorize_url(tracker: TrackerId, state: &str, verifier: Option<&str>) -> Option<String> {
    let (base, params): (&str, Vec<(&str, &str)>) = match tracker {
        TrackerId::ANILIST => (
            "https://anilist.co/api/v2/oauth/authorize",
            vec![("client_id", ANILIST_CLIENT_ID), ("response_type", "token")],
        ),
        TrackerId::MYANIMELIST => (
            "https://myanimelist.net/v1/oauth2/authorize",
            vec![
                ("client_id", MYANIMELIST_CLIENT_ID),
                ("code_challenge", verifier?),
                ("response_type", "code"),
            ],
        ),
        TrackerId::SHIKIMORI => (
            "https://shikimori.one/oauth/authorize",
            vec![
                ("client_id", SHIKIMORI_CLIENT_ID),
                ("redirect_uri", "aniyomi://shikimori-auth"),
                ("response_type", "code"),
            ],
        ),
        TrackerId::BANGUMI => (
            "https://bgm.tv/oauth/authorize",
            vec![
                ("client_id", BANGUMI_CLIENT_ID),
                ("response_type", "code"),
                ("redirect_uri", "aniyomi://bangumi-auth"),
            ],
        ),
        TrackerId::SIMKL => (
            "https://simkl.com/oauth/authorize",
            vec![
                ("response_type", "code"),
                ("client_id", SIMKL_CLIENT_ID),
                ("redirect_uri", "aniyomi://simkl-auth"),
            ],
        ),
        _ => return None,
    };
    let mut url = Url::parse(base).ok()?;
    url.query_pairs_mut()
        .extend_pairs(params)
        .append_pair("state", state);
    Some(url.into())
}

/// One login attempt, from the moment its authorise URL was opened. Lives in memory only, so a restart forgets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingLogin {
    pub tracker: TrackerId,
    pub state: String,
    /// MyAnimeList only. Never written anywhere but this struct.
    pub verifier: Option<String>,
    pub started_at_ms: i64,
}

/// What a redirect carried, with the pending attempt applied. `Accepted` consumes the attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Not an `aniyomi://` URI. Ignored.
    Ignored,
    /// Unknown host or unreadable link. The caller shows "Unrecognized login link".
    Unrecognized,
    /// The user refused, or the redirect had no credential. The stored login stays.
    Cancelled(TrackerId),
    /// No attempt is pending for this tracker. No network call is made.
    NotAllowed(TrackerId),
    /// The attempt is older than [`PENDING_TTL_MS`]. It is dropped.
    Expired(TrackerId),
    /// The echoed `state` differs from the attempt. The attempt stays pending and the stored login stays.
    StateMismatch(TrackerId),
    /// The attempt is consumed. The caller exchanges `value` for a token (authorisation code) or stores it (implicit).
    Accepted(Credential),
}

/// The credential of an accepted redirect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub tracker: TrackerId,
    /// The authorisation code, or the AniList access token.
    pub value: String,
    /// MyAnimeList's PKCE verifier from the attempt.
    pub verifier: Option<String>,
    /// AniList's `expires_in` in seconds, when the redirect carried it.
    pub expires_in: Option<i64>,
}

impl Route {
    /// The toast of a route that stops the login. `name` is the tracker's display name.
    pub fn message(&self, name: &str) -> Option<MsgKey> {
        Some(match self {
            Route::Unrecognized => MsgKey::UnrecognizedLoginLink,
            Route::Cancelled(_) => MsgKey::TrackerLoginCancelled(name.to_owned()),
            Route::NotAllowed(_) | Route::Expired(_) => MsgKey::LoginExpired,
            Route::StateMismatch(_) => MsgKey::TrackerLoginFailed(name.to_owned()),
            Route::Ignored | Route::Accepted(_) => return None,
        })
    }
}

/// Pending attempts, one per tracker. A new attempt replaces the old one (last attempt wins).
#[derive(Debug, Default)]
pub struct PendingLogins(HashMap<TrackerId, PendingLogin>);

impl PendingLogins {
    /// Opens an attempt for `tracker` and returns its authorise URL. `None` when the tracker has no browser login.
    pub fn begin(&mut self, tracker: TrackerId, now_ms: i64) -> Option<String> {
        let state = random_token(STATE_BYTES);
        let verifier = (tracker == TrackerId::MYANIMELIST).then(new_verifier);
        let url = authorize_url(tracker, &state, verifier.as_deref())?;
        self.0.insert(
            tracker,
            PendingLogin {
                tracker,
                state,
                verifier,
                started_at_ms: now_ms,
            },
        );
        Some(url)
    }

    /// The live attempt of `tracker`. An attempt past its TTL is dropped and reported as `Expired`.
    fn live(&mut self, tracker: TrackerId, now_ms: i64) -> Result<&PendingLogin, Route> {
        let expired = match self.0.get(&tracker) {
            None => return Err(Route::NotAllowed(tracker)),
            Some(attempt) => now_ms - attempt.started_at_ms > PENDING_TTL_MS,
        };
        if expired {
            self.0.remove(&tracker);
            return Err(Route::Expired(tracker));
        }
        self.0.get(&tracker).ok_or(Route::NotAllowed(tracker))
    }

    /// Applies a redirect URI to the pending attempts. Nothing here touches the network or the store.
    pub fn route(&mut self, uri: &str, now_ms: i64) -> Route {
        match parse_callback(uri) {
            Redirect::Ignored => Route::Ignored,
            Redirect::Unexpected => Route::Unrecognized,
            Redirect::Cancelled(tracker) => match self.live(tracker, now_ms) {
                Ok(_) => {
                    self.0.remove(&tracker);
                    Route::Cancelled(tracker)
                }
                Err(stop) => stop,
            },
            Redirect::Credential {
                tracker,
                value,
                state,
                expires_in,
            } => {
                let attempt = match self.live(tracker, now_ms) {
                    Ok(attempt) => attempt,
                    Err(stop) => return stop,
                };
                if let Some(echoed) = state {
                    if echoed != attempt.state {
                        return Route::StateMismatch(tracker);
                    }
                }
                let verifier = attempt.verifier.clone();
                self.0.remove(&tracker);
                Route::Accepted(Credential {
                    tracker,
                    value,
                    verifier,
                    expires_in,
                })
            }
        }
    }
}

/// What the open handler does with a redirect (T-27a-022).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handled {
    /// Not ours. Nothing happens.
    Ignored,
    /// Show this toast. The login stops here.
    Toast(MsgKey),
    /// Exchange the credential for a token (or store it, for an implicit grant), then save it with
    /// [`crate::base::BaseTracker::save_oauth`].
    Exchange(Credential),
}

/// Routes an `aniyomi://<tracker>-auth` URI received by the application's open handler: the GUI passes every opened
/// URI here, shows the toast of [`Handled::Toast`], and starts the token exchange for [`Handled::Exchange`].
pub fn handle_redirect(
    pending: &mut PendingLogins,
    trackers: &crate::manager::TrackerManager,
    uri: &str,
    now_ms: i64,
) -> Handled {
    let route = pending.route(uri, now_ms);
    let name = match &route {
        Route::Cancelled(t) | Route::NotAllowed(t) | Route::Expired(t) | Route::StateMismatch(t) => {
            trackers
                .get(*t)
                .map_or_else(|| t.0.to_string(), |tr| tr.name().to_owned())
        }
        _ => String::new(),
    };
    if let Some(message) = route.message(&name) {
        return Handled::Toast(message);
    }
    match route {
        Route::Accepted(credential) => Handled::Exchange(credential),
        _ => Handled::Ignored,
    }
}

/// What a redirect URI carried, before any pending check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Redirect {
    /// Not an `aniyomi://` URI. Ignored.
    Ignored,
    /// `aniyomi://<unknown>-auth`: "Unrecognized login link".
    Unexpected,
    /// The user refused, or the redirect carried no code or token: "Login cancelled". Old credentials stay.
    Cancelled(TrackerId),
    /// An authorisation code (query) or an implicit token (fragment), with the echoed `state` if any.
    Credential {
        tracker: TrackerId,
        value: String,
        state: Option<String>,
        expires_in: Option<i64>,
    },
}

fn tracker_of(host: &str) -> Option<TrackerId> {
    Some(match host.strip_suffix("-auth")? {
        "anilist" => TrackerId::ANILIST,
        "myanimelist" => TrackerId::MYANIMELIST,
        "shikimori" => TrackerId::SHIKIMORI,
        "bangumi" => TrackerId::BANGUMI,
        "simkl" => TrackerId::SIMKL,
        _ => return None,
    })
}

/// The raw value of `key` in a `&`-separated fragment or query. The key must start a segment (the fragment start or
/// after `&`), and the value runs to the next `&`. The value is not percent-decoded (E-50, E-51, E-55).
fn raw_value<'a>(source: &'a str, key: &str) -> Option<&'a str> {
    source
        .split('&')
        .find_map(|segment| segment.strip_prefix(key)?.strip_prefix('='))
}

/// Parses a redirect URI. AniList answers in the fragment (implicit grant, `access_token`), the others in the query
/// (`code`).
pub fn parse_callback(uri: &str) -> Redirect {
    let Ok(url) = Url::parse(uri) else {
        return Redirect::Ignored;
    };
    if url.scheme() != "aniyomi" {
        return Redirect::Ignored;
    }
    let Some(tracker) = url.host_str().and_then(tracker_of) else {
        return Redirect::Unexpected;
    };
    if tracker == TrackerId::ANILIST {
        let fragment = url.fragment().unwrap_or("");
        return match raw_value(fragment, "access_token").filter(|token| !token.is_empty()) {
            Some(token) => Redirect::Credential {
                tracker,
                value: token.to_owned(),
                state: raw_value(fragment, "state").map(str::to_owned),
                expires_in: raw_value(fragment, "expires_in").and_then(|v| v.parse().ok()),
            },
            None => Redirect::Cancelled(tracker),
        };
    }
    let mut value = None;
    let mut state = None;
    for (k, v) in url::form_urlencoded::parse(url.query().unwrap_or("").as_bytes()) {
        if k == "code" && !v.is_empty() {
            value = Some(v.into_owned());
        } else if k == "state" {
            state = Some(v.into_owned());
        }
    }
    match value {
        Some(value) => Redirect::Credential {
            tracker,
            value,
            state,
            expires_in: None,
        },
        None => Redirect::Cancelled(tracker),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tr_27a_022_the_open_handler_gives_a_toast_or_a_credential() {
        let h = crate::fakes::Harness::new();
        let trackers = &h.ctx.trackers;
        let mut pending = PendingLogins::default();
        assert_eq!(
            handle_redirect(&mut pending, trackers, "https://example.test/", 0),
            Handled::Ignored
        );
        assert_eq!(
            handle_redirect(&mut pending, trackers, "aniyomi://unknown-auth?code=x", 0),
            Handled::Toast(MsgKey::UnrecognizedLoginLink)
        );
        assert_eq!(
            handle_redirect(&mut pending, trackers, "aniyomi://simkl-auth?code=x", 0),
            Handled::Toast(MsgKey::LoginExpired),
            "no attempt is pending"
        );
        let url = pending.begin(TrackerId::SIMKL, 0).unwrap();
        let state = Url::parse(&url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned();
        let uri = format!("aniyomi://simkl-auth?code=abc&state={state}");
        let Handled::Exchange(c) = handle_redirect(&mut pending, trackers, &uri, 1) else {
            panic!("an accepted redirect is exchanged");
        };
        assert_eq!((c.tracker, c.value.as_str()), (TrackerId::SIMKL, "abc"));
    }

    fn credential(tracker: TrackerId, value: &str, state: Option<&str>) -> Redirect {
        Redirect::Credential {
            tracker,
            value: value.into(),
            state: state.map(str::to_owned),
            expires_in: None,
        }
    }

    #[test]
    fn tr_27_d17_an_unknown_host_is_an_unexpected_response() {
        assert_eq!(parse_callback("aniyomi://foo?code=x"), Redirect::Unexpected);
        assert_eq!(
            parse_callback("aniyomi://unknown-auth?code=x"),
            Redirect::Unexpected
        );
    }

    #[test]
    fn tr_27_d18_a_fragment_token_without_a_trailing_ampersand_parses() {
        assert_eq!(
            parse_callback("aniyomi://anilist-auth#access_token=abc"),
            credential(TrackerId::ANILIST, "abc", None)
        );
    }

    #[test]
    fn tr_27_d18_a_fragment_token_with_extra_fields_parses() {
        assert_eq!(
            parse_callback(
                "aniyomi://anilist-auth#access_token=abc&token_type=Bearer&expires_in=3600"
            ),
            Redirect::Credential {
                tracker: TrackerId::ANILIST,
                value: "abc".into(),
                state: None,
                expires_in: Some(3600),
            }
        );
    }

    #[test]
    fn tr_27b_021_the_fragment_token_is_the_raw_substring_not_decoded() {
        assert_eq!(
            parse_callback("aniyomi://anilist-auth#access_token=a+b%20c&token_type=Bearer"),
            credential(TrackerId::ANILIST, "a+b%20c", None)
        );
    }

    #[test]
    fn tr_27b_021_access_token_is_matched_only_at_a_segment_start() {
        assert_eq!(
            parse_callback("aniyomi://anilist-auth#old_access_token=no&access_token=yes"),
            credential(TrackerId::ANILIST, "yes", None)
        );
        assert_eq!(
            parse_callback("aniyomi://anilist-auth#xaccess_token=no"),
            Redirect::Cancelled(TrackerId::ANILIST)
        );
    }

    #[test]
    fn tr_27b_021_a_fragment_without_a_token_is_denied_not_a_token() {
        assert_eq!(
            parse_callback("aniyomi://anilist-auth#access_token=&token_type=Bearer"),
            Redirect::Cancelled(TrackerId::ANILIST)
        );
    }

    #[test]
    fn redirect_table_cancelled_rows() {
        assert_eq!(
            parse_callback("aniyomi://anilist-auth#error=access_denied"),
            Redirect::Cancelled(TrackerId::ANILIST)
        );
        assert_eq!(
            parse_callback("aniyomi://bangumi-auth?error=access_denied"),
            Redirect::Cancelled(TrackerId::BANGUMI)
        );
        assert_eq!(
            parse_callback("aniyomi://myanimelist-auth"),
            Redirect::Cancelled(TrackerId::MYANIMELIST)
        );
    }

    #[test]
    fn redirect_table_code_rows_carry_the_state() {
        assert_eq!(
            parse_callback("aniyomi://bangumi-auth?code=xyz&state=s1"),
            credential(TrackerId::BANGUMI, "xyz", Some("s1"))
        );
        assert_eq!(
            parse_callback("aniyomi://simkl-auth?code=a%20b"),
            credential(TrackerId::SIMKL, "a b", None)
        );
    }

    #[test]
    fn redirect_table_other_schemes_are_ignored() {
        assert_eq!(
            parse_callback("http://anilist-auth?code=x"),
            Redirect::Ignored
        );
        assert_eq!(parse_callback("not a uri"), Redirect::Ignored);
    }

    #[test]
    fn tr_27b_025_the_pkce_verifier_is_67_url_safe_characters() {
        let v = new_verifier();
        assert_eq!(v.len(), 67);
        assert!(
            v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
        assert_ne!(v, new_verifier(), "each attempt draws its own verifier");
    }

    #[test]
    fn tr_27a_021_state_is_43_url_safe_characters_per_request() {
        let mut pending = PendingLogins::default();
        let first = pending.begin(TrackerId::ANILIST, 0).unwrap();
        let state = pending.0[&TrackerId::ANILIST].state.clone();
        assert_eq!(state.len(), 43);
        assert!(first.contains(&format!("state={state}")));
        pending.begin(TrackerId::ANILIST, 0).unwrap();
        assert_ne!(pending.0[&TrackerId::ANILIST].state, state);
    }

    #[test]
    fn tr_27a_021_authorize_urls_keep_the_source_order_and_add_state_last() {
        assert_eq!(
            authorize_url(TrackerId::ANILIST, "S", None).unwrap(),
            "https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token&state=S"
        );
        assert_eq!(
            authorize_url(TrackerId::BANGUMI, "S", None).unwrap(),
            "https://bgm.tv/oauth/authorize?client_id=bgm293165b66d7e58156&response_type=code&redirect_uri=aniyomi%3A%2F%2Fbangumi-auth&state=S"
        );
        assert_eq!(
            authorize_url(TrackerId::SIMKL, "S", None).unwrap(),
            "https://simkl.com/oauth/authorize?response_type=code&client_id=aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth&state=S"
        );
    }

    #[test]
    fn tr_27a_021_mal_url_carries_the_challenge_and_no_method() {
        let url = authorize_url(TrackerId::MYANIMELIST, "S", Some("VERIFIER")).unwrap();
        assert!(url.contains("code_challenge=VERIFIER&response_type=code"));
        assert!(!url.contains("code_challenge_method"));
        assert!(authorize_url(TrackerId::MYANIMELIST, "S", None).is_none());
    }

    #[test]
    fn tr_27a_021_trackers_without_a_browser_login_have_no_url() {
        assert!(authorize_url(TrackerId::KITSU, "S", None).is_none());
        assert!(authorize_url(TrackerId::MANGAUPDATES, "S", None).is_none());
        assert!(
            PendingLogins::default()
                .begin(TrackerId::KITSU, 0)
                .is_none()
        );
    }

    #[test]
    fn tr_27b_026_a_code_without_a_pending_attempt_is_not_allowed() {
        let mut pending = PendingLogins::default();
        assert_eq!(
            pending.route("aniyomi://bangumi-auth?code=x&state=s", 0),
            Route::NotAllowed(TrackerId::BANGUMI)
        );
    }

    #[test]
    fn tr_27b_026_an_attempt_expires_after_five_minutes() {
        let mut pending = PendingLogins::default();
        pending.begin(TrackerId::SIMKL, 1_000).unwrap();
        let state = pending.0[&TrackerId::SIMKL].state.clone();
        let uri = format!("aniyomi://simkl-auth?code=x&state={state}");
        assert_eq!(
            pending.route(&uri, 1_000 + PENDING_TTL_MS + 1),
            Route::Expired(TrackerId::SIMKL)
        );
        assert_eq!(
            pending.route(&uri, 1_000 + PENDING_TTL_MS + 2),
            Route::NotAllowed(TrackerId::SIMKL),
            "an expired attempt is dropped"
        );
    }

    #[test]
    fn tr_27b_026_the_attempt_is_still_live_at_exactly_the_ttl() {
        let mut pending = PendingLogins::default();
        pending.begin(TrackerId::SIMKL, 0).unwrap();
        let state = pending.0[&TrackerId::SIMKL].state.clone();
        let uri = format!("aniyomi://simkl-auth?code=x&state={state}");
        assert!(matches!(
            pending.route(&uri, PENDING_TTL_MS),
            Route::Accepted(_)
        ));
    }

    #[test]
    fn tr_27b_026_last_attempt_wins_and_a_mismatched_state_keeps_it_pending() {
        let mut pending = PendingLogins::default();
        pending.begin(TrackerId::BANGUMI, 0).unwrap();
        let old = pending.0[&TrackerId::BANGUMI].state.clone();
        pending.begin(TrackerId::BANGUMI, 0).unwrap();
        let new = pending.0[&TrackerId::BANGUMI].state.clone();
        assert_eq!(
            pending.route(&format!("aniyomi://bangumi-auth?code=x&state={old}"), 0),
            Route::StateMismatch(TrackerId::BANGUMI)
        );
        assert!(matches!(
            pending.route(&format!("aniyomi://bangumi-auth?code=x&state={new}"), 0),
            Route::Accepted(_)
        ));
    }

    #[test]
    fn tr_27b_025_mal_verifier_travels_with_the_accepted_code() {
        let mut pending = PendingLogins::default();
        pending.begin(TrackerId::MYANIMELIST, 0).unwrap();
        let attempt = pending.0[&TrackerId::MYANIMELIST].clone();
        let uri = format!(
            "aniyomi://myanimelist-auth?code=abc&state={}",
            attempt.state
        );
        assert_eq!(
            pending.route(&uri, 0),
            Route::Accepted(Credential {
                tracker: TrackerId::MYANIMELIST,
                value: "abc".into(),
                verifier: attempt.verifier,
                expires_in: None,
            })
        );
        assert!(pending.0.is_empty(), "the attempt is consumed");
    }

    #[test]
    fn tr_27b_021_anilist_accepts_without_state_and_carries_expires_in() {
        let mut pending = PendingLogins::default();
        pending.begin(TrackerId::ANILIST, 0).unwrap();
        assert_eq!(
            pending.route("aniyomi://anilist-auth#access_token=t&expires_in=60", 0),
            Route::Accepted(Credential {
                tracker: TrackerId::ANILIST,
                value: "t".into(),
                verifier: None,
                expires_in: Some(60),
            })
        );
    }

    #[test]
    fn tr_27b_024_a_cancelled_redirect_consumes_the_attempt_and_names_the_tracker() {
        let mut pending = PendingLogins::default();
        pending.begin(TrackerId::SHIKIMORI, 0).unwrap();
        let route = pending.route("aniyomi://shikimori-auth?error=access_denied", 0);
        assert_eq!(route, Route::Cancelled(TrackerId::SHIKIMORI));
        assert_eq!(
            route.message("Shikimori"),
            Some(MsgKey::TrackerLoginCancelled("Shikimori".into()))
        );
        assert!(pending.0.is_empty());
    }

    #[test]
    fn tr_27b_023_unknown_hosts_and_stopped_logins_have_their_toasts() {
        let mut pending = PendingLogins::default();
        let route = pending.route("aniyomi://foo-auth?code=x", 0);
        assert_eq!(
            route.message(""),
            Some(MsgKey::UnrecognizedLoginLink),
            "no hanging state"
        );
        assert_eq!(
            Route::Expired(TrackerId::ANILIST).message("AniList"),
            Some(MsgKey::LoginExpired)
        );
        assert_eq!(Route::Ignored.message("x"), None);
    }
}
