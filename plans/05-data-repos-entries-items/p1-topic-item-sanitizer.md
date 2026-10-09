# Area 05 — topic: chapter/episode name sanitizer

Files: `data/src/main/java/tachiyomi/data/items/chapter/ChapterSanitizer.kt` (46 LOC) and `data/src/main/java/tachiyomi/data/items/episode/EpisodeSanitizer.kt` (46 LOC). The two objects are byte-for-byte equivalent except for the names `ChapterSanitizer`/`CHAPTER_TRIM_CHARS` versus `EpisodeSanitizer`/`EPISODE_TRIM_CHARS`; one Rust function serves both.

## 1. OBSERVED algorithm

```kotlin
fun String.sanitize(title: String): String {
    return trim()                 // 1. Kotlin String.trim(): strip leading/trailing chars where Char.isWhitespace()
        .removePrefix(title)      // 2. if the result startsWith(title) drop exactly that prefix once; else unchanged
        .trim(*CHAPTER_TRIM_CHARS)// 3. strip leading/trailing chars that are in the explicit set below, repeatedly
}
```

Step 3 set (`CHAPTER_TRIM_CHARS`, 30 array entries): `' '` (U+0020), U+0009, U+000A, U+000B, U+000C, U+000D, U+0020 (again), U+0085, U+00A0, U+1680, U+2000, U+2001, U+2002, U+2003, U+2004, U+2005, U+2006, U+2007, U+2008, U+2009, U+200A, U+2028, U+2029, U+202F, U+205F, U+3000, then the separators `'-'`, `'_'`, `','`, `':'`. The set has 29 distinct characters.

Step 1 set (Kotlin/JVM `Char.isWhitespace()` = `Character.isWhitespace(ch) || Character.isSpaceChar(ch)`): U+0009-U+000D, U+001C-U+001F, U+0020, U+00A0, U+1680, U+2000-U+200A, U+2028, U+2029, U+202F, U+205F, U+3000. Differences with Rust `char::is_whitespace` (Unicode White_Space): Rust includes U+0085 but not U+001C-U+001F. U+0085 is stripped by step 3 anyway; U+001C-U+001F are only stripped by Kotlin step 1.

Callers: `SyncChaptersWithSource.kt:68` (`sChapter.name.sanitize(manga.title)`) and `SyncEpisodesWithSource.kt:66` (`sEpisode.name.sanitize(anime.title)`) under `app/src/main/java/eu/kanade/domain/items/{chapter,episode}/interactor/`. The sanitized name replaces the source name BEFORE `prepareNewChapter`, chapter-number recognition (`ChapterRecognition.parseChapterNumber(manga.title, chapter.name, ...)`, area:07-domain-entries-items) and the DB comparison `ShouldUpdateDbChapter`, so the stored `name` is always the sanitized one.

## 2. Proposal (Rust)

```rust
// kuro_core::data::items::sanitize
pub fn sanitize_item_name(name: &str, title: &str) -> String {
    let s = trim_kotlin_whitespace(name);          // step 1
    let s = s.strip_prefix(title).unwrap_or(s);    // step 2: strip_prefix("") is a no-op, like removePrefix("")
    s.trim_matches(|c| TRIM_SET.contains(&c)).to_owned() // step 3
}
const TRIM_SET: [char; 29] = [ /* the 29 distinct characters of section 1 */ ];
fn is_kotlin_whitespace(c: char) -> bool { /* section 1 step 1 set, written out explicitly */ }
```

Use `trim_matches` (repeats at both ends, like Kotlin `trim(vararg chars)`). Do not use `str::trim` for step 1 (different set). Comparison in step 2 is by exact code points, case-sensitive, no Unicode normalisation (a source that returns "Ｏne Piece" in full-width characters will not match "One Piece").

## 3. Behaviour table (test vectors, all OBSERVED consequences of the algorithm)

| name | title | result | Why |
|---|---|---|---|
| `"One Piece 1000"` | `"One Piece"` | `"1000"` | prefix removed, space trimmed |
| `"One Piece - Chapter 12"` | `"One Piece"` | `"Chapter 12"` | prefix removed, then `" - "` trimmed |
| `"Chapter 5 -"` | `"X"` | `"Chapter 5"` | trailing separator trimmed |
| `"_-:Special,"` | `""` | `"Special"` | separators trimmed both ends; empty title is a no-op prefix |
| `"One Piece"` | `"One Piece"` | `""` | the whole name equals the title: empty result (callers store an empty name) |
| `"one piece 3"` | `"One Piece"` | `"one piece 3"` | prefix match is case-sensitive |
| `"  One Piece 3"` | `"One Piece"` | `"3"` | step 1 trims before the prefix check |
| `"One Piece  One Piece 3"` | `"One Piece"` | `"One Piece 3"` | exactly one prefix occurrence is removed; the second occurrence is kept |
| `"\u{00A0}Ch 1\u{3000}"` | `"X"` | `"Ch 1"` | NBSP and ideographic space trimmed |
| `"Ch. 1."` | `"X"` | `"Ch. 1."` | `'.'` is not in the set |
| `"\u{001F}One Piece 1"` | `"One Piece"` | `"1"` | step 1 (Kotlin) strips U+001F so the prefix still matches; a naive Rust `trim()` would not |
