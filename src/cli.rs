//! `kurobayomi --updates-json` (T-50a-015 to T-50a-017, T-50a-034): prints the updates feed as one JSON document and
//! exits. It runs before GTK starts, opens the database read-only, and never writes.
//!
//! Exit codes: 0 ok (empty feeds included), 2 usage, 3 database, 4 preferences. On failure stdout is
//! `{"schema":1,"error":...,"code":...}` and stderr has one human line.

use std::io::Write;
use std::path::PathBuf;

use kuro_platform::paths::Paths;

use crate::feed::{self, ErrorEnvelope, FeedError, Kind, Request, cover, grid};

/// True when the command line asks for the feed.
pub fn wants_updates_json(args: &[String]) -> bool {
    args.iter().any(|a| a == "--updates-json")
}

/// Where the feed reads from; the platform directories unless the flags override them.
struct Dirs {
    config_file: PathBuf,
    db: PathBuf,
    covers: PathBuf,
}

struct Parsed {
    kind: Kind,
    rows: u32,
    cols: u32,
    db: Option<PathBuf>,
    cover_cache: Option<PathBuf>,
    scale: u32,
    now_ms: Option<i64>,
}

fn usage(msg: impl Into<String>) -> FeedError {
    FeedError::Usage(msg.into())
}

fn ranged(flag: &str, v: &str, lo: u32, hi: u32) -> Result<u32, FeedError> {
    v.parse::<u32>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(|| usage(format!("{flag} needs a number from {lo} to {hi}, got {v:?}")))
}

fn parse(args: &[String]) -> Result<Parsed, FeedError> {
    let (mut kind, mut rows, mut cols) = (None, grid::DEFAULT_ROWS, grid::DEFAULT_COLS);
    let (mut db, mut cache, mut scale, mut now_ms) = (None, None, cover::DEFAULT_SCALE, None);
    let mut it = args.iter().map(String::as_str);
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v)),
            _ => (arg, None),
        };
        if flag == "--updates-json" {
            continue;
        }
        let mut value = || inline.or_else(|| it.next()).ok_or_else(|| usage(format!("{flag} needs a value")));
        match flag {
            "--kind" => {
                let v = value()?;
                kind = Some(Kind::parse(v).ok_or_else(|| usage(format!("--kind must be manga or anime, got {v:?}")))?);
            }
            "--rows" => rows = ranged(flag, value()?, 1, grid::MAX_ROWS)?,
            "--cols" => cols = ranged(flag, value()?, 1, grid::MAX_COLS)?,
            "--device-scale" => scale = ranged(flag, value()?, 1, 4)?,
            "--db" => db = Some(PathBuf::from(value()?)),
            "--cover-cache" => cache = Some(PathBuf::from(value()?)),
            "--now-ms" => {
                let v = value()?;
                now_ms = Some(v.parse::<i64>().map_err(|_| usage(format!("--now-ms needs an integer, got {v:?}")))?);
            }
            other => return Err(usage(format!("unknown option {other}"))),
        }
    }
    let kind = kind.ok_or_else(|| usage("--kind <manga|anime> is required"))?;
    Ok(Parsed { kind, rows, cols, db, cover_cache: cache, scale, now_ms })
}

fn run_feed(args: &[String], paths: Result<&dyn Paths, String>, now: i64) -> Result<String, FeedError> {
    let p = parse(args)?;
    let paths = paths.map_err(FeedError::Prefs)?;
    let dirs = Dirs {
        config_file: kuro_prefs::toml_backend::config_file(&paths.config_dir()),
        db: p.db.unwrap_or_else(|| kuro_db::schema_unified::db_path(&paths.data_dir())),
        covers: p.cover_cache.unwrap_or_else(|| paths.widget_covers_dir()),
    };
    let req = Request { kind: p.kind, rows: p.rows, cols: p.cols, now_ms: p.now_ms.unwrap_or(now) };
    let env = feed::build_envelope(
        &req,
        |after, limit| {
            let conn = feed::query::open_read_only(&dirs.db)?;
            feed::query::unread_updates(&conn, p.kind, after, limit)
        },
        |row| cover::cached_path(&dirs.covers, row),
    )?;
    serde_json::to_string(&env).map_err(|e| FeedError::Db(e.to_string()))
}

/// Runs the command with explicit output sinks and platform paths. Returns the process exit code.
pub fn run_with(args: &[String], paths: Result<&dyn Paths, String>, now_ms: i64, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match run_feed(args, paths, now_ms) {
        Ok(json) => {
            let _ = writeln!(out, "{json}");
            0
        }
        Err(e) => {
            let json = serde_json::to_string(&ErrorEnvelope::from(&e)).unwrap_or_default();
            let _ = writeln!(out, "{json}");
            // one human line: the message cannot carry a newline
            let _ = writeln!(err, "kurobayomi: {}", e.message().replace(['\n', '\r'], " "));
            e.exit_code()
        }
    }
}

/// Entry point called from `main` before GTK exists.
pub fn run(args: &[String]) -> u8 {
    let system = kuro_platform::linux::LinuxPaths::system().map_err(|e| format!("cannot resolve the app directories: {e}"));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    run_with(args, system.as_ref().map(|p| p as &dyn Paths).map_err(Clone::clone), now, &mut std::io::stdout(), &mut std::io::stderr())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    fn go(a: &str, paths: &dyn Paths) -> (u8, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(&args(a), Ok(paths), 1_791_460_800_000, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    #[test]
    fn parsing_defaults_and_bounds() {
        let p = parse(&args("--updates-json --kind manga")).unwrap();
        assert_eq!((p.rows, p.cols, p.scale), (2, 4, 2));
        let p = parse(&args("--updates-json --kind=anime --rows=3 --cols 1 --device-scale 4 --now-ms 7")).unwrap();
        assert_eq!((p.kind, p.rows, p.cols, p.scale, p.now_ms), (Kind::Anime, 3, 1, 4, Some(7)));
        for bad in [
            "--updates-json",
            "--updates-json --kind comics",
            "--updates-json --kind manga --rows 0",
            "--updates-json --kind manga --rows 11",
            "--updates-json --kind manga --cols x",
            "--updates-json --kind manga --device-scale 5",
            "--updates-json --kind manga --bogus",
            "--updates-json --kind manga --db",
        ] {
            assert_eq!(parse(&args(bad)).err().map(|e| e.exit_code()), Some(2), "{bad}");
        }
    }

    #[test]
    fn usage_error_has_an_envelope_and_one_stderr_line() {
        let paths = kuro_testkit::TempPaths::new().unwrap();
        let (code, out, err) = go("--updates-json --kind manga --rows 99", &paths);
        assert_eq!(code, 2);
        assert!(out.starts_with("{\"schema\":1,\"error\":\""));
        assert!(out.trim_end().ends_with("\"code\":\"usage\"}"));
        assert_eq!(err.lines().count(), 1);
    }

    #[test]
    fn missing_database_is_exit_3_and_stays_missing() {
        let paths = kuro_testkit::TempPaths::new().unwrap();
        let db = kuro_db::schema_unified::db_path(&paths.data_dir());
        let (code, out, _) = go("--updates-json --kind manga", &paths);
        assert_eq!(code, 3);
        assert!(out.contains("\"code\":\"db_missing\""));
        assert!(!db.exists());
    }

    #[test]
    fn unresolvable_directories_are_a_preferences_error() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(&args("--updates-json --kind manga"), Err("no home".into()), 0, &mut out, &mut err);
        assert_eq!(code, 4);
    }
}
