//! A panic on the process main thread (the GTK main thread in the app) writes exactly one report
//! and exits 101. Libtest runs every test on a named worker thread, so this target has its own
//! `main`, and the panic runs on the real main thread.

use std::path::PathBuf;
use std::process::{Command, Stdio};

const CHILD_DIR: &str = "KURO_MAIN_CRASH_DIR";

fn main() {
    // the detached reporter that the hook starts runs this binary with `--crash-report`
    if std::env::args().any(|a| a == "--crash-report") {
        return;
    }
    if let Ok(dir) = std::env::var(CHILD_DIR) {
        kuro_core::crash::install(PathBuf::from(dir));
        panic!("boom on the main thread");
    }

    let tmp = tempfile::tempdir().unwrap();
    let crashes = tmp.path().join("crashes");
    let status = Command::new(std::env::current_exe().unwrap())
        .env(CHILD_DIR, &crashes)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(101), "exit code");

    let reports: Vec<PathBuf> = std::fs::read_dir(&crashes)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    assert_eq!(reports.len(), 1, "{reports:?}");
    let text = std::fs::read_to_string(&reports[0]).unwrap();
    assert!(text.lines().any(|l| l == "thread: main"), "{text}");
    assert!(text.contains("message: boom on the main thread\n"), "{text}");
    // file and line numbers survive in release builds: `debug = "line-tables-only"` and no strip (T-01a-040).
    // CI builds with debug symbols off to fit the runner disk (CARGO_PROFILE_DEV_DEBUG=0), so the check only applies to builds that keep them.
    let no_line_tables = std::env::var("CARGO_PROFILE_DEV_DEBUG").is_ok_and(|v| v == "0" || v == "none");
    if !no_line_tables {
        assert!(
            text.lines().any(|l| l.trim_start().starts_with("at ") && l.contains("crash_main_thread.rs:")),
            "the backtrace has no file and line for this test: {text}"
        );
    }
    println!("ok: a main-thread panic wrote one report and exited 101");
}
