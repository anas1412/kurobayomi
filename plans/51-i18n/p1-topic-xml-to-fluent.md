# 51-i18n topic: moko XML to Fluent conversion

Scope: the base files `i18n/src/commonMain/moko-resources/base/strings.xml`, `i18n/src/commonMain/moko-resources/base/plurals.xml`, `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`, `i18n-aniyomi/src/commonMain/moko-resources/base/plurals.xml`, and the same file names in each translated locale directory.

PROPOSAL: Kurobayomi keeps the Android-format XML as the translation source (Weblate exports these files), converts them once with an `xtask` into [Fluent](https://projectfluent.org/) `.ftl` files, and loads the `.ftl` files at runtime with `fluent-bundle`. Reasons: Fluent has named arguments, CLDR plural selection per language (so no separate plural rules), and it is designed for translator-edited text. The alternative (gettext `.po`) needs a runtime library and has no named placeholders.

## 1. Source format facts (OBSERVED)

### 1.1 `<string>`

- Attributes seen: `name` (always), `translatable="false"` (22 names total, see the inventory).
- XML comments `<!-- … -->` are section markers and translator notes. Two of them are translator instructions: `<!-- Do not translate "WebView" -->` (strings.xml, `action_open_in_web_view`, `information_webview_required`, `information_webview_outdated`, `exception_http`) and `<!-- This should be to the left for RTL locales -->` (before `pref_chapter_swipe_end`, strings.xml line 310).
- Text escapes present in base values (counts over both base strings files):
  - `\'` (apostrophe): e.g. `onboarding_description` `Let\'s set some things up first.`
  - `\"` (double quote): e.g. `action_add_repo_message` `…ends with \"index.min.json\".`
  - `\n` (newline): e.g. `onboarding_storage_info` `…chapter downloads, backups, and more.\n\nA dedicated folder…`
  - `&amp;` (ampersand): e.g. `pref_appearance_summary` `Theme, date &amp; time format`.
  - Literal `%` placeholders (section 3).
- Unicode characters used as text: `…` (U+2026) in `action_search_hint` ("Search…"), `loading` ("Loading…"), `transition_pages_loading` ("Loading pages…"), `update_check_notification_download_in_progress` ("Downloading…"), `notification_updating_progress` ("Updating library… (%s)").

### 1.2 `<plurals>`

- `<item quantity="…">` children. Quantity values in translated files: `zero`, `one`, `two`, `few`, `many`, `other`. Base files use only `one` and `other` (OBSERVED).
- Within a plural the placeholders may differ between items (see behaviour spec §2).

### 1.3 Files that do not contain text

- `<resources/>` only (empty). Example: `i18n-aniyomi/src/commonMain/moko-resources/ur/strings.xml` has three lines and no `<string>`. Converter rule: an empty file yields a locale with zero messages. Such a locale is not offered in the language picker (see the locale topic).

## 2. Escaping conversion (PROPOSAL, exact)

Applied to every `<string>` and `<item>` text, in this order:

| Source (XML text) | Meaning | Fluent output |
|---|---|---|
| `\\` | backslash | `\\` in a text pattern is not special in Fluent text; emit `\` followed by the literal (no escape needed) |
| `\'` | apostrophe | `'` |
| `\"` | double quote | `"` (text, no escape needed in Fluent text) |
| `\n` | newline | `{"\u000A"}` (a Fluent string literal with a Unicode escape; keeps the pattern on one line) |
| `\t` | tab | `{"\u0009"}` (not present in base; handled for translators) |
| `&amp;` | ampersand | `&` |
| `&lt;` / `&gt;` / `&quot;` / `&apos;` | XML entities | `<` / `>` / `"` / `'` |
| `{` | literal brace | `{"{"}` |
| `}` | literal brace | `{"}"}` |
| `%%` | literal percent | `%` |
| leading spaces | Fluent trims leading and trailing spaces of a pattern | emit `{" "}` for each leading space that must be kept |

Fluent rules that the converter must respect:
- A pattern value cannot start with `[`, `*`, or `.` on a continuation line. Message values are single-line here, so this matters only if a translator puts a newline plus such a character in a value. The converter emits `{"\u000A"}` for every newline, so this case cannot occur.
- Variants: plural messages are written with `{ $count -> … }`. The converter writes the `*[other]` variant as the default.

Escaping must be checked by the test in the edge-cases file (round-trip: a Fluent pattern rendered with no arguments equals the XML text with the escapes resolved).

## 3. Placeholder conversion (PROPOSAL, exact)

Android formats with Java `String.format` rules. Each placeholder becomes a named Fluent argument.

Naming: argument names are `arg1`, `arg2`, … `argN` where N is the 1-based argument index that Android uses. Plural count is always the Fluent argument `count`.

Mapping rules (`%` followed by optional `N$`, flags, width, precision, conversion):

| Java specifier | Meaning | Fluent output | Notes |
|---|---|---|---|
| `%s` (sequential) | string, next argument | `{$argK}` | K is the running counter, starting at 1, incremented per sequential specifier |
| `%N$s` | string, argument N | `{$argN}` | |
| `%d` (sequential) | integer | `{NUMBER($argK, useGrouping: "false")}` | Java `%d` prints no grouping |
| `%N$d` | integer, argument N | `{NUMBER($argN, useGrouping: "false")}` | |
| `%0NNd` (e.g. `%02d`) | zero-padded integer | `{NUMBER($argN, minimumIntegerDigits: 2, useGrouping: "false")}` | Two digits, as in `restore_duration` |
| `%.2f` | decimal, two places | `{NUMBER($argN, minimumFractionDigits: 2, maximumFractionDigits: 2, useGrouping: "false")}` | `player_speed` (non-translatable) |
| `%c` | character | `{$argK}` | Caller passes a one-character string |
| `%%` | percent | `%` | |

Verification item (for the implementer, not decided here): the exact accepted form of boolean-valued NUMBER options in `fluent-bundle` must be confirmed by a unit test (`useGrouping: "false"` as a string literal versus another form). The test is in the edge-cases file.

Examples (base English):

| XML | Fluent |
|---|---|
| `Restrictions: %s` | `Restrictions: { $arg1 }` |
| `%1$s - E%2$s - %3$s` | `{ $arg1 } - E{ $arg2 } - { $arg3 }` |
| `Default (%d)` | `Default ({ NUMBER($arg1, useGrouping: "false") })` |
| `%1$02d min, %2$02d sec` | `{ NUMBER($arg1, minimumIntegerDigits: 2, useGrouping: "false") } min, { NUMBER($arg2, minimumIntegerDigits: 2, useGrouping: "false") } sec` |
| `Download (%1$d%%)`-style `%1$d%%` | `{ NUMBER($arg1, useGrouping: "false") }%` |

Fluent whitespace inside `{ }` is free; the examples keep it for readability.

## 4. Plural conversion (PROPOSAL, exact)

XML:

```xml
<plurals name="relative_time">
    <item quantity="one">Yesterday</item>
    <item quantity="other">%1$d days ago</item>
</plurals>
```

Fluent:

```ftl
relative_time =
    { $count ->
        [one] Yesterday
       *[other] { NUMBER($count, useGrouping: "false") } days ago
    }
```

Rules:
- The Android count argument is the Fluent `count` argument, always the first argument of a plural. Placeholders in the pattern that refer to the count (`%d` or `%1$d` that Android fills with the same number) become `{ NUMBER($count, useGrouping: "false") }`.
- Additional placeholders use `$arg2`, `$arg3`, and so on. Plural calls in Kotlin pass the count as the plural quantity and then as the first format argument, for example `pluralStringResource(MR.plurals.x, n, n)`. Java `%1$d` therefore equals the count.
- Exception, observed: `notification_chapters_multiple_and_more` and `notification_episodes_multiple_and_more` are called with `(plural, remaining, joinedNumbers, remaining)` (`app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt:358-362` and `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt:372-376`). Their quantity is `remaining`, `%1$s` is the joined list, and `%2$d` is `remaining` again. Conversion keeps `$count` as the selector and `$arg1` as the list. See the example below.
- Quantity categories: Fluent selects `zero`, `one`, `two`, `few`, `many`, `other` with the CLDR rules of the bundle locale. Variants not present in a translation fall back to `*[other]`. This is the same behaviour as Android.
- A plural with identical `one` and `other` (for example `download_queue_summary`) is still written as a select with both variants; the output is the same.

Plural examples from the base files:

```ftl
notification_chapters_multiple_and_more =
    { $count ->
        [one] Chapters { $arg1 } and 1 more
       *[other] Chapters { $arg1 } and { NUMBER($arg2, useGrouping: "false") } more
    }
```

Note: in `notification_chapters_multiple_and_more` the `one` form is `Chapters %1$s and 1 more` (the `1` is literal text). The `other` form uses `%2$d`, which the caller fills with the same `remaining` value used as the quantity. In Rust this message takes `count` and `arg2` as one value (the API passes `remaining` for both), so the two can never diverge. The same holds for `notification_episodes_multiple_and_more`.

## 5. Output layout

- One file per locale: `locales/<bcp47>/kurobayomi.ftl` (for example `locales/pt-BR/kurobayomi.ftl`). The English file is `locales/en/kurobayomi.ftl`.
- Message order: all `i18n-aniyomi` messages first, then `i18n` messages not already defined (same key defined in both: keep the Aniyomi version only). Each message is preceded by a comment `# source: <module>` for traceability.
- Non-translatable names (`translatable="false"`) are written to `en.ftl` only. Translation files must not contain them. A translation file that contains one is an error (test T-51a-009).
- Keys: dots become underscores; identifiers must match `[A-Za-z][A-Za-z0-9_-]*`. `player_sheets_filters_Saturation` keeps its capital `S`.

## 6. Overlap and precedence (OBSERVED + PROPOSAL)

27 names exist in both base files (list in the key catalog, rows marked "also in core"). Observed: the app reads these through `AYMR` only; the `MR` copies are never referenced.

PROPOSAL: For those 27 keys the Kurobayomi catalog takes the Aniyomi value for English and for every locale. For a locale, the fallback chain for an overlap key is: Aniyomi translation → Aniyomi English. The core translation is never used for an overlap key, even if it exists. This is the rule that matches what Aniyomi shows the user.

## 7. Locale-specific edge cases in conversion

- Locale directories exist for 66 locales in the core module and 67 in the Aniyomi module (including `ur`, which is empty). Convert every non-empty directory. The directory name maps to a BCP 47 tag through the table in [p1-topic-locale-and-display-names.md](p1-topic-locale-and-display-names.md).
- A translated file may contain a `<string>` whose name is not in base (a stale key). Converter: drop it, log `unknown key <name> in <locale>`.
- A translated `<string>` may be missing (partially translated locale: for example `te` has 192 strings in core and 1 in Aniyomi). Converter: omit the message. Runtime fallback fills the gap (English, or Aniyomi English for overlap keys).
- A translated plural may lack `other`. Converter: fail that plural in that locale with an error, fall back to English for the plural. Android would fall back per quantity; Fluent requires a default variant, so the whole plural falls back to English.
- Placeholder check (required, runs in `xtask`): the set of Java placeholders in the translation must equal the set in the base message (same count, same argument numbers). Mismatch = error for that message in that locale, fallback to English. Reason: a missing argument renders as the raw placeholder text in Fluent and crashes Android's `String.format` (which throws `MissingFormatArgumentException`).

## 8. Runtime loading (PROPOSAL)

- `xtask i18n-import` writes `.ftl` files into `kurobayomi-i18n/locales/`.
- The app embeds them with `include_str!` through a build script that generates `static LOCALES: &[(&str, &str)]` (tag, file text). Size estimate: about 66 files of 50 to 110 KB each; under 10 MB total. Alternative if the binary size matters: GResource (`glib-build-tools` `compile_resources`), loaded with `gio::resources_lookup_data`. Choose at implementation.
- Parsing: `FluentResource::try_new(String)`. On parse error the resource is kept with the valid entries (Fluent's `Err` returns the partial result; log the errors and use `res` from `Err((res, errs))`).
- `FluentBundle` per locale, built lazily on first use and cached.
- `set_use_isolating(false)` on every bundle so placeholders are not wrapped in U+2068 and U+2069 marks.
