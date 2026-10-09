//! Importer tests and fixtures (plan 51-i18n T-51a-029, T-51a-009). The base files are copies of
//! the Aniyomi moko sources under `tests/fixtures/moko`, so the counts are checked against the
//! real input.

use fluent_bundle::FluentArgs;

use crate::catalog::Catalog;
use crate::import::{
    Body, LocaleBuild, Quantity, Source, generate_english, generate_locale, parse_moko,
};

const CORE_STRINGS: &str = include_str!("../tests/fixtures/moko/base/core-strings.xml");
const CORE_PLURALS: &str = include_str!("../tests/fixtures/moko/base/core-plurals.xml");
const ANIYOMI_STRINGS: &str = include_str!("../tests/fixtures/moko/base/aniyomi-strings.xml");
const ANIYOMI_PLURALS: &str = include_str!("../tests/fixtures/moko/base/aniyomi-plurals.xml");
const UR_STRINGS: &str = include_str!("../tests/fixtures/moko/ur/aniyomi-strings.xml");
const RU_STRINGS: &str = include_str!("../tests/fixtures/moko/ru/strings.xml");
const RU_PLURALS: &str = include_str!("../tests/fixtures/moko/ru/plurals.xml");
const ENGLISH_FTL: &str = include_str!("../locales/en/kurobayomi.ftl");

fn parse(xml: &str) -> Vec<Source> {
    parse_moko(xml).expect("fixture parses")
}

/// Core and Aniyomi base sources (strings and plurals concatenated per module).
fn base() -> (Vec<Source>, Vec<Source>) {
    let mut core = parse(CORE_STRINGS);
    core.extend(parse(CORE_PLURALS));
    let mut aniyomi = parse(ANIYOMI_STRINGS);
    aniyomi.extend(parse(ANIYOMI_PLURALS));
    (core, aniyomi)
}

fn ru_core() -> Vec<Source> {
    let mut out = parse(RU_STRINGS);
    out.extend(parse(RU_PLURALS));
    out
}

fn plain(name: &str, text: &str) -> Source {
    Source {
        name: name.to_string(),
        body: Body::Plain(text.to_string()),
        translatable: true,
    }
}

fn has_warning(build: &LocaleBuild, needle: &str) -> bool {
    build.warnings.iter().any(|w| w.contains(needle))
}

#[test]
fn parse_moko_counts_match_the_inventory() {
    // T-51a-029: 857 + 19 core, 570 + 15 Aniyomi, and 22 translatable="false" names.
    let count = |sources: &[Source], plain: bool| {
        sources
            .iter()
            .filter(|s| matches!(s.body, Body::Plain(_)) == plain)
            .count()
    };
    let core_strings = parse(CORE_STRINGS);
    let core_plurals = parse(CORE_PLURALS);
    let aniyomi_strings = parse(ANIYOMI_STRINGS);
    let aniyomi_plurals = parse(ANIYOMI_PLURALS);
    assert_eq!(count(&core_strings, true), 857);
    assert_eq!(count(&core_plurals, false), 19);
    assert_eq!(count(&aniyomi_strings, true), 570);
    assert_eq!(count(&aniyomi_plurals, false), 15);
    let non_translatable = [
        &core_strings,
        &core_plurals,
        &aniyomi_strings,
        &aniyomi_plurals,
    ]
    .iter()
    .flat_map(|list| list.iter())
    .filter(|s| !s.translatable)
    .count();
    assert_eq!(non_translatable, 22);
}

#[test]
fn english_generation_counts_1444_messages() {
    let (core, aniyomi) = base();
    let text = generate_english(&core, &aniyomi).expect("generates");
    let ids = text
        .lines()
        .filter(|l| l.contains(" =") && !l.starts_with('#') && !l.starts_with(' '))
        .count();
    assert_eq!(ids, 1444, "1434 from the Aniyomi base plus the 10 in kurobayomi-extra.ftl");
    assert_eq!(
        text, ENGLISH_FTL,
        "the committed English file is the generator output"
    );
}

#[test]
fn placeholder_mismatch_rejects_only_that_message() {
    // T-51a-009 (E13): onboarding_storage_info drops %2$s, so English is used for it alone.
    let (core, aniyomi) = base();
    let build = generate_locale(&core, &aniyomi, &ru_core(), &[]).expect("builds");
    assert!(
        has_warning(&build, "onboarding_storage_info"),
        "{:?}",
        build.warnings
    );
    assert!(!build.ftl.contains("onboarding_storage_info ="));
    // The other messages in the same file are kept.
    assert!(build.ftl.contains("pref_appearance_summary ="));
}

#[test]
fn reordered_placeholders_are_accepted() {
    // T-51a-009 (E14): pref_relative_format_summary swaps %1$s and %2$s.
    let (core, aniyomi) = base();
    let build = generate_locale(&core, &aniyomi, &ru_core(), &[]).expect("builds");
    assert!(
        build.ftl.contains("pref_relative_format_summary ="),
        "{:?}",
        build.warnings
    );
    assert!(!has_warning(&build, "pref_relative_format_summary"));
}

#[test]
fn stale_key_is_dropped_with_a_warning() {
    // T-51a-029 (E9): zz_not_in_base is not in the English base.
    let (core, aniyomi) = base();
    let build = generate_locale(&core, &aniyomi, &ru_core(), &[]).expect("builds");
    assert!(has_warning(&build, "zz_not_in_base"));
    assert!(!build.ftl.contains("zz_not_in_base"));
}

#[test]
fn empty_locale_produces_no_messages() {
    // T-51a-029: `ur` in Aniyomi is an empty resources file, so nothing is written for it.
    let (core, aniyomi) = base();
    let ur = parse(UR_STRINGS);
    assert!(ur.is_empty());
    let build = generate_locale(&core, &aniyomi, &[], &ur).expect("builds");
    assert_eq!(build.accepted, 0);
    assert!(build.warnings.is_empty());
}

#[test]
fn non_translatable_name_ignores_its_translation() {
    // T-51a-009 (E1): app_name is translatable="false" in the base.
    let (core, aniyomi) = base();
    let build = generate_locale(&core, &aniyomi, &ru_core(), &[]).expect("builds");
    assert!(has_warning(&build, "translatable"));
    assert!(!build.ftl.contains("app_name ="));
}

#[test]
fn aniyomi_translation_wins_for_overlap_names() {
    // pref_behavior is defined by both modules. The core translation must never be used.
    let (core, aniyomi) = base();
    let core_tr = vec![plain("pref_behavior", "Глава")];
    let aniyomi_tr = vec![plain("pref_behavior", "Поведение")];
    let build = generate_locale(&core, &aniyomi, &core_tr, &aniyomi_tr).expect("builds");
    assert!(
        build.ftl.contains("pref_behavior = Поведение"),
        "{}",
        build.ftl
    );
    assert!(!build.ftl.contains("Глава"));

    let core_only = generate_locale(&core, &aniyomi, &core_tr, &[]).expect("builds");
    assert!(!core_only.ftl.contains("pref_behavior ="));
}

#[test]
fn plural_without_other_falls_back_to_english() {
    // T-51a-009 (E12): a plural with no `other` variant is left out.
    let (core, aniyomi) = base();
    let no_other = Source {
        name: "relative_time".into(),
        body: Body::Plural(vec![(Quantity::One, "Вчера".into())]),
        translatable: true,
    };
    let build = generate_locale(&core, &aniyomi, &[no_other], &[]).expect("builds");
    assert!(has_warning(&build, "other"), "{:?}", build.warnings);
    assert!(!build.ftl.contains("relative_time ="));
}

#[test]
fn unsupported_specifier_rejects_only_that_message() {
    // T-51a-009 (E15): %-5s is not a supported Java specifier.
    let (core, aniyomi) = base();
    let bad = plain("pref_appearance_summary", "Тема %-5s");
    let build = generate_locale(&core, &aniyomi, &[bad], &[]).expect("builds");
    assert!(has_warning(&build, "pref_appearance_summary"));
    assert!(!build.ftl.contains("pref_appearance_summary ="));
}

#[test]
fn generated_locale_file_parses() {
    // T-51a-029 (converter validates): every generated locale parses with fluent_syntax.
    let (core, aniyomi) = base();
    let build = generate_locale(&core, &aniyomi, &ru_core(), &[]).expect("builds");
    assert!(build.accepted > 0);
    assert!(fluent_syntax::parser::parse(build.ftl.as_str()).is_ok());
}

#[test]
fn russian_plural_categories_select_few_and_many() {
    // Catalog-level check of the plural forms this locale translates (CLDR rules for ru).
    let (core, aniyomi) = base();
    let build = generate_locale(&core, &aniyomi, &ru_core(), &[]).expect("builds");
    let catalog = Catalog::build(&["ru"], &[("ru", build.ftl.as_str()), ("en", ENGLISH_FTL)]);
    let render = |count: i64| {
        let mut args = FluentArgs::new();
        args.set("count", count);
        catalog
            .format("relative_time", Some(&args))
            .expect("message exists")
            .0
    };
    assert_eq!(render(1), "Вчера");
    assert_eq!(render(2), "2 дня назад");
    assert_eq!(render(5), "5 дней назад");
    assert_eq!(render(11), "11 дней назад");
    assert_eq!(render(21), "Вчера");
    assert_eq!(render(22), "22 дня назад");
    assert_eq!(render(25), "25 дней назад");
}

#[test]
fn overlap_precedence() {
    // T-51a-018 (test 22): each of the 27 names both modules define has the Aniyomi text in the
    // English file, converted, and the core copy is not emitted as a second definition.
    let (core_all, _) = base();
    let aniyomi_all = {
        let mut all = parse(ANIYOMI_STRINGS);
        all.extend(parse(ANIYOMI_PLURALS));
        all
    };
    let mut overlaps = 0;
    for source in &aniyomi_all {
        let Some(core_source) = core_all.iter().find(|c| c.name == source.name) else {
            continue;
        };
        assert_ne!(core_source.body, source.body, "{}", source.name);
        overlaps += 1;
        let id = source.name.replace('.', "_");
        let prefix = format!("{id} =");
        let defined = ENGLISH_FTL
            .lines()
            .filter(|l| l.starts_with(&prefix))
            .count();
        assert_eq!(defined, 1, "{id} must be defined once in the English file");
        match &source.body {
            Body::Plain(text) => {
                let converted = crate::import::convert_pattern(text, None).expect("converts");
                let line = format!("{id} = {converted}");
                assert!(ENGLISH_FTL.lines().any(|l| l == line), "{line}");
            }
            Body::Plural(_) => {}
        }
    }
    assert_eq!(overlaps, 27);
}
