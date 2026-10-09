# Topic: source id and stored-URL compatibility

Why this matters: Aniyomi stores each anime and episode under a `source` id (a `Long`) and a `url` that is relative to the site (`setUrlWithoutDomain`). Backups and existing databases hold these values. Kurobayomi must compute the same id and the same relative URL, or previously saved entries stop matching their source (the library shows them as orphans and re-fetch fails).

Sources:
- `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:101-105` (id), `:685-699` (`getUrlWithoutDomain`), `:665-678` (`setUrlWithoutDomain`).
- `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:87-91` and `:412` (manga twin, space replacement).

Test vectors were produced by running the Kotlin algorithm's Java equivalent (`java.net.URI`, OpenJDK 17) on this machine. The exact program is reproduced in section 4 so an implementer can regenerate them.

---

## 1. Source id

Algorithm (exact):
1. `key = name.lowercase() + "/" + lang + "/" + versionId` (`versionId` decimal).
2. `d = MD5(UTF-8(key))`, 16 bytes.
3. `id = OR_{i=0..7} ( d[i] << (8 * (7 - i)) )` as 64-bit, with `d[i]` unsigned.
4. `id = id AND 0x7FFFFFFFFFFFFFFF`.

Vectors:
| name | lang | versionId | key | id |
|---|---|---|---|---|
| Example | en | 1 | `example/en/1` | `4851548778462321757` (`0x435429e8ceba1c5d`) |
| Aniyomi Test | ja | 2 | `aniyomi test/ja/2` | `7915262807009061958` |

Note that `lowercase()` is applied to the name only, not to lang.

Rust: `generate_id` in `src/source/http.rs` (rust design section 3.7). Use `to_lowercase()`; Java `toLowerCase(Locale.ROOT)` and Rust `to_lowercase` agree on the default Unicode mappings used by source names. Add a test with `"İ"` (U+0130) and `"Σ"` at word end; if they differ, prefer the Java result by special-casing U+0130 to `"i\u{307}"` (Java's root-locale output).

---

## 2. Stored URL: `url without domain`

### 2.1 Kotlin algorithm (anime, OBSERVED)
```
fun getUrlWithoutDomain(orig: String): String = try {
    val uri = URI(orig)                      // java.net.URI, strict RFC 2396 parser
    var out = uri.path                       // DECODED path (getPath)
    if (uri.query != null) out += "?" + uri.query        // DECODED query
    if (uri.fragment != null) out += "#" + uri.fragment  // DECODED fragment
    out
} catch (_: URISyntaxException) { orig }
```
Manga twin (`HttpSource.kt:412`): `URI(orig.replace(" ", "%20"))` before the same steps, and `catch (e: URISyntaxException) { orig }`.

Java facts that matter (verified in section 3):
- `getPath()`, `getQuery()`, `getFragment()` return decoded text. `%2F` in a path becomes `/`; `%26` becomes `&`.
- `null` is returned for components that are absent. `getQuery()` returns `""` for a bare `?`, so `?` alone appends `?`.
- For opaque URIs (`scheme:` followed by something other than `/`, e.g. `mailto:x`, `x:y`), `getPath()` is `null`; the Kotlin `out` is then `null` (later an NPE or a stored literal `"null"`, depending on the caller). Rust treats this as "return the original" (Fix, E-19).
- Rejected inputs (`URISyntaxException`) return the original string unchanged, including the domain.

### 2.2 Characters Java's URI rejects (verified)
Probe: each character inserted as `https://x.org/a<c>b`.
- Rejected: space, `"`, `<`, `>`, `\`, `^`, `{`, `}`, `|`, backtick, `[`, `]`, `%` (when not followed by two hex digits), TAB, DEL, NUL, U+00A0 (no-break space).
- Accepted: `#`, `?`, `&`, `=`, `+`, `;`, `:`, `@`, `$`, `,`, `!`, `'`, `(`, `)`, `*`, `-`, `.`, `_`, `~`, `/`, and non-ASCII letters and symbols such as `é`, `ф`, `✓`.
- Position matters for two cases (verified): `[` and `]` are accepted in a query (`https://x.org/a?b=[1]` parses), and a relative path `/a[1]` parses, while `https://x.org/a[b` (absolute with path) is rejected. `|` is rejected in the fragment (`https://x.org/a#f|g`).
- Malformed escapes are rejected: `%zz`, `%2`. Well-formed escapes that decode to invalid UTF-8 are accepted and decoded with U+FFFD: `%E2%9C` gives `/�`.

Rust rule (Keep, to reproduce exactly): reject the input (return original) when any component contains a character from the rejected set in section 2.2, with the position exceptions above; the test table in section 3 is the specification where a position rule is not derivable. The implementation must pass every row of the table.

### 2.3 Rust algorithm (Proposal, Keep semantics)
1. `if !java_uri_accepts(orig) { return orig }`.
2. Split `orig` into scheme, authority, path, query, fragment using RFC 3986 delimiters (`#` first, then `?`).
3. If there is no scheme and no authority, the path is the whole part before `?`.
4. If the scheme is present and the rest does not start with `/` (opaque), return `orig` (Fix: Kotlin yields null).
5. `path = pct_decode_utf8_lossy(path_raw)`; `query = pct_decode_utf8_lossy(query_raw)` when `?` is present (even if empty); `fragment` likewise when `#` is present.
6. `out = path + ("?" + query if ? present) + ("#" + fragment if # present)`.
7. Empty path with authority (`https://x.org`) yields `""` (Keep). `https://x.org?` yields `?`. `https://x.org#frag` yields `#frag`.
8. `pct_decode` does not change `+`.

Crate choice: `percent-encoding` (`percent_decode_str(s).decode_utf8_lossy()`) for step 5. Do not use `url::Url::parse` for the path, because it normalises (lowercases host, removes dot segments, re-encodes) and would change results.

---

## 3. Vectors (generated, OpenJDK 17.0.20, Kotlin algorithm mirrored)

ANI = `AnimeHttpSource.getUrlWithoutDomain`. MANG = `HttpSource.getUrlWithoutDomain`.

| input | ANI output | MANG output |
|---|---|---|
| `https://aniyomi.org/anime/one-piece` | `/anime/one-piece` | `/anime/one-piece` |
| `https://aniyomi.org/anime/one-piece?page=2` | `/anime/one-piece?page=2` | `/anime/one-piece?page=2` |
| `https://aniyomi.org/a/b#frag` | `/a/b#frag` | `/a/b#frag` |
| `https://aniyomi.org/a%20b/c` | `/a b/c` | `/a b/c` |
| `https://aniyomi.org/a%26b?q=a%26b` | `/a&b?q=a&b` | `/a&b?q=a&b` |
| `https://aniyomi.org/a b/c` | `https://aniyomi.org/a b/c` (rejected, E-06) | `/a b/c` |
| `https://aniyomi.org/x\|y` | `https://aniyomi.org/x\|y` (rejected) | `https://aniyomi.org/x\|y` (rejected) |
| `https://aniyomi.org` | `` (empty) | `` (empty) |
| `https://aniyomi.org/` | `/` | `/` |
| `mailto:someone@example.com` | `null` (path null) | `null` |
| `/relative/path?x=1` | `/relative/path?x=1` | `/relative/path?x=1` |
| `https://aniyomi.org/café/ф` | `/café/ф` | `/café/ф` |
| `https://aniyomi.org/p?q=1+2` | `/p?q=1+2` | `/p?q=1+2` |
| `https://aniyomi.org/p?q=%E2%9C%93` | `/p?q=✓` | `/p?q=✓` |
| `https://aniyomi.org/p?a=1&b=2#x%20y` | `/p?a=1&b=2#x y` | `/p?a=1&b=2#x y` |
| `https://aniyomi.org/with"quote` | rejected: original | `https://aniyomi.org/with"quote` |
| `https://aniyomi.org/with^caret` | rejected: original | rejected: original |
| `https://aniyomi.org/with{brace}` | rejected: original | rejected: original |
| `https://aniyomi.org/with%zz` | rejected: original | rejected: original |
| `http://localhost:1/stream.m3u8?t=1` | `/stream.m3u8?t=1` | `/stream.m3u8?t=1` |
| `https://x.org/p?` | /p? | /p? |
| `https://x.org/p#` | /p# | /p# |
| `https://x.org?` | ? | ? |
| `https://x.org#frag` | #frag | #frag |
| `https://x.org/a?b=[1]` | /a?b=[1] | /a?b=[1] |
| `/a[1]` | /a[1] | /a[1] |
| `//host/path?q` | /path?q | /path?q |
| `http://u:p@x.org:8080/p?q#f` | /p?q#f | /p?q#f |
| `https://x.org/a%2Fb` | /a/b | /a/b |
| `https://x.org/%F0%9F%98%80` | /😀 | /😀 |
| `https://x.org/a+b` | /a+b | /a+b |
| `https://x.org/a?b=c d` | rejected: original | /a?b=c d |
| `x:y` | `null` | `null` |
| `https://x.org/a\u00a0b` (no-break space U+00A0, written as an escape) | rejected: original | rejected: original |
| `https://x.org/a%00b` | `/a` + NUL + `b` (decoded) | `/a` + NUL + `b` (decoded) |
| `https://x.org/a%E2%9C` | `/a` + U+FFFD (decoded, lossy) | `/a` + U+FFFD (decoded, lossy) |

All rows were produced by running the generator in section 4 on OpenJDK 17.0.20.

---

## 4. Generator

Program (kept outside the repo; reproduce with `javac` and `java`):

```java
import java.net.URI;
public class UrlVectors {
    static String anime(String orig) {
        try { URI uri = new URI(orig); String out = uri.getPath();
              if (uri.getQuery() != null) out += "?" + uri.getQuery();
              if (uri.getFragment() != null) out += "#" + uri.getFragment();
              return out; }
        catch (Exception e) { return orig; }
    }
    static String manga(String orig) {
        try { URI uri = new URI(orig.replace(" ", "%20")); String out = uri.getPath();
              if (uri.getQuery() != null) out += "?" + uri.getQuery();
              if (uri.getFragment() != null) out += "#" + uri.getFragment();
              return out; }
        catch (Exception e) { return orig; }
    }
    public static void main(String[] args) {
        for (String in : args) System.out.println(in + "  ANI=[" + anime(in) + "]  MANG=[" + manga(in) + "]");
    }
}
```

The Rust test suite stores the table as `tests/fixtures/url_vectors.tsv` (columns: input, expected_anime, expected_manga). The generator writes the same file; a diff between the two is a failing build.

---

## 5. Consequences for Kurobayomi

1. Matching an existing library entry: compute `url_without_domain` on the stored url the same way; the stored value is already relative, so the function is idempotent for relative inputs (`/a b` is returned as `/a b`).
2. Backups created by Aniyomi contain these relative urls; restore must not re-normalise them (area:22).
3. Changing the id algorithm would orphan every source. Do not change `generate_id` after release; treat it as a frozen contract.
