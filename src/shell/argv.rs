//! Command-line parsing for the launch options `--action`, `--id`, `--search`, `--type` and positional file or URI
//! arguments (T-53a-006, test TC-53-01). `--open`, `--version`, `--run-backup` and `--crash-report` belong to other
//! handlers in `main.rs` and `kuro_platform::deep_link`, so they are skipped here.

use std::path::PathBuf;

use kuro_platform::deep_link::DeepLink;

use super::nav::Kind;

/// The text printed for `--help`.
pub const USAGE: &str = "Usage: kurobayomi [OPTIONS] [FILE.tachibk | URI]...

Options:
  --action=NAME   open a place: show-library, show-animelib, show-updates, show-history, show-browse,
                  show-extensions, show-anime-extensions, show-downloads, show-anime-downloads,
                  show-manga (with --id), show-anime (with --id)
  --id=ID         the entry id for show-manga and show-anime
  --search=TEXT   open global search with TEXT
  --type=KIND     anime (default) or manga, for --search
  --open=LINK     open a deep link, for example updates or manga=7
  --version       print the version and exit
  --run-backup    run one automatic backup and exit
  --help          print this text and exit
";

/// True when the arguments ask for the usage text.
pub fn wants_help(args: &[String]) -> bool {
    args.iter().any(|a| a == "--help" || a == "-h")
}

/// The deep link a desktop action opens. `show-manga` and `show-anime` need an id, so they give `None` without one.
pub fn link_for_action(name: &str, id: Option<i64>) -> Option<DeepLink> {
    Some(match name {
        "show-library" => DeepLink::Library,
        "show-animelib" => DeepLink::AnimeLibrary,
        "show-updates" => DeepLink::Updates,
        "show-history" => DeepLink::History,
        "show-browse" => DeepLink::Sources,
        "show-extensions" => DeepLink::Extensions,
        "show-anime-extensions" => DeepLink::AnimeExtensions,
        "show-downloads" => DeepLink::Downloads,
        "show-anime-downloads" => DeepLink::AnimeDownloads,
        "show-manga" => DeepLink::Manga(id?),
        "show-anime" => DeepLink::Anime(id?),
        _ => return None,
    })
}

/// The desktop action names of the `.desktop` file (behaviour of T-53a-002).
pub const ACTIONS: [&str; 11] = [
    "show-library",
    "show-animelib",
    "show-updates",
    "show-history",
    "show-browse",
    "show-extensions",
    "show-anime-extensions",
    "show-downloads",
    "show-anime-downloads",
    "show-manga",
    "show-anime",
];

/// Actions that need `--id`.
pub fn needs_id(action: &str) -> bool {
    matches!(action, "show-manga" | "show-anime")
}

/// One request to the application, from the command line or an open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchRequest {
    Action { name: String, id: Option<i64> },
    Search { query: String, kind: Kind },
    OpenFile(PathBuf),
    OpenUri(String),
    /// A `--open <link>` deep link.
    Open(DeepLink),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgError {
    MissingValue(String),
    UnknownAction(String),
    UnknownType(String),
    BadId(String),
    UnknownOption(String),
    UnexpectedArgument(String),
    BadLink(String),
}

impl std::fmt::Display for ArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue(o) => write!(f, "{o} needs a value"),
            Self::UnknownAction(a) => write!(f, "unknown action {a}"),
            Self::UnknownType(t) => write!(f, "unknown type {t}, use anime or manga"),
            Self::BadId(v) => write!(f, "--id must be a number, got {v}"),
            Self::UnknownOption(o) => write!(f, "unknown option {o}"),
            Self::UnexpectedArgument(a) => write!(f, "unexpected argument {a}"),
            Self::BadLink(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for ArgError {}

/// Options that take a value and belong to other handlers, so their value is skipped too.
const SKIPPED_WITH_VALUE: [&str; 2] = ["--open", "--crash-report"];
/// Options without a value that belong to other handlers.
const SKIPPED_FLAGS: [&str; 2] = ["--version", "--run-backup"];

/// Parses the arguments after the program name. Requests come out in order: action, search, then files and URIs.
/// An empty `--search=` value produces no request.
pub fn parse_argv(args: &[String]) -> Result<Vec<LaunchRequest>, ArgError> {
    let mut action: Option<String> = None;
    let mut id: Option<i64> = None;
    let mut query: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut positional: Vec<LaunchRequest> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n, Some(v.to_string())),
            _ => (arg, None),
        };
        let value = |i: &mut usize| -> Result<String, ArgError> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| ArgError::MissingValue(name.to_string()))
        };
        match name {
            "--action" => action = Some(value(&mut i)?),
            "--id" => {
                let raw = value(&mut i)?;
                id = Some(raw.parse().map_err(|_| ArgError::BadId(raw.clone()))?);
            }
            "--search" => query = Some(value(&mut i)?),
            "--type" => kind = Some(value(&mut i)?),
            n if SKIPPED_WITH_VALUE.contains(&n) => {
                if inline.is_none() {
                    i += 1;
                }
            }
            n if SKIPPED_FLAGS.contains(&n) => {}
            n if n.starts_with("--") => return Err(ArgError::UnknownOption(arg.to_string())),
            _ if arg.ends_with(".tachibk") => positional.push(LaunchRequest::OpenFile(PathBuf::from(arg))),
            _ if arg.contains("://") => positional.push(LaunchRequest::OpenUri(arg.to_string())),
            _ => return Err(ArgError::UnexpectedArgument(arg.to_string())),
        }
        i += 1;
    }

    let mut out = Vec::new();
    if let Some(name) = action {
        if !ACTIONS.contains(&name.as_str()) {
            return Err(ArgError::UnknownAction(name));
        }
        out.push(LaunchRequest::Action { name: name.clone(), id: if needs_id(&name) { id } else { None } });
    }
    if let Some(query) = query.filter(|q| !q.is_empty()) {
        let kind = match kind.as_deref().map(str::to_ascii_lowercase).as_deref() {
            None | Some("anime") => Kind::Anime,
            Some("manga") => Kind::Manga,
            Some(other) => return Err(ArgError::UnknownType(other.to_string())),
        };
        out.push(LaunchRequest::Search { query, kind });
    }
    out.extend(positional);
    Ok(out)
}

/// Parses every launch option: `--open` first, then [`parse_argv`]. Used for the local command line and for the
/// command line a second launch forwards through GApplication.
pub fn parse_launch(args: &[String]) -> Result<Vec<LaunchRequest>, ArgError> {
    let link = kuro_platform::deep_link::open_from_args(args).map_err(|e| ArgError::BadLink(e.0))?;
    let mut out: Vec<LaunchRequest> = link.into_iter().map(LaunchRequest::Open).collect();
    out.extend(parse_argv(args)?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn action_with_and_without_an_id() {
        assert_eq!(parse_argv(&argv(&["--action=show-updates"])), Ok(vec![LaunchRequest::Action { name: "show-updates".into(), id: None }]));
        assert_eq!(parse_argv(&argv(&["--action", "show-manga", "--id", "7"])), Ok(vec![LaunchRequest::Action { name: "show-manga".into(), id: Some(7) }]));
    }

    #[test]
    fn id_is_dropped_for_actions_that_do_not_use_it() {
        assert_eq!(parse_argv(&argv(&["--action=show-history", "--id=3"])), Ok(vec![LaunchRequest::Action { name: "show-history".into(), id: None }]));
    }

    #[test]
    fn unknown_action_and_bad_id_are_errors() {
        assert_eq!(parse_argv(&argv(&["--action=nope"])), Err(ArgError::UnknownAction("nope".into())));
        assert_eq!(parse_argv(&argv(&["--action=show-anime", "--id=x"])), Err(ArgError::BadId("x".into())));
    }

    #[test]
    fn search_defaults_to_anime_and_reads_manga() {
        assert_eq!(parse_argv(&argv(&["--search=frieren"])), Ok(vec![LaunchRequest::Search { query: "frieren".into(), kind: Kind::Anime }]));
        assert_eq!(parse_argv(&argv(&["--search", "berserk", "--type", "manga"])), Ok(vec![LaunchRequest::Search { query: "berserk".into(), kind: Kind::Manga }]));
    }

    #[test]
    fn empty_search_is_a_no_op_and_unknown_type_is_an_error() {
        assert_eq!(parse_argv(&argv(&["--search="])), Ok(vec![]));
        assert_eq!(parse_argv(&argv(&["--search=x", "--type=video"])), Err(ArgError::UnknownType("video".into())));
    }

    #[test]
    fn files_and_uris_are_positional_in_order() {
        let got = parse_argv(&argv(&["/tmp/backup.tachibk", "aniyomi://add-repo?url=x"])).unwrap();
        assert_eq!(got, vec![LaunchRequest::OpenFile(PathBuf::from("/tmp/backup.tachibk")), LaunchRequest::OpenUri("aniyomi://add-repo?url=x".into())]);
    }

    #[test]
    fn other_handlers_options_are_skipped_with_their_values() {
        assert_eq!(parse_argv(&argv(&["--open", "updates", "--version"])), Ok(vec![]));
        assert_eq!(parse_argv(&argv(&["--open=manga=7", "--run-backup"])), Ok(vec![]));
        assert_eq!(parse_argv(&argv(&["--crash-report", "/tmp/x.json"])), Ok(vec![]));
    }

    #[test]
    fn stray_arguments_are_rejected() {
        assert_eq!(parse_argv(&argv(&["--bogus"])), Err(ArgError::UnknownOption("--bogus".into())));
        assert_eq!(parse_argv(&argv(&["notes.txt"])), Err(ArgError::UnexpectedArgument("notes.txt".into())));
        assert_eq!(parse_argv(&argv(&["--search"])), Err(ArgError::MissingValue("--search".into())));
    }

    #[test]
    fn action_comes_before_search_and_files() {
        let got = parse_argv(&argv(&["x.tachibk", "--search=a", "--action=show-library"])).unwrap();
        assert_eq!(got.len(), 3);
        assert!(matches!(got[0], LaunchRequest::Action { .. }));
        assert!(matches!(got[1], LaunchRequest::Search { .. }));
        assert!(matches!(got[2], LaunchRequest::OpenFile(_)));
    }

    #[test]
    fn every_desktop_action_maps_to_a_deep_link() {
        for name in ACTIONS {
            let id = needs_id(name).then_some(5);
            assert!(link_for_action(name, id).is_some(), "{name}");
        }
        assert_eq!(link_for_action("show-updates", None), Some(DeepLink::Updates));
        assert_eq!(link_for_action("show-manga", Some(7)), Some(DeepLink::Manga(7)));
        assert_eq!(link_for_action("show-anime", None), None, "an entry action without an id opens nothing");
        assert_eq!(link_for_action("nope", None), None);
    }

    #[test]
    fn parse_launch_puts_the_open_link_first_and_reports_a_bad_link() {
        let got = parse_launch(&argv(&["--search=a", "--open", "updates"])).unwrap();
        assert_eq!(got[0], LaunchRequest::Open(DeepLink::Updates));
        assert!(matches!(got[1], LaunchRequest::Search { .. }));
        assert!(matches!(parse_launch(&argv(&["--open=nowhere"])), Err(ArgError::BadLink(_))));
        assert_eq!(parse_launch(&argv(&[])), Ok(vec![]));
    }

    #[test]
    fn help_is_asked_for_by_either_spelling() {
        assert!(wants_help(&argv(&["--help"])));
        assert!(wants_help(&argv(&["--action=show-library", "-h"])));
        assert!(!wants_help(&argv(&["--search=help"])));
        assert!(USAGE.contains("--action") && USAGE.contains("--search") && USAGE.contains("--type"));
    }
}
