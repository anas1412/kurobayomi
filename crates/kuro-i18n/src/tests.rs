//! Runtime catalogue tests: lookup and fallback, missing-key policy, argument checks, isolation
//! marks, the one-time init, and the generated key table.

use std::collections::HashSet;
use std::sync::OnceLock;

use fluent_bundle::{FluentArgs, FluentValue};

use crate::catalog::Catalog;
use crate::locale::LocaleInfo;
use crate::{Arg, MSG_COUNT, MsgKey, catalog, install, tr, tr_plural, trf, warn_once};

const ENGLISH_FTL: &str = include_str!("../locales/en/kurobayomi.ftl");

#[test]
fn english_is_the_default_locale() {
    assert_eq!(tr(MsgKey::pref_fullscreen), "Fullscreen");
    assert_eq!(crate::current_locale().tag, "en");
}

#[test]
fn plural_and_argument_messages_render_english() {
    assert_eq!(tr_plural(MsgKey::relative_time, 1, &[]), "Yesterday");
    assert_eq!(tr_plural(MsgKey::relative_time, 2, &[]), "2 days ago");
    // T-51a-003: restore_duration renders 03 and 07.
    assert_eq!(
        trf(
            MsgKey::restore_duration,
            &[("arg1", Arg::Int(3)), ("arg2", Arg::Int(7))]
        ),
        "03 min, 07 sec"
    );
    // T-51a-003: player_speed renders 1.50x.
    assert_eq!(
        trf(MsgKey::player_speed, &[("arg1", Arg::Dec(1.5))]),
        "1.50x"
    );
    // Plural with two arguments: the count is also the second number.
    let chapters = tr_plural(
        MsgKey::notification_chapters_multiple_and_more,
        3,
        &[("arg1", Arg::Str("A, B"))],
    );
    assert_eq!(chapters, "Chapters A, B and 3 more");
    assert_eq!(
        tr_plural(
            MsgKey::notification_chapters_multiple_and_more,
            1,
            &[("arg1", Arg::Str("A"))]
        ),
        "Chapters A and 1 more"
    );
}

#[test]
fn restore_completed_message_shows_time_and_error_count() {
    // BackupNotifier passes (errorCount, timeString, errorCount): %1$s is the time, %2$s the count.
    let args = [("arg1", Arg::Str("1m 2s"))];
    assert_eq!(
        tr_plural(MsgKey::restore_completed_message, 1, &args),
        "Done in 1m 2s with 1 error"
    );
    assert_eq!(
        tr_plural(MsgKey::restore_completed_message, 3, &args),
        "Done in 1m 2s with 3 errors"
    );
}

#[test]
fn fallback_walks_the_chain_then_english() {
    let sources: &[(&str, &str)] = &[("fr", "a = Bonjour\n"), ("en", "a = Hello\nb = World\n")];
    let catalog = Catalog::build(&["fr"], sources);
    assert_eq!(catalog.format("a", None).unwrap().0, "Bonjour");
    assert_eq!(catalog.format("b", None).unwrap().0, "World");
    assert_eq!(catalog.info(), LocaleInfo::new("fr"));
}

#[test]
fn message_missing_everywhere_is_none_and_warns_once() {
    let catalog = Catalog::build(&["en"], &[("en", "a = Hello\n")]);
    assert!(catalog.format("zz_not_there", None).is_none());
    assert!(warn_once("zz_warn_once_probe"));
    assert!(!warn_once("zz_warn_once_probe"));
}

#[test]
fn missing_argument_renders_placeholder_and_reports_error() {
    // F8: the placeholder text stays visible and Fluent reports the error.
    let catalog = Catalog::build(&["en"], &[("en", "k = Hi { $arg1 }\n")]);
    let (text, errors) = catalog.format("k", None).unwrap();
    assert!(text.contains("{$arg1}"), "{text}");
    assert!(!errors.is_empty());
}

#[test]
fn extra_argument_is_ignored_without_error() {
    // F9: an extra argument produces no error.
    let catalog = Catalog::build(&["en"], &[("en", "k = Hi { $arg1 }\n")]);
    let mut args = FluentArgs::new();
    args.set("arg1", "x");
    args.set("arg2", "y");
    let (text, errors) = catalog.format("k", Some(&args)).unwrap();
    assert_eq!(text, "Hi x");
    assert!(errors.is_empty());
}

#[test]
fn every_key_renders_with_its_declared_arguments() {
    // T-51a-006: each message, called with its declared arguments, has no Fluent error and
    // no isolation marks (X6). Plural messages also get the count.
    let catalog = catalog();
    for key in MsgKey::ALL {
        let mut args = FluentArgs::new();
        for name in key.args() {
            args.set(*name, FluentValue::from(7_i64));
        }
        let (text, errors) = catalog
            .format(key.id(), Some(&args))
            .expect("every key exists in English");
        assert!(errors.is_empty(), "{}: {errors:?}", key.id());
        assert!(
            !text.contains('\u{2068}') && !text.contains('\u{2069}'),
            "{}: {text:?}",
            key.id()
        );
    }
}

#[test]
fn key_table_matches_english_file() {
    assert_eq!(MsgKey::ALL.len(), MSG_COUNT);
    assert_eq!(MSG_COUNT, 1434);
    let ids: HashSet<&str> = MsgKey::ALL.iter().map(|k| k.id()).collect();
    assert_eq!(ids.len(), MSG_COUNT, "ids are unique");
    assert_eq!(MsgKey::r#type.id(), "type");
    assert_eq!(
        MsgKey::extensionStoresScreen_addStore_title.id(),
        "extensionStoresScreen_addStore_title"
    );
    // Each English id is in the file, and the file has no extra messages.
    let resource = fluent_syntax::parser::parse(ENGLISH_FTL).expect("English file parses");
    let file_ids: HashSet<String> = resource
        .body
        .iter()
        .filter_map(|e| match e {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(file_ids.len(), MSG_COUNT);
    assert!(file_ids.iter().all(|id| ids.contains(id.as_str())));
}

#[test]
fn english_file_has_no_android_escapes_or_entities() {
    assert!(!ENGLISH_FTL.contains("\\'"));
    assert!(!ENGLISH_FTL.contains("\\\""));
    assert!(!ENGLISH_FTL.contains("&amp;"));
}

#[test]
fn every_plural_has_an_other_variant() {
    let selects = ENGLISH_FTL.matches("{ $count ->").count();
    let others = ENGLISH_FTL.matches("*[other]").count();
    assert_eq!(selects, 34);
    assert_eq!(others, 34);
}

#[test]
fn all_22_non_translatable_names_are_present() {
    let names = [
        "app_name",
        "action_web_view",
        "ext_installer_packageinstaller",
        "ext_installer_shizuku",
        "ext_installer_private",
        "pref_progress_70",
        "pref_progress_75",
        "pref_progress_80",
        "pref_progress_85",
        "pref_progress_90",
        "pref_progress_95",
        "pref_progress_100",
        "pref_skip_30",
        "pref_skip_20",
        "pref_skip_10",
        "pref_skip_5",
        "pref_skip_3",
        "player_speed",
        "player_sheets_track_title_w_lang",
        "player_sheets_track_title_wo_lang",
        "player_sheets_track_lang_wo_title",
        "player_gesture_seek_indicator",
    ];
    assert_eq!(names.len(), 22);
    let ids: HashSet<&str> = MsgKey::ALL.iter().map(|k| k.id()).collect();
    for name in names {
        assert!(ids.contains(name), "{name}");
    }
}

#[test]
fn init_is_applied_once_and_later_calls_are_ignored() {
    let slot: OnceLock<Catalog> = OnceLock::new();
    let sources: &[(&str, &str)] = &[("en", "a = Hello\n"), ("de", "a = Hallo\n")];
    assert!(install(&slot, &["de"], sources));
    assert!(!install(&slot, &["en"], sources));
    assert_eq!(slot.get().unwrap().format("a", None).unwrap().0, "Hallo");
}

#[test]
fn concurrent_init_sets_exactly_one_catalogue() {
    let slot: OnceLock<Catalog> = OnceLock::new();
    let sources: &[(&str, &str)] = &[("en", "a = Hello\n")];
    let wins: usize = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| install(&slot, &["en"], sources)))
            .collect();
        handles
            .into_iter()
            .map(|h| usize::from(h.join().expect("thread")))
            .sum()
    });
    assert_eq!(wins, 1);
}

#[test]
fn bare_percent_label_renders_literally() {
    assert_eq!(tr(MsgKey::pref_progress_70), "70%");
}
