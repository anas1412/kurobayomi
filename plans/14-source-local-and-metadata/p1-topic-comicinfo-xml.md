# Topic: ComicInfo.xml — byte-level format, read and write rules

Scope: `core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt` (189 lines) plus the call sites in `source-local` and the downloader. Behavior is in `p1-behavior.md` §19; data is in `p1-data-and-config.md` §3.1, §3.2, §5. This file pins down the exact text contract for a Rust implementation and the open items that a golden test must settle.

## 1. Why the format matters beyond this app
ComicInfo v2.0 (https://anansi-project.github.io/docs/comicinfo/schemas/v2.0, cited at `COMIC:64`) is read by other comic managers. Files written by Kurobayomi must stay readable by them, and files written by Aniyomi must stay readable by Kurobayomi. The Tachiyomi-specific elements (`PublishingStatusTachiyomi`, `Categories`, `SourceAniyomi`) are extensions in the XML Schema namespace; other readers ignore unknown elements. Keep them.

## 2. Document skeleton (write side)
```
<?xml version="1.0" encoding="UTF-8"?>
<ComicInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Title>Chapter 12</Title>                       (optional, Kotlin writes only in the downloader)
  <Series>Foo</Series>                     (Kotlin: always in downloader and in local legacy migration)
  <Number>12</Number>                     (downloader only)
  <Summary>A story.</Summary>
  <Writer>A</Writer>
  <Penciller>B</Penciller>
  <Inker>C</Inker>                       (never written by Kotlin)
  <Colorist>D</Colorist>                 (never)
  <Letterer>E</Letterer>                 (never)
  <CoverArtist>F</CoverArtist>           (never)
  <Translator>Team</Translator>             (downloader: chapter scanlator)
  <Genre>Action, Drama</Genre>
  <Tags>Drama</Tags>                         (never)
  <Web>https://a.example/1</Web>                           (downloader only)
  <ty:PublishingStatusTachiyomi xmlns:ty="http://www.w3.org/2001/XMLSchema">Ongoing</ty:PublishingStatusTachiyomi>
  <ty:Categories xmlns:ty="http://www.w3.org/2001/XMLSchema">Read</ty:Categories>      (downloader only)
  <ay:SourceAniyomi xmlns:ay="http://www.w3.org/2001/XMLSchema">Source A</ay:SourceAniyomi> (downloader only)
</ComicInfo>
```
Source of element order: property declaration order in `COMIC:68-84` (`title, series, number, summary, writer, penciller, inker, colorist, letterer, coverArtist, translator, genre, tags, web, publishingStatus, categories, source`). Element names come from the nested classes' `@XmlSerialName` (`COMIC:95-161`).

Which writer produces which elements:
- Local source legacy migration (`SManga.getComicInfo()`, `COMIC:11-31`): Series, Summary, Writer, Penciller, Genre, PublishingStatusTachiyomi.
- Downloader (`Manga.kt:95-125` via `MangaDownloader.kt:666`): Title, Series, Number (when chapter >= 0), Web (joined by space), Summary, Writer, Penciller, Translator (chapter scanlator), Genre (joined `", "`), PublishingStatusTachiyomi, Categories (joined `", "`, when the list is non-null), SourceAniyomi (source display name).

Namespace declarations: the root carries `xmlns:xsd` and `xmlns:xsi`. The `ty` and `ay` prefixes are declared where used (expected; see §5, open item O-1).

## 3. Declaration and whitespace
- `XmlDeclMode.Charset` produces an XML declaration that names the encoding. The Kotlin app writes `UTF-8` bytes. Rust writes `<?xml version="1.0" encoding="UTF-8"?>`.
- `indent = 2`: two spaces per level, one element per line, no blank lines.
- A trailing newline at end of document: not established by the source (O-2).
- Text content is written inline: `<Series>Foo</Series>` (no line breaks inside leaf elements), because every element class is `@XmlValue(true)`.

## 4. Escaping and content rules (write)
- Element text is XML-escaped. The minimum set `&`, `<`, `>` is certain. Escaping of `"` and `'` inside element text is not established (O-3). Rust (quick-xml `BytesText::new`) escapes `& < > " '`; this is valid XML either way. The golden test (O-3) decides whether the upstream output differs.
- Control characters are not valid in XML 1.0; Kotlin does not filter them. A title with a `\u0001` produces invalid XML. Proposal: strip characters not allowed by XML 1.0 (`#x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] | [#x10000-#x10FFFF]`) before writing. Deviation, safe.
- Empty strings are written as empty elements (`<Series></Series>` or `<Series/>`; xmlutil's choice is O-4). Rust writes `<Series></Series>`; the reader accepts both.
- `null` fields are omitted (no element at all).

## 5. Open items (settled by the golden test, §8)
| ID | Question | Current assumption | Impact |
|---|---|---|---|
| O-1 | Are `xmlns:ty` / `xmlns:ay` emitted on each element or on the root? | on the element | byte diff only; readers OK either way |
| O-2 | Trailing newline after `</ComicInfo>`? | none (quick-xml default) and Rust appends `\n` | byte diff only |
| O-3 | Escaping of quotes in text | `& < >` only in assumption; Rust escapes `" '` too | byte diff only |
| O-4 | Empty element form | `<X></X>` | byte diff only |
| O-5 | Attribute quoting | double quotes | none (standard) |
| O-6 | Element name source for nested classes | class `@XmlSerialName` (e.g. `Series`), not property name | critical: if xmlutil used property names, output would be lowercase `series`. Tachiyomi-written files are used by other readers, so the class-name rule is the working assumption. The golden test is the proof. |

## 6. Read rules
1. Parse as UTF-8 (`AndroidXmlReader(stream, "UTF-8")`, `MANGA_SRC:236`). Consume a leading BOM.
2. Root local name must be `ComicInfo` (`COMIC:67`).
3. Children of the root: match local names. Namespace prefixes are ignored (E-CI-6 deviation). Unknown children are skipped (`ignoreUnknownChildren`, `AppModule.kt:181`).
4. Text: concatenate text and CDATA; unescape entities; self-closing element = `""` (Kotlin `@XmlValue` default).
5. Elements that appear more than once: last wins (E-CI-4).
6. Errors: malformed XML, wrong root, empty input -> error (E-CI-7/8).

Element -> field mapping on read is in `p1-data-and-config.md` §3.1 (column "Read by local source").

## 7. Mapping rules (copy into SManga) — restated for the implementer
See `p1-behavior.md` §19.7. The two non-obvious rules:
- Genre: `[genre, tags, categories]`, `distinct` on whole strings, each `trim`med, joined `", "`. No split.
- Artist: `[penciller, inker, colorist, letterer, coverArtist]`, each split on `", "`, `distinct`, `trim`, joined `", "`.
- Status: always assigned; absent element = 0.

## 8. Golden test procedure
1. In an Aniyomi checkout with a JDK: write a small Kotlin test or script that calls `XML { defaultPolicy { ignoreUnknownChildren() }; autoPolymorphic = true; xmlDeclMode = XmlDeclMode.Charset; indent = 2; xmlVersion = XmlVersion.XML10 }` (the configuration of `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:179-189`) and encodes `ComicInfo(...)` with the sample in `p1-edge-cases-and-tests.md` §4.1. Save as `kuro-core/tests/fixtures/comicinfo/golden_upstream.xml`.
2. Rust test `t_14_comicinfo_golden_upstream` compares `to_xml()` with that file after `trim_end()` on both sides, byte for byte otherwise.
3. If O-1..O-4 differ, record the upstream choice in the fixture and change the Rust writer to match. Do not keep a Rust-only variant.
4. Remove the `#[ignore]` on the golden test.

If JDK work is not possible, the fallback is to use the structure in §2 as the fixture and mark O-1..O-4 as "unverified" in the fixture header. The acceptance criterion of T-14a-030 is the golden run.

## 9. Ponytail note
The Rust writer is ~40 lines of explicit `write_text` calls rather than a serde derive with attributes. Reason: quick-xml's serde mapping cannot express per-field namespace prefixes without custom attributes for three fields, and the explicit form is the contract (data §5.3). Ceiling: adding a field means editing `to_xml` and `from_xml` by hand. Upgrade path: a small declarative table `[(name, field, ns)]` used by both writer and reader.
