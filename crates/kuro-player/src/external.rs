//! External players on Linux (area 45 `ExternalIntents.kt`, T-45a-055 to T-45a-058, T-45a-062).
//! Android intents do not exist here. A preset names a program and its argument list. The argument
//! list is built by pure functions and spawned detached, with stdio closed and a new process group.
//! Desktop players do not return a result. Only mpv gets progress, through its IPC socket (`ipc.rs`).

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::load::http_header_fields;
use crate::types::Track;

/// The external player the preference names (`external_player_preference`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Empty or unknown: the desktop's default handler for the URI.
    Default,
    Mpv,
    Vlc,
    Celluloid,
    /// A user command template. Whitespace splits it into arguments. Placeholders: `{url}`,
    /// `{title}`, `{position}` (seconds), `{subtitle}`, `{headers}`.
    Custom(String),
}

impl Preset {
    pub fn from_preference(value: &str, custom_template: &str) -> Self {
        match value {
            "mpv" => Self::Mpv,
            "vlc" => Self::Vlc,
            "celluloid" => Self::Celluloid,
            "custom" => Self::Custom(custom_template.to_owned()),
            _ => Self::Default,
        }
    }
}

/// External player settings (`pref_always_use_external_player` and `external_player_preference`,
/// T-45b-049). The launch itself is `plan` and `run`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExternalPrefs {
    /// Start every episode in the external player.
    pub always_use: bool,
    /// The preset name: `mpv`, `vlc`, `celluloid`, `custom`, or empty for the desktop default.
    pub preference: String,
}

impl ExternalPrefs {
    /// The preset the preference names. `custom_template` is used only for `custom`.
    pub fn preset(&self, custom_template: &str) -> Preset {
        Preset::from_preference(&self.preference, custom_template)
    }
}

/// What one episode launch needs. `position_ms` is the start position (T-45a-062).
#[derive(Clone, Debug, PartialEq)]
pub struct Launch<'a> {
    pub url: &'a str,
    /// `"<anime title> - <episode name>"` (`title`, T-45a-057).
    pub title: &'a str,
    pub start_ms: i64,
    /// Request headers. Empty means none.
    pub headers: &'a [(String, String)],
    /// The subtitle picked for the locale (`select_subtitle`).
    pub subtitle: Option<&'a str>,
    /// mpv IPC socket for progress. Only mpv uses it.
    pub ipc_socket: Option<&'a Path>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Hand the URI to the desktop's default handler.
    Open(String),
    /// Run a program with these arguments. `program` is the resolved executable.
    Run { program: PathBuf, args: Vec<String> },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ExternalError {
    #[error("Player not found: {0}")]
    NotFound(String),
    #[error("the custom player command is empty")]
    EmptyTemplate,
    #[error("could not start the player: {0}")]
    Spawn(String),
}

/// Seconds with three decimals, as the argument lists print a position (`--start=`).
pub fn seconds(ms: i64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

/// The title passed to the player: `"<anime title> - <episode name>"` (T-45a-057).
pub fn title(anime: &str, episode: &str) -> String {
    format!("{anime} - {episode}")
}

/// The content type guessed from the URL's extension (T-45a-059), used only when the desktop opener
/// needs one. A query string is ignored. Anything unknown is `video/any`.
pub fn mime_type(url: &str) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let file = path.rsplit('/').next().unwrap_or(path);
    let ext = file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("mp4") => "video/mp4",
        Some("mkv") => "video/x-matroska",
        Some("m3u8") => "application/x-mpegURL",
        _ => "video/any",
    }
}

/// Start position in ms (T-45a-062). A seen episode restarts at 1 ms, unless the watching position
/// is kept and the saved position is short of the end. Otherwise the saved position is used.
pub fn start_ms(seen: bool, preserve_position: bool, last_ms: i64, total_ms: i64) -> i64 {
    if seen && (!preserve_position || last_ms == total_ms) {
        1
    } else {
        last_ms
    }
}

/// The subtitle track for the system locale (T-45a-058). The first track whose language contains
/// the locale's simple display name wins, ignoring case. Otherwise the first track. No tracks gives
/// nothing.
pub fn select_subtitle<'a>(tracks: &'a [Track], locale_name: &str) -> Option<&'a Track> {
    let wanted = locale_name.to_lowercase();
    tracks
        .iter()
        .find(|track| track.lang.to_lowercase().contains(&wanted))
        .or_else(|| tracks.first())
}

fn header<'h>(headers: &'h [(String, String)], name: &str) -> Option<&'h str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// `Key: value,...` for mpv and celluloid. Empty when there are no headers.
fn joined_headers(headers: &[(String, String)]) -> Option<String> {
    http_header_fields(true, Some(headers), &[]).filter(|text| !text.is_empty())
}

/// Escapes `,` in a value of an mpv option list (`--mpv-options=`).
fn escape_option(value: &str) -> String {
    value.replace(',', "\\,")
}

fn mpv_args(launch: &Launch) -> Vec<String> {
    let mut args = vec![
        format!("--title={}", launch.title),
        format!("--start={}", seconds(launch.start_ms)),
    ];
    if let Some(headers) = joined_headers(launch.headers) {
        args.push(format!("--http-header-fields={headers}"));
    }
    if let Some(subtitle) = launch.subtitle {
        args.push(format!("--sub-file={subtitle}"));
    }
    if let Some(socket) = launch.ipc_socket {
        args.push(format!("--input-ipc-server={}", socket.display()));
    }
    args.push(launch.url.to_owned());
    args
}

fn vlc_args(launch: &Launch) -> Vec<String> {
    let mut args = vec![
        format!("--meta-title={}", launch.title),
        format!("--start-time={}", seconds(launch.start_ms)),
    ];
    // VLC has no combined header option. It takes the user agent and referrer separately.
    if let Some(agent) = header(launch.headers, "User-Agent") {
        args.push(format!("--http-user-agent={agent}"));
    }
    if let Some(referer) = header(launch.headers, "Referer") {
        args.push(format!("--http-referrer={referer}"));
    }
    if let Some(subtitle) = launch.subtitle {
        args.push(format!("--sub-file={subtitle}"));
    }
    args.push(launch.url.to_owned());
    args
}

fn celluloid_args(launch: &Launch) -> Vec<String> {
    let mut options = vec![format!("start={}", seconds(launch.start_ms))];
    if let Some(headers) = joined_headers(launch.headers) {
        options.push(format!("http-header-fields={}", escape_option(&headers)));
    }
    if let Some(subtitle) = launch.subtitle {
        options.push(format!("sub-file={}", escape_option(subtitle)));
    }
    vec![
        format!("--mpv-options={}", options.join(",")),
        launch.url.to_owned(),
    ]
}

/// Splits the template on whitespace, then fills the placeholders in each piece. A piece that is
/// empty after filling is dropped. The first piece is the program and is not filled.
fn custom_args(template: &str, launch: &Launch) -> Result<(String, Vec<String>), ExternalError> {
    let mut pieces = template.split_whitespace();
    let program = pieces
        .next()
        .ok_or(ExternalError::EmptyTemplate)?
        .to_owned();
    let headers = joined_headers(launch.headers).unwrap_or_default();
    let position = seconds(launch.start_ms);
    let subtitle = launch.subtitle.unwrap_or_default();
    let args = pieces
        .map(|piece| {
            piece
                .replace("{url}", launch.url)
                .replace("{title}", launch.title)
                .replace("{position}", &position)
                .replace("{subtitle}", subtitle)
                .replace("{headers}", &headers)
        })
        .filter(|piece| !piece.is_empty())
        .collect();
    Ok((program, args))
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Finds `program` the way a shell would: a name with a `/` is used as it is, anything else is looked
/// up in the directories of `path_var`.
pub fn find_on_path(program: &str, path_var: &OsStr) -> Option<PathBuf> {
    if program.contains('/') {
        let path = PathBuf::from(program);
        return is_executable(&path).then_some(path);
    }
    std::env::split_paths(path_var)
        .map(|dir| dir.join(program))
        .find(|path| is_executable(path))
}

fn resolve(program: &str, path_var: &OsStr) -> Result<PathBuf, ExternalError> {
    find_on_path(program, path_var).ok_or_else(|| ExternalError::NotFound(program.to_owned()))
}

/// Builds what to run for a preset. Nothing is started. A missing program is `NotFound`.
pub fn plan(preset: &Preset, launch: &Launch, path_var: &OsStr) -> Result<Plan, ExternalError> {
    let (program, args) = match preset {
        Preset::Default => return Ok(Plan::Open(launch.url.to_owned())),
        Preset::Mpv => ("mpv", mpv_args(launch)),
        Preset::Vlc => ("vlc", vlc_args(launch)),
        Preset::Celluloid => ("celluloid", celluloid_args(launch)),
        Preset::Custom(template) => {
            let (program, args) = custom_args(template, launch)?;
            return Ok(Plan::Run {
                program: resolve(&program, path_var)?,
                args,
            });
        }
    };
    Ok(Plan::Run {
        program: resolve(program, path_var)?,
        args,
    })
}

/// Runs a plan. A program is spawned detached (stdio closed, own process group) and its child is
/// returned, so mpv can be watched. The desktop default is opened and nothing is returned.
pub fn run(plan: &Plan) -> Result<Option<Child>, ExternalError> {
    match plan {
        Plan::Open(uri) => {
            kuro_platform::opener::Opener::open_target(&kuro_platform::opener::XdgOpener, uri)
                .map_err(|e| ExternalError::Spawn(e.to_string()))?;
            Ok(None)
        }
        Plan::Run { program, args } => Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map(Some)
            .map_err(|e| ExternalError::Spawn(e.to_string())),
    }
}

/// What to save when an external mpv session ends, from its last position and duration in seconds
/// (T-45a-061). Within one second of the end is a completion, and the total is saved as the
/// position. `last_second_seen_ms` and `total_ms` are in ms, as `progress.rs` expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalSave {
    pub last_second_seen_ms: i64,
    pub total_ms: i64,
    pub cause: Option<&'static str>,
}

pub fn final_save(position_secs: f64, duration_secs: f64) -> FinalSave {
    let total_ms = (duration_secs * 1000.0).round() as i64;
    if duration_secs > 0.0 && (duration_secs - position_secs).abs() <= 1.0 {
        FinalSave {
            last_second_seen_ms: total_ms,
            total_ms,
            cause: Some("playback_completion"),
        }
    } else {
        FinalSave {
            last_second_seen_ms: (position_secs * 1000.0).round() as i64,
            total_ms,
            cause: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Track;

    fn headers() -> Vec<(String, String)> {
        vec![
            ("Referer".to_owned(), "https://site".to_owned()),
            ("User-Agent".to_owned(), "UA/1".to_owned()),
        ]
    }

    fn launch<'a>(headers: &'a [(String, String)], subtitle: Option<&'a str>) -> Launch<'a> {
        Launch {
            url: "https://cdn/v.m3u8",
            title: "Show - Pilot",
            start_ms: 12_500,
            headers,
            subtitle,
            ipc_socket: None,
        }
    }

    /// A PATH with one executable named `program` in a temp directory.
    fn path_with(program: &str) -> (tempfile::TempDir, std::ffi::OsString) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(program);
        std::fs::write(&file, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = dir.path().as_os_str().to_owned();
        (dir, path)
    }

    fn argv(plan: Plan) -> Vec<String> {
        let Plan::Run { args, .. } = plan else {
            panic!("expected a program to run");
        };
        args
    }

    #[test]
    fn external_prefs_default_to_the_desktop_handler() {
        let prefs = ExternalPrefs::default();
        assert!(!prefs.always_use);
        assert_eq!(prefs.preset(""), Preset::Default);
        let mpv = ExternalPrefs {
            always_use: true,
            preference: "mpv".to_owned(),
        };
        assert_eq!(mpv.preset(""), Preset::Mpv);
    }

    #[test]
    fn preset_names_map_and_unknown_is_the_default() {
        assert_eq!(Preset::from_preference("", ""), Preset::Default);
        assert_eq!(Preset::from_preference("mpv", ""), Preset::Mpv);
        assert_eq!(Preset::from_preference("vlc", ""), Preset::Vlc);
        assert_eq!(Preset::from_preference("celluloid", ""), Preset::Celluloid);
        assert_eq!(
            Preset::from_preference("custom", "x {url}"),
            Preset::Custom("x {url}".to_owned())
        );
        assert_eq!(Preset::from_preference("is.xyz.mpv", ""), Preset::Default);
    }

    #[test]
    fn default_preset_opens_the_uri_without_a_program() {
        let plan = plan(&Preset::Default, &launch(&[], None), OsStr::new("")).unwrap();
        assert_eq!(plan, Plan::Open("https://cdn/v.m3u8".to_owned()));
    }

    #[test]
    fn mpv_argv_carries_title_start_headers_subtitle_and_socket() {
        let (_dir, path) = path_with("mpv");
        let h = headers();
        let mut l = launch(&h, Some("/subs/en.vtt"));
        let socket = Path::new("/run/user/1000/kuro-mpv.sock");
        l.ipc_socket = Some(socket);
        let built = plan(&Preset::Mpv, &l, &path).unwrap();
        assert_eq!(
            argv(built),
            vec![
                "--title=Show - Pilot".to_owned(),
                "--start=12.500".to_owned(),
                "--http-header-fields=Referer: https://site,User-Agent: UA/1".to_owned(),
                "--sub-file=/subs/en.vtt".to_owned(),
                "--input-ipc-server=/run/user/1000/kuro-mpv.sock".to_owned(),
                "https://cdn/v.m3u8".to_owned(),
            ]
        );
    }

    #[test]
    fn mpv_argv_without_extras_is_minimal() {
        let (_dir, path) = path_with("mpv");
        let built = argv(plan(&Preset::Mpv, &launch(&[], None), &path).unwrap());
        assert_eq!(
            built,
            vec![
                "--title=Show - Pilot",
                "--start=12.500",
                "https://cdn/v.m3u8"
            ]
        );
    }

    #[test]
    fn vlc_takes_user_agent_and_referrer_not_the_mpv_option() {
        let (_dir, path) = path_with("vlc");
        let h = headers();
        let built = argv(plan(&Preset::Vlc, &launch(&h, Some("/s.srt")), &path).unwrap());
        assert_eq!(
            built,
            vec![
                "--meta-title=Show - Pilot",
                "--start-time=12.500",
                "--http-user-agent=UA/1",
                "--http-referrer=https://site",
                "--sub-file=/s.srt",
                "https://cdn/v.m3u8",
            ]
        );
    }

    #[test]
    fn celluloid_packs_options_and_escapes_commas() {
        let (_dir, path) = path_with("celluloid");
        let h = headers();
        let built = argv(plan(&Preset::Celluloid, &launch(&h, Some("/a,b.srt")), &path).unwrap());
        assert_eq!(
            built[0],
            "--mpv-options=start=12.500,http-header-fields=Referer: https://site\\,User-Agent: UA/1,sub-file=/a\\,b.srt"
        );
        assert_eq!(built[1], "https://cdn/v.m3u8");
    }

    #[test]
    fn custom_template_fills_placeholders_one_argument_per_piece() {
        let (_dir, path) = path_with("myplayer");
        let preset =
            Preset::Custom("myplayer --title={title} {position} {subtitle} {url}".to_owned());
        let built = argv(plan(&preset, &launch(&[], None), &path).unwrap());
        // The title has a space and stays one argument. The empty subtitle is dropped.
        assert_eq!(
            built,
            vec!["--title=Show - Pilot", "12.500", "https://cdn/v.m3u8"]
        );
    }

    #[test]
    fn empty_custom_template_is_an_error() {
        let err = plan(
            &Preset::Custom("   ".to_owned()),
            &launch(&[], None),
            OsStr::new(""),
        )
        .unwrap_err();
        assert_eq!(err, ExternalError::EmptyTemplate);
    }

    #[test]
    fn missing_program_reports_player_not_found() {
        let err = plan(
            &Preset::Mpv,
            &launch(&[], None),
            OsStr::new("/nonexistent-dir"),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Player not found: mpv");
    }

    #[test]
    fn start_rule_truth_table() {
        // Not seen: the saved position, whatever the preserve flag says.
        assert_eq!(start_ms(false, false, 60_000, 100_000), 60_000);
        assert_eq!(start_ms(false, true, 60_000, 100_000), 60_000);
        // Seen and preserve off: restart.
        assert_eq!(start_ms(true, false, 60_000, 100_000), 1);
        // Seen and preserve on: restart only when the saved position is the end.
        assert_eq!(start_ms(true, true, 60_000, 100_000), 60_000);
        assert_eq!(start_ms(true, true, 100_000, 100_000), 1);
    }

    #[test]
    fn subtitle_by_locale_then_first_then_none() {
        let tracks = vec![
            Track {
                url: "/fr.vtt".into(),
                lang: "French".into(),
            },
            Track {
                url: "/en.vtt".into(),
                lang: "English (US)".into(),
            },
        ];
        assert_eq!(select_subtitle(&tracks, "english").unwrap().url, "/en.vtt");
        assert_eq!(select_subtitle(&tracks, "Japanese").unwrap().url, "/fr.vtt");
        assert_eq!(select_subtitle(&[], "English"), None);
    }

    #[test]
    fn completion_within_a_second_of_the_end() {
        let done = final_save(1199.5, 1200.0);
        assert_eq!(done.cause, Some("playback_completion"));
        assert_eq!(done.last_second_seen_ms, 1_200_000);
        assert_eq!(done.total_ms, 1_200_000);

        let partial = final_save(600.0, 1200.0);
        assert_eq!(partial.cause, None);
        assert_eq!(partial.last_second_seen_ms, 600_000);
    }

    #[test]
    fn unknown_duration_is_never_a_completion() {
        let unknown = final_save(0.5, 0.0);
        assert_eq!(unknown.cause, None);
    }

    #[test]
    fn title_joins_anime_and_episode() {
        assert_eq!(title("Show", "Pilot"), "Show - Pilot");
    }

    #[test]
    fn finds_an_executable_and_skips_plain_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notexec"), "x").unwrap();
        let path = dir.path().as_os_str();
        assert_eq!(find_on_path("notexec", path), None);
        let (_dir2, path2) = path_with("mpv");
        assert!(find_on_path("mpv", &path2).is_some());
    }

    #[test]
    fn mime_type_follows_the_extension() {
        assert_eq!(mime_type("https://h/a/ep.MP4?token=1"), "video/mp4");
        assert_eq!(mime_type("/media/show.mkv"), "video/x-matroska");
        assert_eq!(mime_type("https://h/master.m3u8"), "application/x-mpegURL");
        assert_eq!(mime_type("https://h/stream"), "video/any");
        assert_eq!(mime_type("https://h.example/dir.v2/stream"), "video/any");
    }
}
