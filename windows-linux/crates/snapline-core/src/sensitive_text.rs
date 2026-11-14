//! Twin of `macOS/Sources/SnaplineCore/SensitiveText.swift`: finds sensitive values in one line of recognised text.

use std::ops::Range;

/// What kind of sensitive value a match is. Users turn each category on or off in Preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SensitiveCategory {
    Contact,
    Payment,
    Secret,
    Face,
}

impl SensitiveCategory {
    pub const ALL: [Self; 4] = [Self::Contact, Self::Payment, Self::Secret, Self::Face];

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Contact => "Contact info (emails, phone numbers)",
            Self::Payment => "Payment data (card numbers, IBANs)",
            Self::Secret => "Secrets (API keys, tokens, passwords)",
            Self::Face => "Faces",
        }
    }
}

/// A value to hide: a byte range into the text passed to [`matches`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SensitiveMatch {
    pub range: Range<usize>,
    pub category: SensitiveCategory,
}

/// Sensitive values found in one line of text, limited to `categories`, sorted and with overlaps merged.
#[must_use]
pub fn matches(text: &str, categories: &[SensitiveCategory]) -> Vec<SensitiveMatch> {
    let mut found = Vec::new();

    for rule in RULES.iter().filter(|r| categories.contains(&r.category)) {
        for raw in rule.pattern.find_group(text, rule.group) {
            let Some(mut hide) = rule.check.resolve(text, raw) else {
                continue;
            };
            if rule.category == SensitiveCategory::Secret {
                hide = extend_to_word_end(text, hide);
            }
            found.push(SensitiveMatch {
                range: hide,
                category: rule.category,
            });
        }
    }

    if categories.contains(&SensitiveCategory::Contact) {
        found.extend(phone_numbers(text).map(|range| SensitiveMatch {
            range,
            category: SensitiveCategory::Contact,
        }));
    }

    merge(found)
}

// MARK: - Rules

struct Rule {
    pattern: Pattern,
    category: SensitiveCategory,
    /// Capture group to hide; 0 is the whole match.
    group: usize,
    check: Check,
}

/// What to do with a raw match before hiding it.
enum Check {
    Keep,
    /// Drop the match unless the matched text passes.
    Valid(fn(&str) -> bool),
    /// Trim trailing groups until the text is a valid IBAN.
    Iban,
}

impl Check {
    fn resolve(&self, text: &str, r: Range<usize>) -> Option<Range<usize>> {
        match self {
            Self::Keep => Some(r),
            Self::Valid(valid) => valid(&text[r.clone()]).then_some(r),
            Self::Iban => iban_match(text, r),
        }
    }
}

/// The `regex` crate is fast but has no lookbehind or lookahead; patterns that need them use `fancy-regex`.
enum Pattern {
    Plain(regex::Regex),
    Fancy(fancy_regex::Regex),
}

impl Pattern {
    fn new(pattern: &str) -> Self {
        if ["(?<!", "(?<=", "(?!", "(?="]
            .iter()
            .any(|look| pattern.contains(look))
        {
            Self::Fancy(fancy_regex::Regex::new(pattern).expect("valid pattern"))
        } else {
            Self::Plain(regex::Regex::new(pattern).expect("valid pattern"))
        }
    }

    /// Byte ranges of capture group `group` in every non-overlapping match, left to right.
    fn find_group(&self, text: &str, group: usize) -> Vec<Range<usize>> {
        match self {
            Self::Plain(re) => re
                .captures_iter(text)
                .filter_map(|c| c.get(group))
                .map(|m| m.range())
                .collect(),
            // A backtracking limit error means no usable match, as NSRegularExpression gives up on such input too.
            Self::Fancy(re) => re
                .captures_iter(text)
                .flatten()
                .filter_map(|c| c.get(group))
                .map(|m| m.range())
                .collect(),
        }
    }
}

fn rule(pattern: &str, category: SensitiveCategory) -> Rule {
    Rule {
        pattern: Pattern::new(pattern),
        category,
        group: 0,
        check: Check::Keep,
    }
}

static RULES: std::sync::LazyLock<Vec<Rule>> = std::sync::LazyLock::new(|| {
    use SensitiveCategory::{Contact, Payment, Secret};
    vec![
        rule(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", Contact),
        Rule {
            check: Check::Valid(is_card_number),
            ..rule(r"(?<!\d)(?:\d[ -]?){12,18}\d(?!\d)", Payment)
        },
        Rule {
            check: Check::Iban,
            ..rule(r"\b[A-Z]{2}\d{2}(?: ?[A-Z0-9]){11,30}\b", Payment)
        },
        rule(
            r"eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*",
            Secret,
        ),
        rule(r"-----BEGIN [A-Z ]*PRIVATE KEY-----", Secret),
        rule(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b", Secret),
        rule(r"AIza[0-9A-Za-z_-]{35}", Secret),
        rule(
            r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})",
            Secret,
        ),
        rule(r"\bxox[abprs]-[A-Za-z0-9-]{10,}", Secret),
        rule(r"\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{16,}", Secret),
        Rule {
            group: 1,
            ..rule(r"(?i)\bbearer\s+([A-Za-z0-9._~+/-]{16,}=*)", Secret)
        },
        Rule {
            group: 1,
            ..rule(
                r#"(?i)\b[A-Za-z_]*(?:password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key|client[_-]?secret)\b\s*[:=]\s*"?([^\s"',;]+)"#,
                Secret,
            )
        },
        // "=" only as base64 padding at the end, so `NAME=value` is not read as one token.
        Rule {
            check: Check::Valid(looks_random),
            ..rule(
                r"(?<![A-Za-z0-9+/_-])[A-Za-z0-9+/_-]{32,}={0,2}(?![A-Za-z0-9+/_=-])",
                Secret,
            )
        },
    ]
});

// MARK: - Phone numbers

/// Phone-shaped runs: an optional `+country` and `(area)`, then digit groups split by single spaces, dashes or dots,
/// then an optional extension (`x123`, `ext. 123`). Not touching letters, other digits, or `.`/`-`/`/` followed by a
/// digit (so dates, versions and IPs end early).
static PHONE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(
        r"(?<![\w.+\-/])(?:\+\d{1,3}[ .\-]?)?(?:\(\d{1,4}\)[ .\-]?)?\d{1,12}(?:[ .\-]\d{1,12})*(?:\s?(?:x|ext\.?)\s?\d{1,5})?(?!\w|[.\-/]\d)",
    )
    .expect("valid pattern")
});

/// Stands in for macOS's `NSDataDetector`, which Rust has no equal for. Tuned against `fixtures/sensitive-text.json`.
fn phone_numbers(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    PHONE
        .find_iter(text)
        .flatten()
        .map(|m| m.range())
        .filter(|r| is_phone_number(&text[r.clone()]))
}

fn is_phone_number(s: &str) -> bool {
    let s = s.split(['x', 'e']).next().unwrap_or(s); // the extension does not count
    let groups: Vec<usize> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|g| !g.is_empty())
        .map(str::len)
        .collect();
    let digits: usize = groups.iter().sum();
    let Some(&last) = groups.last() else {
        return false;
    };
    if !(7..=15).contains(&digits) {
        return false;
    }
    if groups.len() == 1 {
        // 081234567890 or +6281234567890; a shorter lone run is an order number or a date (20261028)
        return s.starts_with('+') || digits >= 10;
    }
    if last < 3 {
        return false; // 2026-10-28
    }
    if groups.len() == 3 && groups.iter().filter(|&&g| g <= 2).count() >= 2 {
        return false; // 28-10-2026
    }
    if s.contains('.') {
        return last == 4; // 415.555.0132, but not 1.250.000 or 192.168.100.200
    }
    true
}

// MARK: - Validators

fn is_card_number(s: &str) -> bool {
    let digits: Vec<u32> = s.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&digits.len()) {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 0 {
                d
            } else {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn is_iban(s: &str) -> bool {
    let compact: Vec<char> = s.chars().filter(|&c| c != ' ').collect();
    if !(15..=34).contains(&compact.len()) {
        return false;
    }
    let mut remainder = 0u32;
    for ch in compact[4..].iter().chain(&compact[..4]) {
        let value = match ch {
            '0'..='9' => *ch as u32 - '0' as u32,
            'A'..='Z' => *ch as u32 - 55,
            _ => return false,
        };
        for digit in value.to_string().chars() {
            remainder = (remainder * 10 + digit.to_digit(10).unwrap_or(0)) % 97;
        }
    }
    remainder == 1
}

/// The IBAN pattern is greedy and can swallow a following capitalised word; drop trailing groups until valid.
fn iban_match(text: &str, r: Range<usize>) -> Option<Range<usize>> {
    let mut candidate = &text[r.clone()];
    loop {
        if is_iban(candidate) {
            return Some(r.start..r.start + candidate.len());
        }
        candidate = &candidate[..candidate.rfind(' ')?];
    }
}

/// Long tokens with letters and digits and high Shannon entropy look like generated keys.
fn looks_random(s: &str) -> bool {
    if !s.chars().any(char::is_alphabetic) || !s.chars().any(char::is_numeric) {
        return false;
    }
    let mut counts = std::collections::HashMap::new();
    for c in s.chars() {
        *counts.entry(c).or_insert(0u32) += 1;
    }
    let n = f64::from(u32::try_from(s.chars().count()).unwrap_or(u32::MAX));
    let entropy: f64 = counts
        .values()
        .map(|&c| {
            let p = f64::from(c) / n;
            -p * p.log2()
        })
        .sum();
    entropy >= 3.5
}

/// Text recognition sometimes misreads one character inside a long token (an "I" as "|"),
/// which would end the match early. A secret therefore runs to the end of its word.
/// It is not extended to the left, so a label like `API_KEY=` stays readable.
fn extend_to_word_end(text: &str, r: Range<usize>) -> Range<usize> {
    let end = text[r.end..]
        .find(char::is_whitespace)
        .map_or(text.len(), |i| r.end + i);
    r.start..end
}

/// Sorted by start; overlapping matches become one, keeping the first match's category.
fn merge(mut items: Vec<SensitiveMatch>) -> Vec<SensitiveMatch> {
    items.sort_by_key(|m| m.range.start);
    let mut out: Vec<SensitiveMatch> = Vec::new();
    for m in items {
        match out.last_mut() {
            Some(last) if m.range.start < last.range.end && m.range.end > last.range.start => {
                last.range = last.range.start.min(m.range.start)..last.range.end.max(m.range.end);
            }
            _ => out.push(m),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hidden<'a>(text: &'a str, categories: &[SensitiveCategory]) -> Vec<&'a str> {
        matches(text, categories)
            .iter()
            .map(|m| &text[m.range.clone()])
            .collect()
    }

    #[test]
    fn category_filter() {
        let text = "mail me@x.io key AKIAIOSFODNN7EXAMPLE";
        assert_eq!(hidden(text, &[SensitiveCategory::Contact]), ["me@x.io"]);
        assert_eq!(
            hidden(text, &[SensitiveCategory::Secret]),
            ["AKIAIOSFODNN7EXAMPLE"]
        );
        assert_eq!(hidden(text, &[]), [] as [&str; 0]);
    }

    #[test]
    fn overlapping_matches_are_merged() {
        // The JWT is also a high-entropy string; it must be reported once.
        let jwt =
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        assert_eq!(matches(jwt, &[SensitiveCategory::Secret]).len(), 1);
    }

    #[test]
    fn byte_ranges_survive_multibyte_text_before_the_match() {
        let text = "⌘ é 😀 mail me@x.io";
        assert_eq!(hidden(text, &[SensitiveCategory::Contact]), ["me@x.io"]);
    }
}
