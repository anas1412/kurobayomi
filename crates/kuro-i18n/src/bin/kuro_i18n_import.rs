//! Regenerates `locales/en/kurobayomi.ftl` from the Aniyomi moko base XML (plan 51-i18n T-51a-001).
//!
//! Usage: kuro-i18n-import <aniyomi-src-dir> <output-file>
//!        kuro-i18n-import locales <aniyomi-src-dir> <locales-dir>
//!        kuro-i18n-import check <aniyomi-src-dir>
//!
//! `check` runs both generations in memory, writes nothing and exits non-zero when the source cannot be read or the
//! English output is not valid Fluent. A translated message that breaks a T-51a-009 rule falls back to English with a
//! warning and does not fail the command (T-51a-024). CI does not run it yet.
//!
//! The second form writes `<locales-dir>/<tag>/kurobayomi.ftl` for every translated locale
//! (T-51a-008). A locale with no accepted message (`ur`) gets no file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use kuro_i18n::import::{Source, generate_english, generate_locale, parse_moko};
use kuro_i18n::locale::android_dir_to_tag;

fn read_base(root: &Path, module: &str, file: &str) -> Result<Vec<Source>, String> {
    let path = root
        .join(module)
        .join("src/commonMain/moko-resources/base")
        .join(file);
    let xml = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_moko(&xml).map_err(|e| format!("{}: {e}", path.display()))
}

fn run(root: &Path, out: Option<&Path>) -> Result<usize, String> {
    let mut core = read_base(root, "i18n", "strings.xml")?;
    core.extend(read_base(root, "i18n", "plurals.xml")?);
    let mut aniyomi = read_base(root, "i18n-aniyomi", "strings.xml")?;
    aniyomi.extend(read_base(root, "i18n-aniyomi", "plurals.xml")?);
    let text = generate_english(&core, &aniyomi).map_err(|e| e.to_string())?;
    if let Some(out) = out {
        fs::write(out, &text).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(core.len() + aniyomi.len())
}

fn read_dir_sources(dir: &Path, files: &[&str]) -> Result<Vec<Source>, String> {
    let mut out = Vec::new();
    for file in files {
        let path = dir.join(file);
        if !path.is_file() {
            continue;
        }
        let xml = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        out.extend(parse_moko(&xml).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    Ok(out)
}

fn run_locales(root: &Path, out: Option<&Path>) -> Result<usize, String> {
    let mut core_base = read_base(root, "i18n", "strings.xml")?;
    core_base.extend(read_base(root, "i18n", "plurals.xml")?);
    let mut aniyomi_base = read_base(root, "i18n-aniyomi", "strings.xml")?;
    aniyomi_base.extend(read_base(root, "i18n-aniyomi", "plurals.xml")?);
    let res = |module: &str| root.join(module).join("src/commonMain/moko-resources");
    let mut dirs: Vec<String> = Vec::new();
    for module in ["i18n", "i18n-aniyomi"] {
        let entries = fs::read_dir(res(module)).map_err(|e| format!("{module}: {e}"))?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name != "base" && !dirs.contains(&name) {
                dirs.push(name);
            }
        }
    }
    dirs.sort();
    let files = ["strings.xml", "plurals.xml"];
    let mut written = 0;
    for dir in dirs {
        let core_tr = read_dir_sources(&res("i18n").join(&dir), &files)?;
        let aniyomi_tr = read_dir_sources(&res("i18n-aniyomi").join(&dir), &files)?;
        let build = generate_locale(&core_base, &aniyomi_base, &core_tr, &aniyomi_tr)
            .map_err(|e| format!("{dir}: {e}"))?;
        let tag = android_dir_to_tag(&dir);
        println!(
            "{tag}: {} messages, {} warnings",
            build.accepted,
            build.warnings.len()
        );
        if build.accepted == 0 {
            continue;
        }
        if let Some(out) = out {
            let target = out.join(&tag);
            fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
            let file = target.join("kurobayomi.ftl");
            fs::write(&file, &build.ftl).map_err(|e| format!("{}: {e}", file.display()))?;
        }
        written += 1;
    }
    Ok(written)
}

/// `check`: generates English and every locale in memory. Returns the number of locales that have messages.
fn check(root: &Path) -> Result<usize, String> {
    run(root, None)?;
    run_locales(root, None)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "check" {
        return match check(&PathBuf::from(&args[2])) {
            Ok(n) => {
                println!("ok: English and {n} locales validate");
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("kuro-i18n-import: {message}");
                ExitCode::FAILURE
            }
        };
    }
    if args.len() == 4 && args[1] == "locales" {
        return match run_locales(&PathBuf::from(&args[2]), Some(&PathBuf::from(&args[3]))) {
            Ok(n) => {
                println!("wrote {n} locale files to {}", args[3]);
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("kuro-i18n-import: {message}");
                ExitCode::FAILURE
            }
        };
    }
    if args.len() != 3 {
        eprintln!("usage: kuro-i18n-import <aniyomi-src-dir> <output-file> | locales <aniyomi-src-dir> <locales-dir> | check <aniyomi-src-dir>");
        return ExitCode::from(2);
    }
    match run(&PathBuf::from(&args[1]), Some(&PathBuf::from(&args[2]))) {
        Ok(sources) => {
            println!("read {sources} source entries, wrote {}", args[2]);
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("kuro-i18n-import: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE_STRINGS: &str = include_str!("../../tests/fixtures/moko/base/core-strings.xml");
    const CORE_PLURALS: &str = include_str!("../../tests/fixtures/moko/base/core-plurals.xml");
    const ANIYOMI_STRINGS: &str = include_str!("../../tests/fixtures/moko/base/aniyomi-strings.xml");
    const ANIYOMI_PLURALS: &str = include_str!("../../tests/fixtures/moko/base/aniyomi-plurals.xml");
    const RU_STRINGS: &str = include_str!("../../tests/fixtures/moko/ru/strings.xml");
    const RU_PLURALS: &str = include_str!("../../tests/fixtures/moko/ru/plurals.xml");

    /// An Aniyomi source tree holding the importer fixtures: the base files and a Russian translation.
    fn tree() -> std::io::Result<tempfile::TempDir> {
        let root = tempfile::tempdir()?;
        for (module, dir, strings, plurals) in [
            ("i18n", "base", CORE_STRINGS, CORE_PLURALS),
            ("i18n-aniyomi", "base", ANIYOMI_STRINGS, ANIYOMI_PLURALS),
            ("i18n", "ru", RU_STRINGS, RU_PLURALS),
        ] {
            let folder = root.path().join(module).join("src/commonMain/moko-resources").join(dir);
            fs::create_dir_all(&folder)?;
            fs::write(folder.join("strings.xml"), strings)?;
            fs::write(folder.join("plurals.xml"), plurals)?;
        }
        Ok(root)
    }

    #[test]
    fn check_passes_on_the_fixture_tree_and_writes_nothing() -> std::io::Result<()> {
        let root = tree()?;
        let before = fs::read_dir(root.path())?.count();
        let locales = check(root.path()).map_err(std::io::Error::other)?;
        assert!(locales >= 1, "the Russian translation is generated");
        assert_eq!(fs::read_dir(root.path())?.count(), before, "nothing is written");
        Ok(())
    }

    #[test]
    fn check_fails_on_a_duplicate_name_in_a_base_file() -> std::io::Result<()> {
        let root = tree()?;
        let broken = root.path().join("i18n/src/commonMain/moko-resources/base/strings.xml");
        // a name defined twice in one module is an error (T-51a-002)
        fs::write(broken, "<resources><string name=\"a\">x</string><string name=\"a\">y</string></resources>")?;
        assert!(check(root.path()).is_err());
        Ok(())
    }

    #[test]
    fn check_fails_when_the_source_tree_is_missing() {
        assert!(check(Path::new("/no/such/aniyomi")).is_err());
    }
}
