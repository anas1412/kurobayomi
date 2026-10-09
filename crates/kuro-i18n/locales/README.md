# Kurobayomi translations

- English source: `en/kurobayomi.ftl`. Every other locale is a `<tag>/kurobayomi.ftl` file in this folder.
- Translators edit in Weblate or edit the `.ftl` files directly. The file format is Fluent (`.ftl`), not Android XML.
- Regenerating from the Aniyomi XML: `kuro-i18n-import <aniyomi-src-dir> <output-file>` writes the English file, and `kuro-i18n-import locales <aniyomi-src-dir> <locales-dir>` writes every locale file. Run it only when the upstream strings change, because it overwrites the `.ftl` files. Make translation edits in Weblate first.
- Validation: `kuro-i18n-import` fails on any Fluent or placeholder error. The planned `xtask i18n-import` and `xtask i18n-check` wrappers (T-51a-008, T-51a-009) do not exist yet.
- Overlap names (defined in both the core and Aniyomi XML) take the Aniyomi English text, and each is defined once in `en/kurobayomi.ftl` (test `overlap_precedence`).

Not ported: the Gradle, moko and Android manifest plumbing. See `plans/51-i18n/p1-data-and-config.md` section 2.
