# Topic: mpv script editor, code editor, and the syntax highlighter

Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Owner area: 48-settings-player-and-scripts. The file-list behaviour is in `p1-behavior.md` sections 10 and 11; this file holds the exact rule sets and the Linux design decisions for the highlighter.

## 1. Files on disk

Base directory `<base>/mpv-config/` (see `p1-behavior.md` section 16):

| Path | Edited by | Read by player |
|---|---|---|
| `mpv-config/scripts/*.lua` | Script editor (type "Scripts", extension `lua`) | Copied to `filesDir/mpv/scripts` when `mpv_scripts` is on (`PlayerActivity.kt` `copyUserFiles`) |
| `mpv-config/script-opts/*.conf` | Script editor (type "Script options", extension `conf`) | Copied to `filesDir/mpv/script-opts` when `mpv_scripts` is on |
| `mpv-config/shaders/*` | Not edited in-app | Copied to `filesDir/mpv/shaders` when `mpv_scripts` is on |
| `mpv-config/fonts/*` | Not edited in-app (subtitle typography card reads it) | Copied to `filesDir/mpv/fonts`; set as `sub-fonts-dir` and `osd-fonts-dir` |
| `mpv-config/mpv.conf`, `mpv-config/input.conf` | Mirror written by the Advanced page when the all-files permission exists | **Never read** (observed). The player reads the preference values instead. |

Internal runtime directory `filesDir/mpv/` (`MPV_DIR = "mpv"`, `PlayerActivity.kt:176`) is rebuilt at each player start (`p1-behavior.md` section 9). `mpv.conf` and `input.conf` there are overwritten from preferences every time.

## 2. Editor file list: rules that matter for the port

- Only regular files are listed; subdirectories are skipped.
- Unsorted (filesystem order).
- Creating a file whose name already exists (ignoring case) is blocked by validation, not by the filesystem.
- Rename does not enforce the extension, and the list is not refreshed when files change outside the app.
- Name validity uses `DiskUtil.buildValidFilename` (see `p1-behavior.md` section 10).

**Proposal (Rust):**
- List with `std::fs::read_dir`, filter `is_file()`, sort by name case-insensitively (a deliberate improvement; keep the list stable across refreshes).
- Refresh the list on window focus and after each own operation. Optionally watch the directory with `gio::FileMonitor`.
- Enforce the type extension on rename (keep the extension, let the user edit only the base name), or show a warning. Record as a change.
- Use the same character rules as `buildValidFilename`, but treat a leading dot as allowed only if the user confirms (hidden files). Default: keep the Kotlin rule.
- Size label: decimal units (1000), as in `StorageUtil.kt`. Date label: `MMMM d, yyyy HH:mm` in the system locale. Use `chrono` with the locale's month names if available, otherwise English.

## 3. Code editor: rules that matter for the port

- Loads the whole file as UTF-8 (lossy). Saves the full text back as UTF-8. Non-UTF-8 files change on the first save.
- Highlight only when the text is at most **15000** characters (`CodeEditScreenModel.kt:133`). Above that the editor is plain text.
- Modified flag set by any text change; reset only by a successful save.
- Save truncates then writes (not atomic). Proposal: write a sibling temp file and rename.
- Unsaved-changes guard on back (app bar and system back).
- Line gutter: numbers `1..n` where `n` is the layout line count (not the logical line count, so wrapped lines are counted as lines in the Kotlin version; the port should count logical lines and disable wrapping).
- Monospace 14 sp, line height 20 sp.

**Proposal (Rust):** `gtk::TextView` with monospace font (`set_monospace(true)`), `set_wrap_mode(None)` (no wrap, horizontal scroll, matching the Kotlin horizontal scroll), `set_show_line_numbers` only if GtkSourceView is used. Preferred: `sourceview5` crate (GtkSourceView 5) with a `GtkSourceBuffer` and its own `GtkSourceLanguage`. Lua is available as a built-in language; for `conf` use the built-in `ini` language as the closest match, and keep the custom regex colouring (section 5) for the exact Kotlin look. **Verify crate compatibility**: `sourceview5` must target the same `gtk4` major/minor as `gtk = gtk4 0.11`. If it does not, fall back to a plain `gtk::TextView` plus `gtk::TextTag`s applied from the same rule set.

## 4. Highlighter engine (`Highlight.kt`)

Data model (exact):

```kotlin
sealed class HighlightType(open val regex: Regex, open val isString: Boolean) {
    data class Full(override val regex: Regex, override val isString: Boolean = false, val color: Color)
    data class Groups(override val regex: Regex, override val isString: Boolean = false, val colors: List<Color>)
}
data class Highlight(val rules: List<HighlightType>)
```

Algorithm `Highlight.toAnnotatedString(text)`:
```
spanStyles = []
stringRanges = []                       // list of IntRange
for rule in rules (in declared order):
    for match in rule.regex.findAll(text):          // non-overlapping, left to right
        for (index, group) in match.groups.withIndex():
            if group == null: continue
            if rule is Groups and index == 0: continue   // Groups rules colour only capture groups
            if any range in stringRanges contains group.range.first: continue
            color = rule is Full ? rule.color : rule.colors[index - 1]
            spanStyles += Span(color, start = group.range.first, end = group.range.last + 1)
            if rule.isString: stringRanges += group.range
return AnnotatedString(text, spanStyles)
```

Observed properties:
- **Full rules colour every non-null group**, including capture groups. For the Lua multi-line comment rule, group 1 (the `=` run) is coloured as a comment too. For the Lua string rule, group 1 duplicates group 0.
- **Groups rules index colours from `colors[index - 1]`.** A regex with more capture groups than colours throws `IndexOutOfBoundsException`. All built-in Groups rules have exactly one capture group.
- **Ranges are not removed.** Later rules add spans on top of earlier ones. Compose draws overlapping `SpanStyle`s, so the later colour wins where they overlap (the rule order below therefore matters: comments and strings last). Verify visually in the port.
- Strings are detected by rule order: the string rule is 8th (Lua) or 5th (conf). Matches of earlier rules that start inside a string are still added, then covered by the later string span. Matches of rules **after** the string rule that start inside a recorded string are skipped.
- Only the **start index** of a group is tested against `stringRanges`.

The palette `githubTheme` (`SyntaxHighlight.kt:5-13`):

| Name | Hex |
|---|---|
| keyword | `FF7B72` |
| literal | `79C0FF` |
| comment | `9198A1` |
| string | `A5D6FF` |
| definition | `D2A8FF` |
| key | `7EE787` |
| value | `D29922` |

(Each as `Color(0xFFxxxxxx)`, fully opaque. The class `SyntaxColors` has exactly these seven fields.)

### 4.1 Lua rules (`luaHighlight(githubTheme)`, `SyntaxHighlight.kt:27-96`), in order

| # | Type | Regex (verbatim from source, Kotlin raw string) | Flags | Colour |
|---|---|---|---|---|
| 1 | Full | `(?<![=<>~:/])(?:==\|~=\|<=\|>=\|\/\/\|\.\.\|[=+\-*/%^#<>])` | none | keyword |
| 2 | Full | `\b([a-zA-Z_]\w*)(?=\s*:)` | none | definition |
| 3 | Groups [literal] | `([a-zA-Z_]\w*)(?=\s*\()` | none | literal |
| 4 | Groups [definition] | `function\s+([a-zA-Z_][\w.]*)(?=\s*\()` | none | definition |
| 5 | Groups [keyword] | `\b(and\|break\|do\|else\|elseif\|end\|for\|function\|if\|in\|local\|not\|or\|repeat\|return\|then\|until\|while)\b` | none | keyword |
| 6 | Groups [literal] | `\b(true\|false\|nil)\b` | none | literal |
| 7 | Full | `(?<!\w)[+-]?(?:0x[\da-f]+\|(?:(?:\.\d+\|\d+(?:\.\d*)?)(?:e[+\-]?\d+)?))` | IGNORE_CASE | literal |
| 8 | Full, isString | `(?<!--[^\n]{0,120})("(?:[^"\\]\|\\[\s\S])*"\|\'(?:[^'\\]\|\\[\s\S])*\')` | none | string |
| 9 | Full | `--.*$` | MULTILINE | comment |
| 10 | Full | `--\[(=*)\[.*?--\]\1\]` | DOT_MATCHES_ALL | comment |

Note: rule 1 is written as `"""(?<![=<>~:/])(?:==|~=|<=|>=|\/\/|\.\.|[=+\-*/%^#<>])"""` in the source; in the table `\|` is only an escape for the Markdown table. The regex itself contains `|` alternation. Rule 9 uses `${'$'}` in Kotlin, which is a literal `$`.

Rule 10 uses a back-reference `\1` (matches the same number of `=` as the opening bracket). This is the reason the Rust port needs a regex engine with back-references.

### 4.2 conf rules (`confHighlight(githubTheme)`, `SyntaxHighlight.kt:98-137`), in order

| # | Type | Regex (verbatim) | Flags | Colour |
|---|---|---|---|---|
| 1 | Full | `^\s*[\w.-]+(?=\s*=)` | MULTILINE | literal |
| 2 | Full | `(?<==)[ \t]*[\w ~,./;:-]+` | none | literal |
| 3 | Groups [value] | `(?<![\w-])(yes\|no)(?![\w-])` | none | value |
| 4 | Full | `\b\d+(\.\d+)?\b` | none | value |
| 5 | Full, isString | `(?<!#[^\n]{0,120})("(?:[^"\\]\|\\.)*"\|'(?:[^'\\]\|\\.)*')` | none | key |
| 6 | Full | `#.*` | none | comment |

Notes:
- Rule 2 uses a look-behind `(?<==)` (position just after `=`), so it colours the value.
- Rule 4 has a capture group `(\.\d+)?` which Full rules also colour (the decimal part is coloured twice).
- Rule 5 colours strings with the **key** colour (`7EE787`), not the string colour. Observed and intentional in source.
- `.` in rule 5 does not match newline (no DOT_MATCHES_ALL), so multi-line strings are not matched in conf files.

### 4.3 Regex dialect requirements

- Lookahead and look-behind (`(?=`, `(?!`, `(?<=`, `(?<!`) are required (rules 1-3, 5-8 in Lua; several in conf).
- Back-reference `\1` is required (Lua rule 10).
- Java `\w` is ASCII `[A-Za-z0-9_]` by default. Rust `regex` and `fancy-regex` use Unicode `\w` by default. **Replace every `\w` with `[A-Za-z0-9_]` and every `\d` with `[0-9]`** to keep the output identical for non-ASCII text.
- `\s` in Java matches `[ \t\n\x0B\f\r]`; Rust `\s` is Unicode. Use `[ \t\n\x0B\f\r]` explicitly if identical behaviour matters for comments and keys.
- Flags map: IGNORE_CASE → `(?i)`, MULTILINE → `(?m)`, DOT_MATCHES_ALL → `(?s)`.
- The Rust crate choice is `fancy-regex` (`p1-rust-design.md` section 6). Compile every rule once at startup (`once_cell::sync::Lazy` or `std::sync::OnceLock`).

## 5. Performance

- Each keystroke re-runs the whole rule set on the whole text (no incremental highlight). Limit: 15000 characters, then plain text.
- Rule 10 uses `.*?` with DOT_MATCHES_ALL; on a large unterminated `--[[` it scans to the end of the text for every start position. Mitigate by the 15000-character cap and by running highlight on a worker thread for texts above 4000 characters (proposal).
- `findAll` returns non-overlapping matches; overlapping highlights are produced by the rule order, not by the matcher.

## 6. Editor-related decisions for the port

- Keep the regex rule sets as data (`const LUA_RULES: &[Rule]` with `name`, `regex`, `kind`, `colours`). This makes the rule order visible in review.
- The highlighter is shared with the custom-button dialog (`p1-topic-custom-buttons.md` section 5, which highlights Lua with no limit). The port uses one implementation, with the 15000-character limit applied only to the full-file editor (documented as a deliberate difference; the dialog is small).
- Scripts can be edited while the player runs; the port should warn that changes apply at the next player start (the Kotlin app has no such warning).
