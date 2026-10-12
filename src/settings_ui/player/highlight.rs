//! The syntax highlighter of the conf dialogs (task T-48a-039): the conf rule set of the Kotlin app, in the
//! same order and with the same colour per rule (`plans/48-settings-player-and-scripts/p1-topic-mpv-editor-and-
//! highlight.md` sections 4.1 and 4.2). The patterns need look-around and back-references, so they run on
//! `fancy-regex`. Java `\w`, `\d` and `\s` are written as explicit ASCII classes, so non-ASCII text colours as in the
//! Kotlin app. A text longer than [`MAX_CHARS`] is not highlighted (edge E-CE-02).
//!
//! The spans are byte ranges. A full rule colours every group that matched, including group 0; a groups rule colours
//! its capture groups, one colour each. A string rule records its ranges, and a later rule whose match starts inside
//! one of them is dropped, so a comment marker inside a string is not coloured as a comment.
use std::sync::OnceLock;

use fancy_regex::Regex;

/// Longer texts are not highlighted.
pub const MAX_CHARS: usize = 15_000;

/// The GitHub-like palette of the Kotlin app (`githubTheme`).
pub mod color {
    pub const KEYWORD: &str = "#FF7B72";
    pub const LITERAL: &str = "#79C0FF";
    pub const COMMENT: &str = "#9198A1";
    pub const STRING: &str = "#A5D6FF";
    pub const DEFINITION: &str = "#D2A8FF";
    pub const KEY: &str = "#7EE787";
    pub const VALUE: &str = "#D29922";
}

/// How a rule colours its matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    /// Every matched group, group 0 included, takes this colour.
    Full(&'static str),
    /// Capture group `n` (1-based) takes `colors[n - 1]`; group 0 is not coloured.
    Groups(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub pattern: &'static str,
    pub paint: Paint,
    /// Matches of this rule are strings: later rules do not colour inside them.
    pub is_string: bool,
}

/// One coloured range of the text, as byte offsets (`start..end`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub color: &'static str,
}

/// A rule set with its compiled patterns, compiled on first use.
pub struct RuleSet {
    rules: &'static [Rule],
    compiled: OnceLock<Vec<Regex>>,
}

/// The conf rules, in the order of the Kotlin app. The string rule is the fifth.
pub static CONF: RuleSet = RuleSet { rules: &CONF_RULES, compiled: OnceLock::new() };

const CONF_RULES: [Rule; 6] = [
    Rule {
        pattern: r"(?m)^[ \t\n\x0B\f\r]*[A-Za-z0-9_.-]+(?=[ \t\n\x0B\f\r]*=)",
        paint: Paint::Full(color::LITERAL),
        is_string: false,
    },
    Rule {
        pattern: r"(?<==)[ \t]*[A-Za-z0-9_ ~,./;:-]+",
        paint: Paint::Full(color::LITERAL),
        is_string: false,
    },
    Rule {
        pattern: r"(?<![A-Za-z0-9_-])(yes|no)(?![A-Za-z0-9_-])",
        paint: Paint::Groups(&[color::VALUE]),
        is_string: false,
    },
    Rule {
        pattern: r"\b[0-9]+(\.[0-9]+)?\b",
        paint: Paint::Full(color::VALUE),
        is_string: false,
    },
    Rule {
        pattern: r#"(?<!#[^\n]{0,120})("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')"#,
        paint: Paint::Full(color::KEY),
        is_string: true,
    },
    Rule {
        pattern: r"#.*",
        paint: Paint::Full(color::COMMENT),
        is_string: false,
    },
];

/// The conf spans of `text`.
pub fn conf(text: &str) -> Vec<Span> {
    highlight(&CONF, text)
}

/// Runs every rule of `set` over `text`, in order, and returns the spans in the order they were found.
pub fn highlight(set: &RuleSet, text: &str) -> Vec<Span> {
    if text.chars().count() > MAX_CHARS {
        return Vec::new();
    }
    let regexes = set.compiled.get_or_init(|| set.rules.iter().map(|rule| compile(rule.pattern)).collect());
    let mut spans = Vec::new();
    let mut strings: Vec<(usize, usize)> = Vec::new();
    for (rule, regex) in set.rules.iter().zip(regexes) {
        for found in regex.captures_iter(text) {
            let Ok(captures) = found else { continue };
            for index in 0..captures.len() {
                let Some(group) = captures.get(index) else { continue };
                let color = match rule.paint {
                    Paint::Full(color) => color,
                    Paint::Groups(_) if index == 0 => continue,
                    Paint::Groups(colors) => match colors.get(index - 1) {
                        Some(color) => color,
                        None => continue,
                    },
                };
                if strings.iter().any(|&(start, end)| (start..end).contains(&group.start())) {
                    continue;
                }
                spans.push(Span { start: group.start(), end: group.end(), color });
                if rule.is_string {
                    strings.push((group.start(), group.end()));
                }
            }
        }
    }
    spans
}

fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|err| panic!("a highlighter rule does not compile: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(source: &str, span: &Span) -> String {
        source[span.start..span.end].to_owned()
    }

    fn has(spans: &[Span], source: &str, text: &str, color: &str) -> bool {
        spans.iter().any(|s| s.color == color && text_of(source, s) == text)
    }

    #[test]
    fn every_conf_rule_compiles() {
        for rule in CONF_RULES.iter() {
            let _ = compile(rule.pattern);
        }
    }

    #[test]
    fn conf_keys_are_literals_and_values_follow_the_equals_sign() {
        let source = "hwdec=auto";
        let spans = conf(source);
        assert!(has(&spans, source, "hwdec", color::LITERAL));
        assert!(has(&spans, source, "auto", color::LITERAL));
    }

    #[test]
    fn yes_and_no_are_values_and_numbers_are_values() {
        let source = "deband=yes\nvolume-max=130";
        let spans = conf(source);
        assert!(has(&spans, source, "yes", color::VALUE));
        assert!(has(&spans, source, "130", color::VALUE));
    }

    #[test]
    fn a_conf_comment_runs_to_the_end_of_its_line() {
        let source = "# note\nkey=1";
        let spans = conf(source);
        assert!(has(&spans, source, "# note", color::COMMENT));
    }

    #[test]
    fn a_text_over_the_limit_is_not_highlighted() {
        let long = "x".repeat(MAX_CHARS + 1);
        assert!(conf(&long).is_empty());
        assert!(!conf(&"1".repeat(MAX_CHARS)).is_empty(), "a text of exactly the limit is highlighted");
    }
}
