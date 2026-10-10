//! The updates feed shared by the `--updates-json` command line and the desktop applets (T-50a-001 to T-50a-017).
//! Pure data and rules; nothing here touches GTK. Port of the Aniyomi widget feed: recent window, unread filter,
//! one cell per entry, a cap of rows x cols, and the app-lock rule.

pub mod cover;
pub mod grid;
pub mod lock;
pub mod query;
pub mod window;

use std::collections::HashSet;

use serde::Serialize;

/// Which library a feed reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Manga,
    Anime,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "manga" => Some(Kind::Manga),
            "anime" => Some(Kind::Anime),
            _ => None,
        }
    }
}

/// One unread update as the database returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateRow {
    pub entry_id: i64,
    pub title: String,
    /// `chapterId` (manga) or `episodeId` (anime).
    pub update_id: i64,
    pub thumbnail_url: Option<String>,
    pub cover_last_modified: i64,
}

/// Why a feed run failed. The exit code and the `code` of the error envelope come from here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedError {
    Usage(String),
    DbMissing(String),
    DbBusy,
    Db(String),
    Prefs(String),
}

impl FeedError {
    pub fn code(&self) -> &'static str {
        match self {
            FeedError::Usage(_) => "usage",
            FeedError::DbMissing(_) => "db_missing",
            FeedError::DbBusy => "db_busy",
            FeedError::Db(_) => "db_error",
            FeedError::Prefs(_) => "prefs_error",
        }
    }

    /// 2 usage, 3 database, 4 preferences.
    pub fn exit_code(&self) -> u8 {
        match self {
            FeedError::Usage(_) => 2,
            FeedError::DbMissing(_) | FeedError::DbBusy | FeedError::Db(_) => 3,
            FeedError::Prefs(_) => 4,
        }
    }

    pub fn message(&self) -> String {
        match self {
            FeedError::Usage(m) | FeedError::Db(m) | FeedError::Prefs(m) => m.clone(),
            FeedError::DbMissing(path) => format!("database not found: {path}"),
            FeedError::DbBusy => "database busy".into(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GridOut {
    pub rows: u32,
    pub cols: u32,
    pub cap: u32,
}

#[derive(Debug, Serialize)]
pub struct Item {
    pub entry_id: i64,
    pub kind: Kind,
    pub title: String,
    pub unread_update_id: i64,
    pub cover_path: Option<String>,
}

/// The schema-1 success payload. Field order is the order of `p1-data-and-config.md` 11.1.
#[derive(Debug, Serialize)]
pub struct Envelope {
    pub schema: u32,
    pub generated_at_ms: i64,
    pub kind: Kind,
    pub locked: bool,
    pub grid: GridOut,
    pub window_after_ms: i64,
    pub items: Vec<Item>,
}

/// The schema-1 failure payload printed on stdout next to a non-zero exit.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub schema: u32,
    pub error: String,
    pub code: &'static str,
}

impl From<&FeedError> for ErrorEnvelope {
    fn from(e: &FeedError) -> Self {
        ErrorEnvelope { schema: 1, error: e.message(), code: e.code() }
    }
}

/// Keeps the first row of each entry in input order and stops at `cap` (a row without a cover is kept).
pub fn dedupe_and_cap(rows: Vec<UpdateRow>, cap: usize) -> Vec<UpdateRow> {
    let mut seen = HashSet::new();
    rows.into_iter().filter(|r| seen.insert(r.entry_id)).take(cap).collect()
}

/// What one feed request asks for.
pub struct Request {
    pub kind: Kind,
    pub rows: u32,
    pub cols: u32,
    pub now_ms: i64,
}

/// Builds the payload. The lock is checked first; when locked, `query` is never called and no title or cover leaves
/// the process (T-50a-009).
pub fn build_envelope(
    req: &Request,
    is_locked: impl FnOnce() -> Result<bool, FeedError>,
    query: impl FnOnce(i64, usize) -> Result<Vec<UpdateRow>, FeedError>,
    cover_path: impl Fn(&UpdateRow) -> Option<String>,
) -> Result<Envelope, FeedError> {
    let cap = req.rows * req.cols;
    let after = window::window_start_ms(req.now_ms);
    let locked = is_locked()?;
    let items = if locked {
        Vec::new()
    } else {
        dedupe_and_cap(query(after, grid::DB_LIMIT)?, cap as usize)
            .into_iter()
            .map(|r| Item {
                entry_id: r.entry_id,
                kind: req.kind,
                cover_path: cover_path(&r),
                unread_update_id: r.update_id,
                title: r.title,
            })
            .collect()
    };
    Ok(Envelope {
        schema: 1,
        generated_at_ms: req.now_ms,
        kind: req.kind,
        locked,
        grid: GridOut { rows: req.rows, cols: req.cols, cap },
        window_after_ms: after,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entry: i64, title: &str) -> UpdateRow {
        UpdateRow { entry_id: entry, title: title.into(), update_id: entry * 10, thumbnail_url: None, cover_last_modified: 0 }
    }

    #[test]
    fn dedupe_keeps_the_first_row_per_entry() {
        let out = dedupe_and_cap(vec![row(1, "a"), row(2, "b"), row(1, "c")], 8);
        assert_eq!(out.iter().map(|r| (r.entry_id, r.title.as_str())).collect::<Vec<_>>(), vec![(1, "a"), (2, "b")]);
    }

    #[test]
    fn cap_stops_after_that_many_distinct_entries() {
        let rows: Vec<_> = (1..=20).map(|i| row(i, "t")).collect();
        let out = dedupe_and_cap(rows, 8);
        assert_eq!(out.iter().map(|r| r.entry_id).collect::<Vec<_>>(), (1..=8).collect::<Vec<_>>());
    }

    #[test]
    fn a_locked_feed_never_queries() {
        let req = Request { kind: Kind::Manga, rows: 2, cols: 4, now_ms: 0 };
        let env = build_envelope(&req, || Ok(true), |_, _| panic!("queried while locked"), |_| panic!("cover looked up")).unwrap();
        assert!(env.locked && env.items.is_empty());
        let json = serde_json::to_string(&env).unwrap();
        for key in ["title", "cover_path", "unread_update_id"] {
            assert!(!json.contains(key), "{key} leaked");
        }
    }

    #[test]
    fn a_lock_read_error_fails_closed() {
        let req = Request { kind: Kind::Anime, rows: 1, cols: 1, now_ms: 0 };
        let err = build_envelope(&req, || Err(FeedError::Prefs("x".into())), |_, _| panic!("queried"), |_| None).unwrap_err();
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn hostile_titles_round_trip_through_json() {
        let title = "a \"quoted\"\nline \u{202e}rtl \\ tab\t";
        let req = Request { kind: Kind::Manga, rows: 1, cols: 1, now_ms: 0 };
        let env = build_envelope(&req, || Ok(false), |_, _| Ok(vec![row(1, title)]), |_| None).unwrap();
        let back: serde_json::Value = serde_json::from_str(&serde_json::to_string(&env).unwrap()).unwrap();
        assert_eq!(back["items"][0]["title"], title);
        assert_eq!(back["items"][0]["cover_path"], serde_json::Value::Null);
    }

    #[test]
    fn exit_codes_and_codes() {
        let cases = [
            (FeedError::Usage("u".into()), 2, "usage"),
            (FeedError::DbMissing("p".into()), 3, "db_missing"),
            (FeedError::DbBusy, 3, "db_busy"),
            (FeedError::Db("d".into()), 3, "db_error"),
            (FeedError::Prefs("p".into()), 4, "prefs_error"),
        ];
        for (e, exit, code) in cases {
            assert_eq!((e.exit_code(), e.code()), (exit, code));
        }
    }

    // ---- applet packages (T-50a-018, T-50a-020, T-50a-022, T-50a-024) ----

    fn dist(path: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("dist").join(path)
    }

    #[test]
    fn both_applets_share_their_qml_and_differ_in_metadata() {
        for file in ["contents/ui/main.qml", "contents/ui/CoverTile.qml", "contents/code/grid.js"] {
            let manga = std::fs::read(dist(&format!("plasmoid/org.kurobayomi.updates.manga/{file}"))).unwrap();
            let anime = std::fs::read(dist(&format!("plasmoid/org.kurobayomi.updates.anime/{file}"))).unwrap();
            assert_eq!(manga, anime, "{file} differs between the packages");
        }
        for (kind, name) in [("manga", "Updates"), ("anime", "Anime Updates")] {
            let dir = format!("plasmoid/org.kurobayomi.updates.{kind}");
            let meta: serde_json::Value =
                serde_json::from_slice(&std::fs::read(dist(&format!("{dir}/metadata.json"))).unwrap()).unwrap();
            assert_eq!(meta["KPackageStructure"], "Plasma/Applet");
            assert_eq!(meta["KPlugin"]["Id"], format!("org.kurobayomi.updates.{kind}"));
            assert_eq!(meta["KPlugin"]["Name"], name);
            assert_eq!(meta["KPlugin"]["Description"], "See your recently updated library entries");
            assert!(!dist(&format!("{dir}/contents/ui/ConfigGeneral.qml")).exists());
        }
    }

    #[test]
    fn applet_behaviour_is_in_the_qml() {
        let qml = std::fs::read_to_string(dist("plasmoid/org.kurobayomi.updates.manga/contents/ui/main.qml")).unwrap();
        // 60 s poll run at start, 300 ms resize debounce, 6 s kill, stderr through console.warn
        assert!(qml.contains("interval: 60000") && qml.contains("triggeredOnStart: true"));
        assert!(qml.contains("interval: 300") && qml.contains("interval: 6000"));
        assert!(qml.contains("console.warn"));
        // the clear model on lock, and the locked text key
        assert!(qml.contains("payload.locked ? [] : payload.items"));
        assert!(qml.contains("Widget not available when app lock is enabled"));
        // the empty text is the manga one on both kinds
        assert!(qml.contains("No recent updates") && !qml.contains("Nothing watched recently"));
        let tile = std::fs::read_to_string(dist("plasmoid/org.kurobayomi.updates.manga/contents/ui/CoverTile.qml")).unwrap();
        for needle in ["activeFocusOnTab: true", "Keys.onReturnPressed", "Keys.onSpacePressed", "Accessible.name: title", "delay: 500", "Qt.PointingHandCursor", "#1F888888"] {
            assert!(tile.contains(needle), "CoverTile.qml lacks {needle}");
        }
        // no fixed colour besides the placeholder: every `#` is the placeholder or the source-name suffix
        for file in [&qml, &tile] {
            let colours = file.matches('#').count() - file.matches("\" #\"").count();
            assert_eq!(colours, file.matches("#1F888888").count());
        }
    }

    #[test]
    fn a_cell_command_names_its_entry_in_the_deep_link_form() {
        use kuro_platform::deep_link::DeepLink;
        assert_eq!(DeepLink::parse("manga=12"), Some(DeepLink::Manga(12)));
        let node = std::process::Command::new("node").arg("--version").output();
        if node.is_err() {
            return; // no node here: the cell command is checked in the JS test only when node exists
        }
        let js = std::fs::read_to_string(dist("plasmoid/org.kurobayomi.updates.manga/contents/code/grid.js")).unwrap();
        let src: String = js.lines().filter(|l| !l.starts_with(".pragma")).collect::<Vec<_>>().join("\n");
        let script = format!(
            "const g = new Function({src:?} + '; return {{rowsCols, openCommand, feedCommand}}')();\
             const t = [[64,95,0,0,1,1],[128,190,0,0,2,2],[256,190,0,0,2,4],[256,190,24,24,1,4],[640,950,0,0,10,10],[2000,2000,0,0,10,10],[10,10,0,0,1,1],[256,100,10,0,1,4],[256,-5,0,0,1,4]];\
             for (const [w,h,pt,pb,r,c] of t) {{ const o = g.rowsCols(w,h,pt,pb); if (o.rows !== r || o.cols !== c) throw new Error(JSON.stringify([w,h,pt,pb,o])); }}\
             console.log(g.openCommand('manga', 12) + '|' + g.openCommand('anime', 7) + '|' + g.feedCommand('anime', 3, 5));"
        );
        let out = std::process::Command::new("node").arg("-e").arg(script).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let out = String::from_utf8(out.stdout).unwrap();
        assert_eq!(out.trim(), "kurobayomi --open manga=12|kurobayomi --open anime=7|kurobayomi --updates-json --kind anime --rows 3 --cols 5");
        // each cell's command differs by id
        assert_ne!(DeepLink::Manga(1).to_arg(), DeepLink::Manga(2).to_arg());
    }

    #[test]
    fn desktop_file_has_the_updates_action() {
        let text = std::fs::read_to_string(dist("kurobayomi.desktop")).unwrap();
        assert!(text.contains("Actions=Updates;"));
        assert!(text.contains("[Desktop Action Updates]\nName=Updates\nExec=kurobayomi --open updates\n"));
        assert_eq!(kuro_platform::deep_link::DeepLink::parse("updates"), Some(kuro_platform::deep_link::DeepLink::Updates));
        if let Ok(out) = std::process::Command::new("desktop-file-validate").arg(dist("kurobayomi.desktop")).output() {
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
        }
    }
}
