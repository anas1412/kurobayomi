//! Deep links: the eleven places a second launch or a notification can open (`--open <name>[=<id>]`).

use std::fmt;

/// One deep-link destination. `Manga` and `Anime` carry the entry id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeepLink {
    AnimeLibrary,
    Library,
    Manga(i64),
    Anime(i64),
    Updates,
    History,
    Sources,
    AnimeExtensions,
    Extensions,
    AnimeDownloads,
    Downloads,
}

/// Every variant name without its id, in the order of the `app.open-*` actions.
pub const NAMES: [&str; 11] = [
    "anime-library",
    "library",
    "manga",
    "anime",
    "updates",
    "history",
    "sources",
    "anime-extensions",
    "extensions",
    "anime-downloads",
    "downloads",
];

impl DeepLink {
    /// Parses `name` or `name=<id>`. `manga` and `anime` need an integer id; anything else gives `None`.
    pub fn parse(arg: &str) -> Option<DeepLink> {
        let (name, id) = match arg.split_once('=') {
            Some((name, id)) => (name, Some(id)),
            None => (arg, None),
        };
        let needs_id = |id: Option<&str>| id.and_then(|v| v.parse::<i64>().ok());
        Some(match (name, id) {
            ("manga", _) => DeepLink::Manga(needs_id(id)?),
            ("anime", _) => DeepLink::Anime(needs_id(id)?),
            ("anime-library", None) => DeepLink::AnimeLibrary,
            ("library", None) => DeepLink::Library,
            ("updates", None) => DeepLink::Updates,
            ("history", None) => DeepLink::History,
            ("sources", None) => DeepLink::Sources,
            ("anime-extensions", None) => DeepLink::AnimeExtensions,
            ("extensions", None) => DeepLink::Extensions,
            ("anime-downloads", None) => DeepLink::AnimeDownloads,
            ("downloads", None) => DeepLink::Downloads,
            _ => return None,
        })
    }

    /// The name accepted by [`DeepLink::parse`], without the id.
    pub fn name(&self) -> &'static str {
        match self {
            DeepLink::AnimeLibrary => "anime-library",
            DeepLink::Library => "library",
            DeepLink::Manga(_) => "manga",
            DeepLink::Anime(_) => "anime",
            DeepLink::Updates => "updates",
            DeepLink::History => "history",
            DeepLink::Sources => "sources",
            DeepLink::AnimeExtensions => "anime-extensions",
            DeepLink::Extensions => "extensions",
            DeepLink::AnimeDownloads => "anime-downloads",
            DeepLink::Downloads => "downloads",
        }
    }

    /// The application action that performs this link, for example `app.open-manga`.
    pub fn action_name(&self) -> String {
        format!("app.open-{}", self.name())
    }

    /// The text form that round-trips through [`DeepLink::parse`], for example `manga=12` or `updates`.
    pub fn to_arg(&self) -> String {
        match self {
            DeepLink::Manga(id) | DeepLink::Anime(id) => format!("{}={id}", self.name()),
            _ => self.name().to_owned(),
        }
    }
}

/// A command line that does not name a valid deep link. The process exits with code 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Exit code for a rejected command line.
pub const USAGE_EXIT_CODE: i32 = 2;

/// Finds `--open <arg>` or `--open=<arg>` in the argument list (without the program name).
/// `Ok(None)` when the flag is absent; an unknown name or a missing id is a [`UsageError`].
pub fn open_from_args<S: AsRef<str>>(args: &[S]) -> Result<Option<DeepLink>, UsageError> {
    let mut iter = args.iter().map(AsRef::as_ref);
    while let Some(arg) = iter.next() {
        let value = if arg == "--open" {
            match iter.next() {
                Some(v) => v,
                None => return Err(UsageError("--open needs a value, for example --open updates".into())),
            }
        } else if let Some(v) = arg.strip_prefix("--open=") {
            v
        } else {
            continue;
        };
        return DeepLink::parse(value)
            .map(Some)
            .ok_or_else(|| UsageError(format!("unknown or incomplete deep link: {value}")));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_round_trips_through_its_text_form() {
        let all = [
            DeepLink::AnimeLibrary,
            DeepLink::Library,
            DeepLink::Manga(12),
            DeepLink::Anime(3),
            DeepLink::Updates,
            DeepLink::History,
            DeepLink::Sources,
            DeepLink::AnimeExtensions,
            DeepLink::Extensions,
            DeepLink::AnimeDownloads,
            DeepLink::Downloads,
        ];
        assert_eq!(all.len(), NAMES.len());
        for link in all {
            assert_eq!(DeepLink::parse(&link.to_arg()), Some(link.clone()), "{}", link.to_arg());
            assert!(NAMES.contains(&link.name()));
            assert_eq!(link.action_name(), format!("app.open-{}", link.name()));
        }
    }

    #[test]
    fn manga_and_anime_need_an_integer_id() {
        assert_eq!(DeepLink::parse("manga"), None);
        assert_eq!(DeepLink::parse("manga=abc"), None);
        assert_eq!(DeepLink::parse("anime="), None);
        assert_eq!(DeepLink::parse("manga=12"), Some(DeepLink::Manga(12)));
    }

    #[test]
    fn unknown_names_and_ids_on_plain_names_are_rejected() {
        assert_eq!(DeepLink::parse("nope"), None);
        assert_eq!(DeepLink::parse("updates=4"), None);
        assert_eq!(DeepLink::parse(""), None);
    }

    #[test]
    fn action_names_are_the_eleven_app_open_actions() {
        let names: Vec<String> = NAMES
            .iter()
            .map(|n| DeepLink::parse(n).or_else(|| DeepLink::parse(&format!("{n}=1"))).unwrap().action_name())
            .collect();
        assert_eq!(names.len(), 11);
        assert!(names.contains(&"app.open-anime-library".to_owned()));
        assert!(names.contains(&"app.open-downloads".to_owned()));
    }

    #[test]
    fn open_flag_is_read_in_both_spellings() {
        assert_eq!(open_from_args(&["--open", "updates"]), Ok(Some(DeepLink::Updates)));
        assert_eq!(open_from_args(&["--open=manga=7"]), Ok(Some(DeepLink::Manga(7))));
        assert_eq!(open_from_args::<&str>(&[]), Ok(None));
        assert_eq!(open_from_args(&["--other", "x"]), Ok(None));
    }

    #[test]
    fn missing_id_or_unknown_name_is_a_usage_error() {
        assert!(open_from_args(&["--open", "manga"]).is_err());
        assert!(open_from_args(&["--open", "manga=abc"]).is_err());
        assert!(open_from_args(&["--open", "bogus"]).is_err());
        assert!(open_from_args(&["--open"]).is_err());
        assert_eq!(USAGE_EXIT_CODE, 2);
    }
}
