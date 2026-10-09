# Topic: natural sort and browse order

Scope: `core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:37-40` (the wrapper), the external algorithm `net.greypanther.natsort` (artifact `net.grey-panther:natural-comparator:1.1`), and every place the local source sorts. Behavior: `p1-behavior.md` §4.5, §6.3, §8.3. Rust port: `kuro-source-local/src/natsort.rs`.

## 1. Where it is used (exhaustive, local source only)
| Use | Kotlin | Comparator |
|---|---|---|
| Chapter tie-break by name | `MANGA_SRC:290-293` | `compareToCaseInsensitiveNaturalOrder`, descending (`c2.name.compare(c1.name)`) |
| Episode tie-break by name | `ANIME_SRC:284-287` | same, descending |
| Directory cover candidates | `MANGA_SRC:340-346` | ascending |
| Archive cover candidates | `MANGA_SRC:354-357` | ascending (case-insensitive) |
| Browse "Popular" (title) sort | `MANGA_SRC:107-111`, `ANIME_SRC:107-113` | NOT natural: `String.CASE_INSENSITIVE_ORDER` |
| Browse "Latest" (date) sort | `MANGA_SRC:113-119` | by `lastModified` (no name compare) |

Consequence: the title sort in the browse list is plain case-insensitive lexicographic, so `10` sorts before `9`. The chapter list is natural. Keep both; they are user-visible.

## 2. The comparator (exact algorithm)
Source: upstream `AbstractSimpleNaturalComparator.java`, `SimpleNaturalComparator.java`, `CaseInsensitiveSimpleNaturalComparator.java` in `gpanther/java-nat-sort` (read for this plan).

Pseudo-code over Java `char` (UTF-16 code units):
```
compare(s1, s2):
  i1 = i2 = 0
  while i1 < len1 and i2 < len2:
    c1 = s1[i1]; i1++
    c2 = s2[i2]; i2++
    d1 = is_digit(c1); d2 = is_digit(c2)
    if d1 and not d2: return -1           # digit sorts before non-digit at this position
    if not d1 and d2: return +1
    if not d1 and not d2:
        c = compare_chars(c1, c2)          # CI: lower(c1) - lower(c2); CS: c1 - c2
        if c != 0: return c
    else:                                  # both digits: consume the whole digit run of each
        n1 = c1 - '0'; while i1 < len1 and is_digit(s1[i1]): n1 = n1*10 + digit; i1++
        n2 = c2 - '0'; while i2 < len2 and is_digit(s2[i2]): n2 = n2*10 + digit; i2++
        if n1 != n2: return unsigned_compare(n1, n2)   # 64-bit, wraps on overflow
  if i1 < len1: return +1                  # s1 longer
  if i2 < len2: return -1                  # s2 longer
  return 0
```
Important: the digit-run value is accumulated over the whole run (not just the first digit), and equal values continue the outer loop with both indices already past the runs. Leading zeros therefore do not affect the outcome: `"z010"` equals `"z10"`.

## 3. Worked examples (from the algorithm, cross-checked against upstream test cases)
| Pair | Result | Why |
|---|---|---|
| `z2.doc` vs `z10.doc` | -1 | 2 < 10 at the digit run |
| `z010.doc` vs `z10.doc` | 0 | 10 = 10, then `.doc` equal |
| `z10` vs `z10a` | -1 | equal until end of s1; s2 has more |
| `10` vs `z` | -1 | digit before letter |
| `a` vs `A` (CI) | 0 | lowercase equal |
| `a` vs `A` (CS) | +1 (`'a'`=97 > `'A'`=65) | code unit difference |
| `9223372036854775806` vs `9223372036854775807` | -1 | fits in u64 |
| `18446744073709551614` vs `18446744073709551615` | -1 | fits in u64 |
| `99999999999999999999` (20 digits) | wraps | u64 wrap-around; deterministic but not numeric |
| `" 1"` vs `"1"` | +1 | space (non-digit) vs digit: space is non-digit, so `d1 = false, d2 = true` -> +1 |
| `"Ch 1"` vs `"Ch1"` | `' '` (32) vs `'1'`: non-digit vs digit -> +1 | the space sorts after digits |

## 4. Rust port contract
- Iterate `encode_utf16()` into a `Vec<u16>` (or work on the iterator with peekable) so surrogate pairs compare as two units, as in Java.
- `is_digit(u) = (b'0' as u16..=b'9' as u16).contains(&u)`.
- `java_lower(u)`: `Character.toLowerCase(char)` is a one-to-one mapping for BMP characters; for characters whose lowercase is more than one char, or for surrogates, return `u` unchanged. Implementation in `p1-rust-design.md` §4.5.
- Return `std::cmp::Ordering`. Only the sign is observable in Kotlin (`compareTo` results are compared with 0), so the magnitude of Java's difference need not be reproduced.
- The unsigned compare: `n1.cmp(&n2)` on `u64`.

## 5. Browse title sort (not natural)
`String.CASE_INSENSITIVE_ORDER` (Java): compares each `char` after `toUpperCase(c)` and, if different, after `toLowerCase`. Rust: compare `chars()` pairs by `to_uppercase()` then `to_lowercase()` for single-character mappings; for multi-character mappings compare the first mapped char. This is the same for ASCII and Latin scripts, which is the only range users sort by in practice. Test: `t_14_sort_title_is_not_natural` (`9`, `10`, `a` -> ascending `10`, `9`, `a`).

## 6. Stability and ties
- Kotlin `sortedWith` and `sortedBy` are stable. Rust `Vec::sort_by` is stable. Equal keys keep the input order.
- Input order differs: Kotlin uses filesystem order (undefined); Rust sorts the listing by name first. Thus for two entries that compare equal under the natural comparator (`z010` and `z10`), Rust's result is name-sorted order (`z010` < `z10` byte-wise, but they are equal under the comparator so the stable sort keeps `z010` first). This is documented as E-CHAPTERS-6 and is the only permitted source of difference.

## 7. Tests
- Port all upstream cases (`p1-edge-cases-and-tests.md` §3.2).
- Port upstream `InputOutputTest` with `alphanum_test_1_input.txt`/`_output.txt` and `alphanum_test_2_*`: sort the input with the case-sensitive comparator and compare to the output file.
- Chapter sort test: `Ch 1a`, `Ch 1b`, `Ch 1` with numbers 1.1, 1.2, 1.0 -> order `1.2`, `1.1`, `1.0`.

## 8. Ponytail note
A 40-line port beats a crate: the existing crates (`natord`, `alphanumeric-sort`) differ in leading-zero handling and in digit-vs-letter ordering, which would silently change chapter order for some titles. Ceiling: none. Upgrade path: none needed for parity.
