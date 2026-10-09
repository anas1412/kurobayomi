# Area 07 — topic: chapter / episode / season number recognition

Files: `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterRecognition.kt` (127 lines), `domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeRecognition.kt` (138), `domain/src/main/java/tachiyomi/domain/items/season/service/SeasonRecognition.kt` (137); test `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt` (34 `@Test` methods, 57 assertions). The episode and season recognisers have NO upstream tests. Every vector in this file was executed against a Python prototype of the algorithm below (`re` module, consuming-prefix rewrite of the look-behinds); the prototype reproduces all 57 upstream assertions, so the rewrite is behaviour-equivalent on that suite. Expected values for episode/season were produced by the same prototype and are therefore "what the Kotlin should return by construction of its regexes", not independently confirmed against a JVM; the Rust task must still compare against a JVM run if one is available (T-07a-012 acceptance).

## 1. Purpose and call sites

Sources give chapters/episodes/seasons whose number field is often `-1` (unknown). The sync code (`SyncChaptersWithSource.kt:97-102`, `SyncEpisodesWithSource.kt:95-100`, `SyncSeasonsWithSource.kt:58-62`, area:10-app-domain-layer) calls `parse*Number(entryTitle, itemName, sourceNumber)` for every item. The result is stored in `chapter_number` / `episode_number` / `season_number`; it drives "mark duplicate as read" matching, missing-item detection, number sort and display ("Chapter 12.5").

Signature (all three): `parseXNumber(title: String, name: String, number: Double? = null): Double`.

## 2. Algorithm (identical skeleton)

```
1. if number != null and (number == -2.0 or number > -1.0): return number
2. s = name.lowercase()
       .replace(title.lowercase(), "")      // literal, all occurrences, empty title = no-op
       .trim()
       .replace(',', '.')
       .replace('-', '.')
   s = remove every whitespace char that is immediately followed by "extra" | "special" | "omake"
3. (episode and season only) while tagRegex matches anywhere in s: s = tagRegex.replaceAll(s, "")
4. matches = all non-overlapping matches of NUMBER in s, left to right
5. if matches.isEmpty(): return number ?: -1.0
6. if matches.len() > 1:
       n = unwanted.replaceAll(s, "")
       if m = basic.firstMatch(n): return valueOf(m)
       if m = NUMBER.firstMatch(n): return valueOf(m)
       // else fall through
7. return valueOf(matches[0])
```

`valueOf(m) = double(group1) + addition` where `addition`:

```
if group2 non-empty:                     // ".005" -> 0.005, ".1" -> 0.1
    return double(group2)
if group3 non-empty:
    if group3 contains "extra":   return 0.99
    if group3 contains "omake":   return 0.98
    if group3 contains "special": return 0.97
    t = group3 with leading '.' characters removed
    if t.len == 1: return alphaPostfix(t[0])
return 0.0
alphaPostfix(c) = n = c - 'a' + 1; if n >= 10: 0.0 else n / 10.0      // a=0.1, b=0.2, c=0.3, d=0.4, e=0.5, f=0.6, g=0.7, h=0.8, i=0.9, j through z = 0.0
```

`NUMBER = ([0-9]+)(\.[0-9]+)?(\.?[a-z]+)?`. Notes: group 3 may be several letters (`vol`, `extravol`, `p`, `th`), in which case only the three keywords or a single-letter suffix contribute; anything else is ignored (0.0). Because step 2 lower-cases first, only lowercase letters ever reach `[a-z]`; non-ASCII letters never match `[a-z]` (so `4é` gives group3 empty).

The value is computed as an IEEE double sum `initial + addition`. Expected values in tests must be written as that sum or compared with the same expression, because e.g. `3.0 + 0.97` is `3.9699999999999998` and is not the literal `3.97` (while `404.0 + 0.99` equals the literal `404.99` and `4.0 + 0.1` equals `4.1`). The upstream Kotlin test compares with `shouldBe` on the literal and passes only for the values listed; copy those literals only for those inputs.

## 3. Regexes per recogniser

| Recogniser | `basic` (look-behind) | `unwanted` | tag strip |
|---|---|---|---|
| Chapter | `(?<=ch\.) *NUMBER` | `\b(?:v\|ver\|vol\|version\|volume\|season\|s)[^a-z]?[0-9]+` | none |
| Episode | `(?<=\be\.\|\be\|episode\|\bep) *NUMBER` | `\b(?:v\|ver\|version\|season\|s)[^a-z]?[0-9]+\|\b\d+p\b\|hi10` | `^\[[^\]]+\]\|\[[^\]]+\]\s*$\|^\([^\)]+\)\|\([^\)]+\)\s*$` |
| Season | `(?<=\bs\.\|\bs\|season) *NUMBER` | `\b\d+p\b\|\d+x\d+\|Hi10\|\(\d+\)` | same as episode |

All three share `unwantedWhiteSpace = \s(?=extra|special|omake)`.

`\|` above is an escaped alternation bar for the markdown table only; in the regex it is a plain `|`.

## 4. Rust regex crate rewrite (Proposal)

The `regex` crate has neither look-behind nor look-ahead. Both uses reduce to consuming forms with identical match results for these patterns:

| Kotlin | Rust (`regex`) | Why equivalent |
|---|---|---|
| `\s(?=extra\|special\|omake)` replaced by `""` | `\s(extra\|special\|omake)` replaced by `$1` | the keywords contain no whitespace, so consuming them cannot hide a later match |
| chapter `basic`: `(?<=ch\.) *NUM`, take groups 1-3 | `ch\. *(?P<a>[0-9]+)(?P<b>\.[0-9]+)?(?P<c>\.?[a-z]+)?` | first position preceded by `ch.` with a number following is the same start as first `ch.` occurrence followed by number |
| episode `basic`: `(?<=\be\.\|\be\|episode\|\bep) *NUM` | `(?:\be\.\|\be\|episode\|\bep) *(?P<a>[0-9]+)(?P<b>\.[0-9]+)?(?P<c>\.?[a-z]+)?` | the alternatives end at different offsets but no input in the tested domain has two competing prefixes where the one starting first ends later (a prefix inside another needs a word boundary inside a word) |
| season `basic`: `(?<=\bs\.\|\bs\|season) *NUM` | `(?:\bs\.\|\bs\|season) *(?P<a>[0-9]+)(?P<b>\.[0-9]+)?(?P<c>\.?[a-z]+)?` | same argument (alternatives `\bs\.`, `\bs`, `season`) |

Alternative if exact semantics are ever in doubt: the `fancy-regex` crate supports look-around and can run the original patterns verbatim; it is slower (backtracking) and adds a dependency, so the consuming rewrite is the default. Decision recorded in p1-topic-quirks-and-decisions.md D1.

Other Rust specifics:

- `\b` in the `regex` crate is Unicode-aware by default; Java's `\b` is ASCII-ish for `\w` in this pattern context. After lower-casing, only digits and letters matter; use `(?-u:\b)` to force ASCII word boundaries and match Java for non-Latin titles (a Japanese character adjacent to `s2` would otherwise move the boundary).
- `$` in `tagRegex`: Java `$` also matches before a final line terminator; Rust `$` (without `(?m)`) only at the very end. Names do not contain newlines; the difference is irrelevant. Use `\z`-equivalent behaviour.
- `String::to_lowercase()` is Unicode-aware like Kotlin's `lowercase()` (Locale.ROOT). Both map `İ` to `i̇`.
- Compile the regexes once (`std::sync::LazyLock<Regex>`); parsing runs for every item on every refresh (hundreds to thousands of calls).
- Parse digits with `str::parse::<f64>()`; group1 can be arbitrarily long (e.g. a timestamp title `20240101`), still fine; a 400-digit run parses to `inf` in Rust and `Infinity` in Kotlin: same.

## 5. Verified vectors

### 5.1 Chapter (all 57 upstream assertions; title = column 1)

| Title | Name | Expected |
|---|---|---|
| Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4: Misrepresentation | 4.0 |
| Mokushiroku Alice | Mokushiroku Alice Vol. 1 Ch. 4: Misrepresentation | 4.0 |
| Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch. 4: Misrepresentation | 4.0 (test `Space between ch x`) |
| Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4.1: Misrepresentation | 4.1 |
| Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4.4: Misrepresentation | 4.4 |
| Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4.a: Misrepresentation | 4.1 |
| Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4.b: Misrepresentation | 4.2 |
| Mokushiroku Alice | Mokushiroku Alice Vol.1 Ch.4.extra: Misrepresentation | 4.99 |
| Bleach | Bleach 567 Down With Snowwhite | 567.0 |
| Bleach | Bleach 567.1 Down With Snowwhite | 567.1 |
| Bleach | Bleach 567.4 Down With Snowwhite | 567.4 |
| Bleach | Bleach 567.a Down With Snowwhite | 567.1 |
| Bleach | Bleach 567.b Down With Snowwhite | 567.2 |
| Bleach | Bleach 567.extra Down With Snowwhite | 567.99 |
| Solanin | Solanin 028 Vol. 2 | 28.0 |
| Solanin | Solanin 028.1 Vol. 2 | 28.1 |
| Solanin | Solanin 028.4 Vol. 2 | 28.4 |
| Solanin | Solanin 028.a Vol. 2 | 28.1 |
| Solanin | Solanin 028.b Vol. 2 | 28.2 |
| Solanin | Solanin 028.extra Vol. 2 | 28.99 |
| Onepunch-Man | Onepunch-Man Punch Ver002 028 | 28.0 |
| Onepunch-Man | Onepunch-Man Punch Ver002 028.1 | 28.1 |
| Onepunch-Man | Onepunch-Man Punch Ver002 028.4 | 28.4 |
| Onepunch-Man | Onepunch-Man Punch Ver002 028.a | 28.1 |
| Onepunch-Man | Onepunch-Man Punch Ver002 028.b | 28.2 |
| Onepunch-Man | Onepunch-Man Punch Ver002 028.extra | 28.99 |
| random | Vol.1 Ch.5v.2: Alones | 5.0 |
| Ayame 14 | Ayame 14 1 - The summer of 14 | 1.0 |
| Ayame 14 | Vol.1 Ch.1: March 25 (First Day Cohabiting) | 1.0 |
| random | Vol.001 Ch.003: Kaguya Doesn't Know Much | 3.0 |
| Onepunch-Man | Onepunch-Man Punch Ver002 086 : Creeping Darkness [3] | 86.0 |
| Ansatsu Kyoushitsu | Ansatsu Kyoushitsu 011v002: Assembly Time | 11.0 |
| Tokyo ESP | Tokyo ESP 027: Part 002: Chapter 001 | 27.0 |
| One-punch Man | Mag Version 195.5 | 195.5 |
| random | Foo | -1.0 |
| random | Fairy Tail 404: 00:00 | 404.0 |
| random | Asu No Yoichi 19a | 19.1 |
| Fairy Tail | Fairy Tail 404.extravol002 | 404.99 |
| Fairy Tail | Fairy Tail 404 extravol002 | 404.99 |
| Fairy Tail | Fairy Tail 404.omakevol002 | 404.98 |
| Fairy Tail | Fairy Tail 404 omakevol002 | 404.98 |
| Fairy Tail | Fairy Tail 404.specialvol002 | 404.97 |
| Fairy Tail | Fairy Tail 404 specialvol002 | 404.97 |
| One Piece | One Piece 300,a | 300.1 |
| One Piece | One Piece Ch,123,extra | 123.99 |
| One Piece | One Piece the sunny, goes swimming 024,005 | 24.005 |
| Solo Leveling | ch 122-a | 122.1 |
| Solo Leveling | Solo Leveling Ch.123-extra | 123.99 |
| Solo Leveling | Solo Leveling, 024-005 | 24.005 |
| Solo Leveling | Ch.191-200 Read Online | 191.2 |
| D.I.C.E | D.I.C.E[Season 001] Ep. 007 | 7.0 |
| The Gamer | S3 - Chapter 20 | 20.0 |
| One Outs | One Outs 001 | 1.0 |
| The Sister of the Woods with a Thousand Young | The 1st Night | 1.0 |
| The Sister of the Woods with a Thousand Young | The 2nd Night | 2.0 |
| The Sister of the Woods with a Thousand Young | The 3rd Night | 3.0 |
| The Sister of the Woods with a Thousand Young | The 4th Night | 4.0 |

(`Ch.191-200` hits `191.200`, parsed as 191 + 0.200 = 191.2; the Kotlin test writes `191.200`.)

### 5.2 Chapter — additional vectors (new tests)

| Title | Name | `number` arg | Expected |
|---|---|---|---|
| X | Ch.4.j | none | 4.0 (letter j gives n = 10, not below 10) |
| X | Ch.4.i | none | 4.9 |
| X | Ch.4.ab | none | 4.0 (two letters, not a keyword) |
| X | Chapter 12 | none | 12.0 |
| X | 12 | none | 12.0 |
| X | Foo | -1.0 | -1.0 |
| X | Foo | -2.0 | -2.0 (step 1 returns the sentinel) |
| X | Foo 7 | 5.0 | 5.0 (step 1) |
| X | Foo 7 | -2.0 | -2.0 |
| X | Foo 7 | -1.5 | 7.0 (not in the shortcut range, so parsed) |
| X | Foo 7 | -0.5 | -0.5 (> -1.0, returned as is) |
| X | Foo 7 | 0.0 | 0.0 |
| X | Foo | none | -1.0 |
| X | Ch.0 | none | 0.0 |
| X | Vol.2 Ch.10 Part 3 | none | 10.0 (three numbers, `unwanted` removes `vol.2`, `basic` then finds `ch.10`) |
| X | Ch.10-15 | none | 10.15 (hyphen becomes dot) |
| Ayame | Ayame 14 - 7 | none | 14.0 (see note below the table) |
| (empty) | Ch. 5 | none | 5.0 |

Note on the last-but-one row: `Ayame 14 - 7` with title `Ayame` becomes ` 14 - 7` -> `14 . 7`; two matches `14` and `7`; `unwanted` removes nothing; `basic` (`ch\.`) absent; the first `number` match wins: 14.0.

### 5.3 Episode (no upstream tests; prototype values)

| Title | Name | Expected |
|---|---|---|
| kaguya-sama wa kokurasetai | kaguya-sama wa kokurasetai - s01e01v2 (BD 1080p HEVC) | 1.0 |
| Kaguya | [Flugel] Kaguya-sama wa Kokurasetai - s01e01v2 (BD 1080p HEVC) [Multi Audio] [80AC7B2E] | 1.0 |
| Show | Show - Episode 12 | 12.0 |
| Show | Show Ep 12.5 | 12.5 |
| Show | Show E07 | 7.0 |
| Show | Show - 03 [1080p] | 3.0 |
| Show | Show 05 (BD 1080p) | 5.0 |
| Show | Show Episode 5 special | 5.0 + 0.97 (the space before `special` is removed, giving `5special`, group3 `special`) |
| Show | Show S2 Ep 3 | 3.0 |
| Show | Foo | -1.0 |
| Show | Show 12 Hi10 1080p | 12.0 |
| Show | [Group] Show - 08v2 [720p] | 8.0 |
| Show | Show e5.5 | 5.5 |
| Show | Show 1080p 03 | 3.0 |
| X | X - 07v2 | 7.0 |
| X | 1080p | 1080.0 (single match, nothing to strip: quirk) |
| X | S01E05 | 5.0 |
| X | S01E05 [1080p] | 5.0 |
| X | Ep.12.5 | 12.5 |
| X | (2020) Show 04 | 4.0 (leading `(2020)` stripped by `tagRegex`) |

With `number` argument: `("x","Foo", 5.0)` gives 5.0; `-2.0` gives -2.0; `-1.0` gives -1.0; `-0.5` gives -0.5; `("x","Foo 3", -1.0)` gives 3.0.

### 5.4 Season (no upstream tests; prototype values)

| Title | Name | Expected |
|---|---|---|
| Boku no Hero Academia | Boku no Hero Academia Season 2 | 2.0 |
| Boku no Hero Academia | [FLE] Boku no Hero Academia Season 2 (BD Remux 1080p H.264 FLAC) [Dual Audio] | 2.0 |
| Boku no Hero Academia | Boku.no.Hero.Academia.S02.1080p-ITH | 2.108 (QUIRK: the dotted title is not removed; the single match is `02.1080p` = 2 + 0.108; the comment in the source claims 2) |
| Boku no Hero Academia | Boku no Hero Academia 2 | 2.0 |
| Boku no Hero Academia | Boku no Hero Academia (2017) | -1.0 (trailing `(2017)` is a tag, nothing left) |
| Show | Show Season 3 Part 2 | 3.0 |
| Show | Show S3 | 3.0 |
| Show | Show: The Movie | -1.0 |
| Show | Show 2nd Season | 2.0 |
| Show | Show Hi10 s02 | 2.0 (`basic` finds `s02` first; capital-H `unwanted` branch cannot fire) |
| Show | Show s.2 | 2.0 |
| Show | Show season 1.5 | 1.5 |
| Show | Show 1920x1080 Season 4 | 4.0 |
| X | X S2 | 2.0 |
| X | Season 2.5 | 2.5 |
| X | X (2019) | -1.0 |
| X | X 2nd Season 1080p | 2.0 |
| X | X Part 2 | 2.0 |

## 6. Known defects to preserve or decide (see p1-topic-quirks-and-decisions.md)

- Q5: `SeasonRecognition.unwanted` contains `Hi10` with a capital `H` but the input is lower-cased (the episode version correctly has `hi10`).
- Q6: names containing dots between words (`Boku.no.Hero.Academia.S02.1080p-ITH`) keep the title and yield fractional garbage (2.108).
- Q7: `1080p` as the only number gives 1080; no resolution stripping happens with a single match.
- Q8 (recognition numbering; not the SeasonSorter quirk Q8 of p1-topic-quirks-and-decisions.md): ordinals (`2nd`) rely on group3 letters being ignored: `2nd` gives 2.0 by accident.
- Default decision: bug-for-bug parity because stored numbers are compared across syncs (a changed parse changes `chapter_number`, which `ShouldUpdateDbChapter` sees as an update and which flips duplicate-read matching for existing libraries).
