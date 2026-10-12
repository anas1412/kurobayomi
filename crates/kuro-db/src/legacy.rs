//! The Aniyomi manga migrations 1..32, embedded for the legacy importer (T-03a-022). `sql/legacy/N.sql` upgrades a
//! database whose `PRAGMA user_version` is `N` to `N+1`. The files are generated from the `.sqm` text by
//! [`strip_annotations`] and committed; a test regenerates them when the Aniyomi source is at hand (TC-I01).
//! Fresh databases never run them: the baseline is created directly.

use rusqlite::Connection;

/// The oldest `user_version` the importer accepts (the state `14.sqm` upgrades).
pub const OLDEST_IMPORTABLE: i64 = 14;
/// The newest `user_version` the importer accepts: the version Aniyomi's manga database ends at.
pub const NEWEST_IMPORTABLE: i64 = 33;

/// `sql/legacy/1.sql` .. `32.sql`; index `n - 1` upgrades version `n` to `n + 1`.
pub const MIGRATIONS: [&str; 32] = [
    include_str!("../sql/legacy/1.sql"),
    include_str!("../sql/legacy/2.sql"),
    include_str!("../sql/legacy/3.sql"),
    include_str!("../sql/legacy/4.sql"),
    include_str!("../sql/legacy/5.sql"),
    include_str!("../sql/legacy/6.sql"),
    include_str!("../sql/legacy/7.sql"),
    include_str!("../sql/legacy/8.sql"),
    include_str!("../sql/legacy/9.sql"),
    include_str!("../sql/legacy/10.sql"),
    include_str!("../sql/legacy/11.sql"),
    include_str!("../sql/legacy/12.sql"),
    include_str!("../sql/legacy/13.sql"),
    include_str!("../sql/legacy/14.sql"),
    include_str!("../sql/legacy/15.sql"),
    include_str!("../sql/legacy/16.sql"),
    include_str!("../sql/legacy/17.sql"),
    include_str!("../sql/legacy/18.sql"),
    include_str!("../sql/legacy/19.sql"),
    include_str!("../sql/legacy/20.sql"),
    include_str!("../sql/legacy/21.sql"),
    include_str!("../sql/legacy/22.sql"),
    include_str!("../sql/legacy/23.sql"),
    include_str!("../sql/legacy/24.sql"),
    include_str!("../sql/legacy/25.sql"),
    include_str!("../sql/legacy/26.sql"),
    include_str!("../sql/legacy/27.sql"),
    include_str!("../sql/legacy/28.sql"),
    include_str!("../sql/legacy/29.sql"),
    include_str!("../sql/legacy/30.sql"),
    include_str!("../sql/legacy/31.sql"),
    include_str!("../sql/legacy/32.sql"),
];

/// The SQL of migration `n` (upgrading version `n` to `n + 1`), for `n` in 1..=32.
pub fn migration(n: i64) -> Option<&'static str> {
    usize::try_from(n - 1).ok().and_then(|i| MIGRATIONS.get(i)).copied()
}

/// Applies migrations `from..=32` in order, one `BEGIN IMMEDIATE` / `COMMIT` pair each, and stamps `user_version`
/// after each. The caller turns `foreign_keys` off first, as Android ran them. With `drop_history_temp` the table
/// that migration 15 leaves behind is dropped right after that step.
pub fn replay(c: &Connection, from: i64, drop_history_temp: bool) -> rusqlite::Result<()> {
    replay_with(c, from, drop_history_temp, &mut |_| {})
}

/// [`replay`] that calls `on_step(n)` before migration `n` runs.
pub fn replay_with(c: &Connection, from: i64, drop_history_temp: bool, on_step: &mut dyn FnMut(i64)) -> rusqlite::Result<()> {
    for n in from..=32 {
        let Some(sql) = migration(n) else { continue };
        on_step(n);
        c.execute_batch("BEGIN IMMEDIATE")?;
        let step = c.execute_batch(sql).and_then(|()| {
            if drop_history_temp && n == 15 {
                c.execute_batch("DROP TABLE IF EXISTS history_temp")?;
            }
            c.pragma_update(None, "user_version", n + 1)
        });
        match step {
            Ok(()) => c.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = c.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
    }
    Ok(())
}

/// The four stripping rules that turn an SQLDelight `.sqm` file into plain SQLite SQL:
/// 1. lines starting with `import ` are removed;
/// 2. the tokens `AS Boolean`, `AS Long`, `AS Float` and `AS Date` are deleted (regex
///    `\bAS (Boolean|Long|Float|Date)(?![A-Za-z])`; the spaces around them stay);
/// 3. `"System category can't be deleted"` becomes `'System category can''t be deleted'` and
///    `VALUES (0, "", -1, 0)` becomes `VALUES (0, '', -1, 0)`;
/// 4. everything else, comments included, is kept byte for byte.
pub fn strip_annotations(sqm: &str) -> String {
    let kept: String = sqm.split_inclusive('\n').filter(|l| !l.starts_with("import ")).collect();
    let mut out = String::with_capacity(kept.len());
    let mut rest = kept.as_str();
    let mut prev: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        if c == 'A' && !prev.is_some_and(is_word_char) {
            if let Some(len) = annotation_len(rest) {
                prev = rest[..len].chars().last();
                rest = &rest[len..];
                continue;
            }
        }
        out.push(c);
        prev = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    out.replace("\"System category can't be deleted\"", "'System category can''t be deleted'")
        .replace("VALUES (0, \"\", -1, 0)", "VALUES (0, '', -1, 0)")
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Length of an `AS Boolean|Long|Float|Date` token at the start of `text`, when one is there and no letter follows.
fn annotation_len(text: &str) -> Option<usize> {
    let after = text.strip_prefix("AS ")?;
    ["Boolean", "Long", "Float", "Date"].into_iter().find_map(|word| {
        let tail = after.strip_prefix(word)?;
        (!tail.starts_with(|c: char| c.is_ascii_alphabetic())).then(|| text.len() - tail.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stripping_rules_apply_to_a_sample() {
        let sqm = "import kotlin.Boolean;\n\nALTER TABLE t ADD COLUMN a INTEGER AS Boolean DEFAULT 0 NOT NULL;\nx REAL AS Float NOT NULL,\ny INTEGER AS Long;\nz AS Dates AS Date\n-- keep AS Long in names: ASLong\nINSERT INTO c VALUES (0, \"\", -1, 0);\nRAISE(ABORT, \"System category can't be deleted\")";
        let out = strip_annotations(sqm);
        assert!(!out.contains("import "));
        assert!(out.contains("a INTEGER  DEFAULT 0 NOT NULL;"), "the token goes, the spaces stay: {out}");
        assert!(out.contains("x REAL  NOT NULL,") && out.contains("y INTEGER ;"));
        assert!(out.contains("z AS Dates "), "`AS Dates` is not an annotation (a letter follows)");
        assert!(out.contains("ASLong"), "no word boundary and no space: kept");
        assert!(out.contains("VALUES (0, '', -1, 0)"));
        assert!(out.contains("RAISE(ABORT, 'System category can''t be deleted')"));
        assert!(!out.contains('"'));
    }

    #[test]
    fn every_embedded_migration_is_free_of_annotations_and_quotes() {
        for (i, sql) in MIGRATIONS.iter().enumerate() {
            assert!(!sql.lines().any(|l| l.starts_with("import ")), "{}.sql", i + 1);
            for word in ["AS Boolean", "AS Long", "AS Float", "AS Date"] {
                assert!(!sql.contains(word), "{}.sql has {word}", i + 1);
            }
            assert!(!sql.contains('"'), "{}.sql has a double-quoted string", i + 1);
        }
    }

    #[test]
    fn every_migration_executes_in_order_on_a_version_14_database() {
        let c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "OFF").unwrap();
        c.execute_batch(include_str!("../tests/fixtures/legacy_v13.sql")).unwrap();
        c.pragma_update(None, "user_version", OLDEST_IMPORTABLE).unwrap();
        replay(&c, OLDEST_IMPORTABLE, false).unwrap();
        assert_eq!(c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 33);
    }
}
