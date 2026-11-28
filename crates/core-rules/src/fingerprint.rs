//! What names a finding across edits of its file: the rule, the trimmed text of the finding's first
//! line, and its occurrence among the findings of that rule on equal lines. Moving or reindenting
//! the line keeps it; editing the line changes it. Fix ids hash it ([`crate::fix_ids`]), and
//! `--new` compares the findings of a file with those of its text at a git revision by it.
//!
//! concept: none (infrastructure: finding fingerprints)

use crate::diagnostic::Diagnostic;
use crate::lines::LineIndex;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fingerprint<'a> {
    pub rule: &'a str,
    /// The finding's first line without leading and trailing whitespace.
    pub line: &'a str,
    /// How many findings of the rule on an equal line come before this one.
    pub occurrence: u32,
}

/// The fingerprints of the findings of a file over `text`, in their order.
pub fn fingerprints<'a>(
    text: &'a str,
    lines: &LineIndex,
    diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
) -> Vec<Fingerprint<'a>> {
    let mut seen: HashMap<(&str, &str), u32> = HashMap::new();
    diagnostics
        .into_iter()
        .map(|d| {
            let (start, end) = lines.line_range(text, lines.line(d.range.start()));
            let line = text[start..end].trim();
            let occurrence = seen.entry((d.rule.as_str(), line)).or_default();
            let n = *occurrence;
            *occurrence += 1;
            Fingerprint {
                rule: &d.rule,
                line,
                occurrence: n,
            }
        })
        .collect()
}

/// Which findings of a file (`now`, over `text`) are new against the file at a base revision:
/// `base` is its text and findings there, `None` when the file is not in it (every finding is new).
/// An unchanged text has no new finding (also none from rules that judge several files); else a
/// finding is new when no finding at the base has its fingerprint.
pub fn new_findings<'a>(
    text: &'a str,
    now: impl IntoIterator<Item = &'a Diagnostic>,
    base: Option<(&str, &[Diagnostic])>,
) -> Vec<bool> {
    let now = now.into_iter();
    let Some((base_text, base)) = base else {
        return now.map(|_| true).collect();
    };
    if base_text == text {
        return now.map(|_| false).collect();
    }
    let old: HashSet<Fingerprint> = fingerprints(base_text, &LineIndex::new(base_text), base)
        .into_iter()
        .collect();
    fingerprints(text, &LineIndex::new(text), now)
        .iter()
        .map(|f| !old.contains(f))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Category, Severity};
    use core_syntax::{TextRange, TextSize};

    fn finding(rule: &str, at: u32) -> Diagnostic {
        Diagnostic {
            rule: rule.into(),
            category: Category::Style,
            severity: Severity::Warning,
            message: String::new(),
            range: TextRange::new(TextSize::from(at), TextSize::from(at + 1)),
            related: Vec::new(),
            fixes: Vec::new(),
        }
    }

    /// Findings of rule `r` at the start of each line that contains `yes`.
    fn findings(text: &str) -> Vec<Diagnostic> {
        let mut at = 0;
        let mut out = Vec::new();
        for line in text.split_inclusive('\n') {
            if line.contains("yes") {
                out.push(finding("r", at));
            }
            at += line.len() as u32;
        }
        out
    }

    fn new(base: Option<&str>, text: &str) -> Vec<bool> {
        let (now, old) = (findings(text), base.map(findings));
        new_findings(text, &now, base.zip(old.as_deref()))
    }

    #[test]
    fn an_unchanged_or_moved_finding_is_old_and_an_edited_line_is_new() {
        let base = "a: yes\nb: 1\n";
        assert_eq!(new(Some(base), base), [false]);
        // A line above and more indentation move the finding, not its fingerprint.
        assert_eq!(new(Some(base), "x: 1\n  a: yes\nb: 1\n"), [false]);
        assert_eq!(new(Some(base), "a: yes # c\nb: 1\n"), [true]);
        assert_eq!(new(Some(base), "a: yes\nb: yes\n"), [false, true]);
    }

    #[test]
    fn equal_lines_count_by_occurrence_and_an_absent_file_is_all_new() {
        let base = "a: yes\nb: 1\n";
        assert_eq!(new(Some(base), "a: yes\nb: 1\na: yes\n"), [false, true]);
        assert_eq!(new(None, base), [true]);
    }
}
