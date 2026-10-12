//! `kurobayomi --updates-json` end to end (T-50a-015 to T-50a-017, T-50a-035): golden envelopes, exit codes, the
//! read-only open, and the no-display run. The binary runs with a cleared environment, so it cannot see a display.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const NOW: &str = "1791460800000";
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/feed");

struct Env {
    home: tempfile::TempDir,
}

impl Env {
    fn new() -> Env {
        Env { home: tempfile::tempdir().unwrap() }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.home.path().join(name)
    }

    fn manga_fixture(&self) -> PathBuf {
        let path = self.path("a.db");
        kuro_db::schema_unified::open_file(&path).unwrap().execute_batch(&fixture("manga.sql")).unwrap();
        path
    }

    fn anime_fixture(&self) -> PathBuf {
        let path = self.path("b.db");
        kuro_db::schema_unified::open_file(&path).unwrap().execute_batch(&fixture("anime.sql")).unwrap();
        path
    }

    /// Runs the binary with only HOME, PATH and a UTC zone: no display, no config from the real user.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_kurobayomi"))
            .env_clear()
            .env("HOME", self.home.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("TZ", "UTC")
            .args(args)
            .output()
            .unwrap()
    }

    /// stdout with the temp directory and the checkout replaced by fixed names, so goldens are machine independent.
    fn stdout(&self, out: &Output) -> String {
        String::from_utf8(out.stdout.clone())
            .unwrap()
            .replace(self.home.path().to_str().unwrap(), "$TMP")
            .replace(env!("CARGO_MANIFEST_DIR"), "$ROOT")
    }
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(FIXTURES).join(name)).unwrap()
}

/// Compares with `tests/fixtures/feed/<name>`; `UPDATE_GOLDEN=1` rewrites the file.
fn assert_golden(name: &str, got: &str) {
    let path = Path::new(FIXTURES).join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, got).unwrap();
    }
    assert_eq!(got, std::fs::read_to_string(&path).unwrap(), "golden {name}");
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap()
}

fn stderr_lines(out: &Output) -> usize {
    String::from_utf8_lossy(&out.stderr).lines().count()
}

#[test]
fn normal_manga_envelope_matches_the_golden_file() {
    let env = Env::new();
    let db = env.manga_fixture();
    let out = env.run(&[
        "--updates-json", "--kind", "manga", "--rows", "2", "--cols", "3", "--now-ms", NOW,
        "--db", db.to_str().unwrap(), "--cover-cache", &format!("{FIXTURES}/covers"),
    ]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stderr_lines(&out), 0);
    let text = env.stdout(&out);
    assert_golden("manga_normal.json", &text);
    // titles with quotes, a newline and an RTL override round-trip
    let v = json(&String::from_utf8(out.stdout).unwrap());
    assert_eq!(v["items"][1]["title"], "Beta \"Quoted\"\nline \u{202e}RTL");
    assert!(v["items"][0]["cover_path"].as_str().unwrap().ends_with(".png"));
    assert!(v["items"][1]["cover_path"].is_null());
}

#[test]
fn one_column_three_rows_gives_three_items_in_feed_order() {
    let env = Env::new();
    let db = env.manga_fixture();
    let out = env.run(&["--updates-json", "--kind", "manga", "--rows", "3", "--cols", "1", "--now-ms", NOW, "--db", db.to_str().unwrap()]);
    let v = json(&String::from_utf8(out.stdout).unwrap());
    let ids: Vec<i64> = v["items"].as_array().unwrap().iter().map(|i| i["entry_id"].as_i64().unwrap()).collect();
    assert_eq!(ids, vec![1, 2, 4]);
    assert_eq!(v["grid"]["cap"], 3);
}

#[test]
fn normal_anime_envelope_matches_the_golden_file() {
    let env = Env::new();
    let db = env.anime_fixture();
    let out = env.run(&["--updates-json", "--kind", "anime", "--now-ms", NOW, "--db", db.to_str().unwrap(), "--cover-cache", &format!("{FIXTURES}/covers")]);
    assert_eq!(out.status.code(), Some(0));
    assert_golden("anime_normal.json", &env.stdout(&out));
}

#[test]
fn empty_envelope_matches_the_golden_file() {
    let env = Env::new();
    let db = env.manga_fixture();
    let out = env.run(&["--updates-json", "--kind", "manga", "--now-ms", "1900000000000", "--db", db.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    assert_golden("manga_empty.json", &env.stdout(&out));
}

#[test]
fn exit_2_usage_matches_the_golden_file() {
    let env = Env::new();
    let out = env.run(&["--updates-json", "--kind", "manga", "--rows", "11"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stderr_lines(&out), 1);
    assert_golden("error_usage.json", &env.stdout(&out));
}

#[test]
fn exit_3_missing_database_matches_the_golden_file_and_creates_nothing() {
    let env = Env::new();
    let db = env.path("data/a.db");
    let out = env.run(&["--updates-json", "--kind", "manga", "--db", db.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(stderr_lines(&out), 1);
    assert_golden("error_db_missing.json", &env.stdout(&out));
    assert!(!db.exists() && !db.parent().unwrap().exists());
}

#[test]
fn exit_3_for_a_file_that_is_not_a_database() {
    let env = Env::new();
    let db = env.path("junk.db");
    std::fs::write(&db, b"this is not sqlite, not even close to a database header").unwrap();
    let out = env.run(&["--updates-json", "--kind", "manga", "--db", db.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(stderr_lines(&out), 1);
    assert_eq!(json(&String::from_utf8(out.stdout).unwrap())["code"], "db_error");
}

#[test]
fn the_database_file_is_never_written() {
    let env = Env::new();
    let db = env.manga_fixture();
    let before = std::fs::read(&db).unwrap();
    let names = |p: &Path| {
        // SQLite's own WAL index and log appear beside any WAL database a reader opens; nothing else may
        let mut v: Vec<_> = std::fs::read_dir(p)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| !n.to_string_lossy().ends_with("-shm") && !n.to_string_lossy().ends_with("-wal"))
            .collect();
        v.sort();
        v
    };
    let files_before = names(env.home.path());
    let out = env.run(&["--updates-json", "--kind", "manga", "--now-ms", NOW, "--db", db.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(std::fs::read(&db).unwrap(), before);
    assert_eq!(names(env.home.path()), files_before);
}

/// 500 distinct favourites with one unread update each: the whole run stays under 50 ms (T-50a-035).
#[test]
#[ignore = "benchmark"]
fn five_hundred_rows_finish_under_50_ms() {
    let env = Env::new();
    let db = env.path("a.db");
    let c = kuro_db::schema_unified::open_file(&db).unwrap();
    let after = 1_783_512_000_000_i64;
    for i in 1..=500 {
        c.execute(
            "INSERT INTO entries(_id,kind,source,url,title,status,favorite,initialized,viewer,item_flags,cover_last_modified,date_added) VALUES(?1,'manga',1,'/m'||?1,'T',1,1,1,0,0,0,1000)",
            [i],
        )
        .unwrap();
        c.execute(
            "INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(?1,?1,'/c','C',0,0,0,1.0,0,5000,?2)",
            rusqlite::params![i, after + 1000],
        )
        .unwrap();
    }
    drop(c);
    let best = (0..5)
        .map(|_| {
            let started = std::time::Instant::now();
            let out = env.run(&["--updates-json", "--kind", "manga", "--rows", "10", "--cols", "10", "--now-ms", NOW, "--db", db.to_str().unwrap()]);
            assert_eq!(out.status.code(), Some(0));
            started.elapsed()
        })
        .min()
        .unwrap();
    assert!(best < std::time::Duration::from_millis(50), "{best:?}");
}
