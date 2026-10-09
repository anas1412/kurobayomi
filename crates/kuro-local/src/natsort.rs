//! `natural_cmp_ci` and `natural_cmp`: port of `net.greypanther.natsort` (area:14 section 4.5, topic-natural-sort).

use std::cmp::Ordering;

/// Kotlin `compareToCaseInsensitiveNaturalOrder`: `CaseInsensitiveSimpleNaturalComparator` over UTF-16 units.
pub fn natural_cmp_ci(a: &str, b: &str) -> Ordering {
    cmp_units(
        &a.encode_utf16().collect::<Vec<_>>(),
        &b.encode_utf16().collect::<Vec<_>>(),
        true,
    )
}

/// `SimpleNaturalComparator`, case-sensitive. Only the fixture test uses it; the source compares with `natural_cmp_ci`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    cmp_units(
        &a.encode_utf16().collect::<Vec<_>>(),
        &b.encode_utf16().collect::<Vec<_>>(),
        false,
    )
}

fn cmp_units(s1: &[u16], s2: &[u16], case_insensitive: bool) -> Ordering {
    let (mut i, mut j) = (0, 0);
    while i < s1.len() && j < s2.len() {
        let (c1, c2) = (s1[i], s2[j]);
        let (d1, d2) = (is_digit(c1), is_digit(c2));
        match (d1, d2) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {
                let (x, y) = if case_insensitive {
                    (lower(c1), lower(c2))
                } else {
                    (c1, c2)
                };
                if x != y {
                    return x.cmp(&y);
                }
                i += 1;
                j += 1;
            }
            (true, true) => {
                let (n1, next1) = digit_run(s1, i);
                let (n2, next2) = digit_run(s2, j);
                if n1 != n2 {
                    return n1.cmp(&n2);
                }
                i = next1;
                j = next2;
            }
        }
    }
    if i < s1.len() {
        Ordering::Greater
    } else if j < s2.len() {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

fn is_digit(unit: u16) -> bool {
    (u16::from(b'0')..=u16::from(b'9')).contains(&unit)
}

/// Value of the digit run starting at `start`, and the index just past it. Wrapping `u64`, as the Java code.
fn digit_run(units: &[u16], start: usize) -> (u64, usize) {
    let mut value: u64 = 0;
    let mut i = start;
    while let Some(&unit) = units.get(i).filter(|&&u| is_digit(u)) {
        value = value.wrapping_mul(10).wrapping_add(u64::from(unit - u16::from(b'0')));
        i += 1;
    }
    (value, i)
}

/// `Character.toLowerCase(char)`: one-to-one mappings only. Surrogates and multi-char mappings stay unchanged.
fn lower(unit: u16) -> u16 {
    let Some(c) = char::from_u32(u32::from(unit)) else {
        return unit;
    };
    let mut lowered = c.to_lowercase();
    match (lowered.next(), lowered.next()) {
        (Some(l), None) if u32::from(l) <= 0xFFFF => l as u16,
        _ => unit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Ordering::{Equal, Greater, Less};

    /// The 13 default cases of topic-natural-sort section 3.2 (plus the case-insensitive `a = A`).
    #[test]
    fn the_upstream_cases_hold() {
        let cases: [(&str, Ordering, &str); 13] = [
            ("z2.doc", Less, "z10.doc"),
            ("z010.doc", Equal, "z10.doc"),
            ("z10", Less, "z10a"),
            ("z10a", Greater, "z10"),
            ("a", Less, "z"),
            ("10", Less, "z"),
            ("z", Greater, "10"),
            ("10", Less, "20"),
            ("10", Greater, "5"),
            ("10", Equal, "10"),
            ("9223372036854775806", Less, "9223372036854775807"),
            ("18446744073709551614", Less, "18446744073709551615"),
            ("a", Equal, "A"),
        ];
        for (a, want, b) in cases {
            assert_eq!(natural_cmp_ci(a, b), want, "{a} vs {b}");
        }
    }

    #[test]
    fn the_case_sensitive_variant_keeps_case_apart() {
        assert_ne!(natural_cmp("a", "A"), Equal);
        assert_eq!(natural_cmp("z010", "z10"), Equal);
    }

    #[test]
    fn a_digit_run_sorts_numerically_and_leading_zeros_do_not_matter() {
        let mut names = vec!["Ch 10", "Ch 9", "Ch 010", "Ch 2"];
        names.sort_by(|a, b| natural_cmp_ci(a, b));
        assert_eq!(names, ["Ch 2", "Ch 9", "Ch 10", "Ch 010"]);
    }
}
