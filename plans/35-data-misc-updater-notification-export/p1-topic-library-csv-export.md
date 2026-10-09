# Topic: library CSV export (byte-exact)

Scope file: `app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt` (96 lines). UI owner: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (outside scope, described in p1-ui-parity.md section 4). Data owners: area:07-domain-entries-items (domain models), area:05-data-repos-entries-items (repositories), area:08-domain-library-category-history-updates (favourites interactors).

## 1. Purpose

Lets the user save a list of their library (every favourite manga and anime) as a spreadsheet-friendly file, with a choice of columns. Nothing is imported back from this file; there is no matching import in Aniyomi.

## 2. Input

Favourites: every row with `favorite = 1` from `animes` (first) and `mangas` (second), in SQLite scan order (`animes.sq:87-90`, `mangas.sq:76-79`). Fields used: `title`, `author` (nullable), `artist` (nullable), and the entry type. Kurobayomi adds `ORDER BY _id` (PROPOSAL) for a reproducible order; the byte output for a fixed DB is then fixed.

## 3. Column model

| Option | Column text | Value | Order when enabled |
|---|---|---|---|
| Title | (none, no header row) | `title` as stored | 1 |
| Type | (none) | `"manga"` or `"anime"` (lowercase of the enum name) | 2 |
| Author | (none) | `author` or empty | 3 |
| Artist | (none) | `artist` or empty | 4 |

There is no header row (OBSERVED: `generateCsvData` adds no header). Columns appear in the fixed order above regardless of the order of the checkboxes.

## 4. Cell rule (exact)

For a cell value `v` (nullable):
1. `v == null` or `v` is blank (empty or only whitespace, Kotlin `isBlank`) -> cell is the empty string.
2. Else if `v` contains any of `\r`, `\n`, `"`, `,` -> cell is `"` + `v` with each `"` replaced by `""` + `"`.
3. Else cell is `v` exactly (spaces kept, not quoted).

Notes:
- Rule 1 happens before rule 2, so a blank value containing a comma is emptied, not quoted.
- Only the four characters in rule 2 trigger quoting. A value with a leading or trailing space and nothing else is written raw. Spreadsheet programs trim such spaces; that is parity behaviour.

## 5. Row and file rule (exact)

- A row is the enabled cells joined with `,`.
- Rows are joined with `\r\n`.
- There is no trailing newline after the last row.
- Zero favourites: the file is empty (0 bytes).
- Zero enabled columns: every row is the empty string, so N rows give N-1 `\r\n` pairs (N = 1 gives an empty file; N = 2 gives `\r\n`).
- Encoding: UTF-8 bytes of the string, no byte-order mark.

## 6. Worked examples

Input (all four options on):

| # | kind | title | author | artist |
|---|---|---|---|---|
| 1 | anime | Frieren | Kanehito Yamada | (null) |
| 2 | manga | He said "hi", then left | line one<LF>line two | (empty string) |
| 3 | manga | Padding  | (whitespace only) | Ayumi |

Output bytes (`<CRLF>` stands for the two bytes `\r\n`, `<LF>` for one line-feed byte inside a value):

```
Frieren,anime,Kanehito Yamada,<CRLF>
"He said ""hi"", then left",manga,"line one<LF>line two",<CRLF>
Padding  ,manga,,Ayumi
```

Row 3: `Padding  ` has trailing spaces and is not quoted (rule 3). The whitespace-only author becomes empty (rule 1). Row 2: the author contains a newline, so it is quoted (rule 2).

Options with Title off and Type on (the UI prevents this, the renderer does not):
```
anime
manga
manga
```
with `<CRLF>` between rows and none at the end.

## 7. Kotlin to Rust mapping

| Kotlin (`LibraryExporter.kt`) | Rust (`src/export/library_csv.rs`) |
|---|---|
| `escapeRequired = listOf("\r", "\n", "\"", ",")` (line 63) | `v.contains(['\r', '\n', '"', ','])` |
| `column.isNullOrBlank()` (line 87) | `None` or `v.trim().is_empty()` |
| `column.replace("\"", "\"\"").let { "\"$it\"" }` (line 89) | `format!("\"{}\"", v.replace('"', "\"\""))` |
| `rows.joinToString("\r\n")` (line 85) | `rows.join("\r\n")` |
| `columns.joinToString(",")` (line 86) | `cells.join(",")` |
| `csvData.toByteArray()` (line 57) | `text.as_bytes()` |

`isBlank()` in Kotlin treats any `Char.isWhitespace()` character as blank; Rust `char::is_whitespace` uses the Unicode White_Space property. The two sets agree for ordinary text. Any difference is limited to rare control and format code points in author or artist fields and is accepted; the golden test covers only ASCII and common Unicode spaces.

## 8. Writing

Aniyomi: `context.contentResolver.openOutputStream(uri)` then write (LibraryExporter.kt:55-58), then the success callback runs even when the stream is null (silent defect).

Kurobayomi (PROPOSAL, T-35a-074):
1. Build the full string in memory (library sizes make this safe; streaming is a later change if a library is above 100 000 rows).
2. Write `<name>.kuro-tmp` in the same directory, `sync_all`, rename over the target.
3. On any error remove the temp file and return `ExportError::Write(path, io_error)`; the UI shows the error text.
4. Run on `spawn_blocking`; never on the GTK thread.

## 9. Tests (byte-exact)

- Example in section 6 compared with a golden file (`tests/fixtures/export_golden_all_columns.csv`, bytes compared, not text).
- Zero favourites => empty output.
- Zero columns with 1 and 2 rows (empty and `\r\n`).
- Quote doubling `He said "hi"` => `"He said ""hi"""`.
- Blank author (`""`, `"   "`) => empty cell.
- Trailing space kept, not quoted.
- Null author and artist => empty cells.
- `write_csv` replaces an existing file; a failed write leaves the old file untouched and no temp file.

Fixtures are described in p1-edge-cases-and-tests.md section 6.

## 10. Known behaviours kept on purpose (parity)

- No header row. A header would make the file friendlier but changes the byte contract; adding one is a user-visible change and is not in this plan.
- No formula escaping. A title starting with `=` is written raw; spreadsheet programs may evaluate it. Documented in the export help text (PROPOSAL string).
- No BOM. Excel on Windows reads non-ASCII names incorrectly without one; Kurobayomi targets Linux spreadsheet tools which read UTF-8 by default.
- Options are not saved between app runs.
- The file name suggestion is `kurobayomi_library.csv` (the Android default was `aniyomi_library.csv`, SettingsDataScreen.kt:397). The brand change is the only intentional difference in the default file name.
