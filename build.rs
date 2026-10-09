//! Compiles the bundled icon theme (`src/theme/icons`) into a GResource under
//! `/io/github/blackbox/Kurobayomi/icons`; `theme::icons::register` loads it at startup.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

const PREFIX: &str = "/io/github/blackbox/Kurobayomi/icons";

fn files(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).expect("read icon dir").map(|e| e.expect("dir entry").path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "svg") {
            out.push(path.strip_prefix(root).expect("under root").to_string_lossy().into_owned());
        }
    }
}

fn main() {
    let root = PathBuf::from("src/theme/icons");
    println!("cargo:rerun-if-changed=src/theme/icons");
    println!("cargo:rerun-if-changed=build.rs");
    let mut list = Vec::new();
    files(&root, &root, &mut list);
    let mut xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n  <gresource prefix=\"{PREFIX}\">\n");
    for f in &list {
        let _ = writeln!(xml, "    <file>{f}</file>");
        println!("cargo:rerun-if-changed=src/theme/icons/{f}");
    }
    xml.push_str("  </gresource>\n</gresources>\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let xml_path = out.join("kurobayomi.gresource.xml");
    std::fs::write(&xml_path, xml).expect("write gresource xml");
    let status = Command::new("glib-compile-resources")
        .arg(format!("--sourcedir={}", root.display()))
        .arg(format!("--target={}", out.join("kurobayomi.gresource").display()))
        .arg(&xml_path)
        .status()
        .expect("glib-compile-resources must be installed (part of the GLib development tools)");
    assert!(status.success(), "glib-compile-resources failed");
}
