//! Regenerates `locales/en/kurobayomi.ftl` from the Aniyomi moko base XML (plan 51-i18n T-51a-001).
//!
//! Usage: kuro-i18n-import <aniyomi-src-dir> <output-file>

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use kuro_i18n::import::{Source, generate_english, parse_moko};

fn read_base(root: &Path, module: &str, file: &str) -> Result<Vec<Source>, String> {
    let path = root
        .join(module)
        .join("src/commonMain/moko-resources/base")
        .join(file);
    let xml = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_moko(&xml).map_err(|e| format!("{}: {e}", path.display()))
}

fn run(root: &Path, out: &Path) -> Result<usize, String> {
    let mut core = read_base(root, "i18n", "strings.xml")?;
    core.extend(read_base(root, "i18n", "plurals.xml")?);
    let mut aniyomi = read_base(root, "i18n-aniyomi", "strings.xml")?;
    aniyomi.extend(read_base(root, "i18n-aniyomi", "plurals.xml")?);
    let text = generate_english(&core, &aniyomi).map_err(|e| e.to_string())?;
    fs::write(out, &text).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(core.len() + aniyomi.len())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: kuro-i18n-import <aniyomi-src-dir> <output-file>");
        return ExitCode::from(2);
    }
    match run(&PathBuf::from(&args[1]), &PathBuf::from(&args[2])) {
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
