//! Cross-checks `schemes.rs` against the Aniyomi colour XML checked in under `fixtures/`
//! (T-49b-002, p2-edge-cases 4.1). Test-only: the fixtures are not read at runtime.

use std::collections::BTreeMap;

use super::schemes::SCHEMES;
use super::tokens::{Rgba, Role, ROLE_COUNT};

/// XML key for each role, in `Role` index order (the generator's `ROLES`).
const KEYS: [&str; ROLE_COUNT] = [
    "primary", "onPrimary", "primaryContainer", "onPrimaryContainer", "inversePrimary",
    "secondary", "onSecondary", "secondaryContainer", "onSecondaryContainer",
    "tertiary", "onTertiary", "tertiaryContainer", "onTertiaryContainer",
    "background", "onBackground", "surface", "onSurface", "surfaceVariant",
    "onSurfaceVariant", "surfaceTint", "inverseSurface", "inverseOnSurface",
    "error", "onError", "errorContainer", "onErrorContainer",
    "outline", "outlineVariant", "scrim", "surfaceBright", "surfaceDim",
    "surfaceContainer", "surfaceContainerHigh", "surfaceContainerHighest",
    "surfaceContainerLow", "surfaceContainerLowest",
    "primaryInverse", "elevationOverlay",
];

/// `SCHEMES` name and fixture file, in `SCHEMES` order.
const FILES: [(&str, &str); 18] = [
    ("Default", "colors_tachiyomi.xml"),
    ("Monochrome", "colors_monochrome.xml"),
    ("Yotsuba", "colors_yotsuba.xml"),
    ("Tako", "colors_tako.xml"),
    ("Cloudflare", "color_cloudflare.xml"),
    ("Doom", "color_doom.xml"),
    ("Lavender", "color_lavender.xml"),
    ("Matrix", "color_matrix.xml"),
    ("Sapphire", "color_sapphire.xml"),
    ("GreenApple", "colors_greenapple.xml"),
    ("MidnightDusk", "colors_midnightdusk.xml"),
    ("Nord", "colors_nord.xml"),
    ("StrawberryDaiquiri", "colors_strawberry.xml"),
    ("TealTurquoise", "colors_tealturqoise.xml"),
    ("TidalWave", "colors_tidalwave.xml"),
    ("YinYang", "colors_yinyang.xml"),
    ("CottonCandy", "colors_cottoncandy.xml"),
    ("Mocha", "colors_mocha.xml"),
];

/// Roles present per scheme, `(light, dark)`, counting the two non-M3 keys when they are declared:
/// `p2-data-and-config.md` Appendix B. Fails when a table is edited without this list.
fn present_count(name: &str) -> (usize, usize) {
    match name {
        "Default" => (33, 33),
        "Monochrome" | "Lavender" | "GreenApple" | "StrawberryDaiquiri" => (35, 35),
        "Yotsuba" | "Tako" | "MidnightDusk" | "TealTurquoise" | "TidalWave" | "YinYang" => (28, 28),
        "Cloudflare" | "Doom" | "Matrix" | "Sapphire" => (23, 23),
        "Nord" => (31, 32),
        "CottonCandy" => (23, 22),
        "Mocha" => (22, 22),
        other => panic!("no coverage row for {other}"),
    }
}

/// Parsed `<color>` values by role key, with `@color/` references resolved inside the file.
/// Each value is the raw literal text (`#RRGGBB` or `#AARRGGBB`).
fn parse_xml(text: &str) -> BTreeMap<String, String> {
    let mut raw: BTreeMap<String, String> = BTreeMap::new();
    let mut rest = text;
    while let Some(start) = rest.find("<color name=\"") {
        rest = &rest[start + "<color name=\"".len()..];
        let (name, after) = rest.split_once("\">").expect("unterminated color tag");
        let (value, after) = after.split_once("</color>").expect("unterminated color");
        let key = name.split_once('_').map_or(name, |(_, k)| k);
        raw.insert(key.to_string(), value.trim().to_string());
        rest = after;
    }
    let mut out = BTreeMap::new();
    for (key, value) in &raw {
        let resolved = match value.strip_prefix("@color/") {
            Some(reference) => {
                let target = reference.split_once('_').map_or(reference, |(_, k)| k);
                raw.get(target).cloned().unwrap_or_default()
            }
            None => value.clone(),
        };
        out.insert(key.clone(), resolved);
    }
    out
}

fn fixture(name: &str, dark: bool) -> BTreeMap<String, String> {
    let file = FILES.iter().find(|(n, _)| *n == name).expect("fixture file").1;
    let dir = if dark { "values-night" } else { "values" };
    let path = format!("{}/src/theme/fixtures/{dir}/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    parse_xml(&text)
}

#[test]
fn fixtures_cover_all_eighteen_schemes_in_order() {
    assert_eq!(SCHEMES.len(), FILES.len());
    for (scheme, (name, _)) in SCHEMES.iter().zip(FILES) {
        assert_eq!(scheme.name, name);
    }
}

#[test]
fn every_declared_xml_value_equals_the_scheme_value() {
    for scheme in SCHEMES.iter() {
        for dark in [false, true] {
            let xml = fixture(scheme.name, dark);
            let table = if dark { &scheme.dark } else { &scheme.light };
            for (i, key) in KEYS.iter().enumerate() {
                match xml.get(*key) {
                    Some(literal) => {
                        let want = Rgba::parse_hex(literal)
                            .unwrap_or_else(|| panic!("{} {key}: bad literal {literal}", scheme.name));
                        assert_eq!(table[i], Some(want), "{} dark={dark} {key}", scheme.name);
                    }
                    None => assert_eq!(
                        table[i], None,
                        "{} dark={dark} {key} is not in the XML",
                        scheme.name
                    ),
                }
            }
        }
    }
}

#[test]
fn role_set_per_scheme_matches_the_coverage_table() {
    for scheme in SCHEMES.iter() {
        let (light, dark) = present_count(scheme.name);
        for (is_dark, want) in [(false, light), (true, dark)] {
            let table = if is_dark { &scheme.dark } else { &scheme.light };
            let present = table.iter().filter(|v| v.is_some()).count();
            assert_eq!(present, want, "{} dark={is_dark}", scheme.name);
        }
    }
}

#[test]
fn xml_keys_are_all_known_roles() {
    for (name, _) in FILES {
        for dark in [false, true] {
            for key in fixture(name, dark).keys() {
                assert!(KEYS.contains(&key.as_str()), "{name} dark={dark}: unknown key {key}");
            }
        }
    }
}

#[test]
fn eight_digit_hex_only_where_listed() {
    let listed = ["Cloudflare", "Doom", "Matrix", "Sapphire"];
    for (name, _) in FILES {
        for dark in [false, true] {
            for (key, literal) in fixture(name, dark) {
                let digits = literal.strip_prefix('#').unwrap_or_else(|| panic!("{name} {key}: {literal}"));
                assert!(digits.bytes().all(|c| c.is_ascii_hexdigit()), "{name} {key}: {literal}");
                match digits.len() {
                    6 => {}
                    8 => {
                        assert!(listed.contains(&name) && key == "onSurfaceVariant", "{name} {key}: {literal}");
                        let want = if dark { "D8FFFFFF" } else { "D849454E" };
                        assert_eq!(digits.to_ascii_uppercase(), want, "{name} dark={dark}");
                    }
                    n => panic!("{name} {key}: {n} digits"),
                }
            }
        }
    }
}

#[test]
fn listed_schemes_do_carry_the_eight_digit_value() {
    for name in ["Cloudflare", "Doom", "Matrix", "Sapphire"] {
        for (dark, want) in [(false, 0xD849_454E_u32), (true, 0xD8FF_FFFF)] {
            let scheme = SCHEMES.iter().find(|s| s.name == name).unwrap();
            let table = if dark { &scheme.dark } else { &scheme.light };
            assert_eq!(table[Role::OnSurfaceVariant as usize], Some(Rgba::argb(want)), "{name}");
        }
    }
}
