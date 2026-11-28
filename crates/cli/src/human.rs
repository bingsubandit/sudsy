//! The human report, the one people and coding agents read: each finding with its source line,
//! its notes and its fixes (the edited line, or a short `-`/`+` pair), and the id `fix --apply`
//! takes for every fix that `fix` does not apply by itself. A report of more than
//! [`COLLAPSE_ABOVE`] findings is one line per rule. `concise` is one line per finding.

use crate::report::{FileDiagnostics, clean, position};
use core_rules::fix_ids::fix_ids;
use core_rules::{Diagnostic, Edit, FORMAT, Fix, LineIndex, Severity};
use std::collections::HashMap;
use std::fmt::Write;

/// Above this many findings the report is collapsed by rule. A full finding takes 2-4 lines, so
/// 100 findings are about 300 lines: read in one go, while the collapsed form of a bigger report
/// saved agents 21-38% of input tokens (agent-workflows.md, E8, 127 findings).
pub const COLLAPSE_ABOVE: usize = 100;
/// Lines shown of each side of a multi-line change.
const SHOWN_LINES: usize = 3;
/// A longer line is cut to this many characters around the finding.
const LINE_CHARS: usize = 160;
/// Characters kept before the finding's column when a long line is cut.
const LINE_LEAD: usize = 40;
/// Locations listed for each rule of a collapsed report.
const FIRST_LOCATIONS: usize = 3;

pub fn human(files: &[FileDiagnostics], collapse: bool) -> String {
    let total: usize = files.iter().map(|f| f.diagnostics.len()).sum();
    if collapse && total > COLLAPSE_ABOVE {
        return collapsed(files, total);
    }
    let mut out = String::new();
    for file in files {
        let lines = LineIndex::new(&file.text);
        let ids = fix_ids(&file.path, &file.text, &lines, &file.diagnostics);
        for (d, ids) in file.diagnostics.iter().zip(ids) {
            header(&mut out, file, &lines, d);
            if d.rule != FORMAT {
                source_line(&mut out, file, &lines, d);
            }
            for r in &d.related {
                let at = position(file, &lines, r.range.start());
                let _ = writeln!(
                    out,
                    "    note: {} ({}:{})",
                    clean(&r.message),
                    at.line,
                    at.column
                );
            }
            for (fix, id) in d.fixes.iter().zip(ids) {
                let kind = fix.kind.as_str();
                let title = clean(&fix.title);
                let _ = match id {
                    Some(id) => writeln!(out, "    fix {id} ({kind}): {title}"),
                    None => writeln!(out, "    fix ({kind}): {title}"),
                };
                change(&mut out, &file.text, &lines, fix);
            }
        }
    }
    out
}

/// `path:line:col: severity[rule]: message` for each finding.
pub fn concise(files: &[FileDiagnostics]) -> String {
    let mut out = String::new();
    for file in files {
        let lines = LineIndex::new(&file.text);
        for d in &file.diagnostics {
            header(&mut out, file, &lines, d);
        }
    }
    out
}

fn header(out: &mut String, file: &FileDiagnostics, lines: &LineIndex, d: &Diagnostic) {
    let at = position(file, lines, d.range.start());
    let _ = writeln!(
        out,
        "{}:{}:{}: {}[{}]: {}",
        clean(&file.path),
        at.line,
        at.column,
        d.severity.as_str(),
        d.rule,
        clean(&d.message)
    );
}

/// `    12 | text of the line`, unless the line is blank.
fn source_line(out: &mut String, file: &FileDiagnostics, lines: &LineIndex, d: &Diagnostic) {
    let start = usize::from(d.range.start()).min(file.text.len());
    let line = lines.line(d.range.start());
    let (from, to) = lines.line_range(&file.text, line);
    let text = &file.text[from..to];
    if !text.trim().is_empty() {
        let _ = write!(out, "    {} | ", line + 1);
        excerpt(out, text, start - from);
    }
}

/// What a fix does to the lines it touches: the edited line when it changes one line, else the
/// old lines (`-`) and the new ones (`+`), at most [`SHOWN_LINES`] of each. Lines the edits leave
/// as they were are not shown.
fn change(out: &mut String, text: &str, lines: &LineIndex, fix: &Fix) {
    let mut edits: Vec<&Edit> = fix.edits.iter().collect();
    edits.sort_by_key(|e| (e.range.start(), e.range.end()));
    let (Some(first), Some(end)) = (edits.first(), edits.iter().map(|e| e.range.end()).max())
    else {
        return;
    };
    let first_line = lines.line(first.range.start());
    let last_line = lines.line(end);
    let from = lines.line_range(text, first_line).0;
    let to = if last_line + 1 < lines.line_count() {
        lines.line_range(text, last_line + 1).0
    } else {
        text.len()
    };
    let Some(old) = text.get(from..to) else {
        return;
    };
    let mut new = String::with_capacity(old.len());
    let mut at = from;
    for e in &edits {
        let (s, t) = (usize::from(e.range.start()), usize::from(e.range.end()));
        if let Some(kept) = text.get(at..s.max(at)) {
            new.push_str(kept);
        }
        new.push_str(&e.text);
        at = at.max(t);
    }
    new.push_str(text.get(at..to).unwrap_or(""));
    let (old, new) = (split(old), split(&new));
    let same_start = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let same_end = old[same_start..]
        .iter()
        .rev()
        .zip(new[same_start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old = &old[same_start..old.len() - same_end];
    let new = &new[same_start..new.len() - same_end];
    match (old, new) {
        ([], []) => {}
        ([_], [line]) => {
            let around = if same_start == 0 {
                usize::from(first.range.start()) - from
            } else {
                0
            };
            let number = first_line + same_start + 1;
            let _ = write!(out, "    {number} | ");
            excerpt(out, line, around);
        }
        _ => {
            for (sign, side) in [('-', old), ('+', new)] {
                for line in side.iter().take(SHOWN_LINES) {
                    let _ = write!(out, "    {sign} ");
                    excerpt(out, line, 0);
                }
                if side.len() > SHOWN_LINES {
                    let more = side.len() - SHOWN_LINES;
                    let _ = writeln!(out, "    {sign} ... ({more} more lines)");
                }
            }
        }
    }
}

/// The lines of `text`, without their breaks.
fn split(text: &str) -> Vec<&str> {
    text.split_inclusive('\n')
        .map(|l| l.trim_end_matches(['\n', '\r']))
        .collect()
}

/// Writes `line` and a line break, cut to [`LINE_CHARS`] characters around the byte offset
/// `around` when it is longer.
fn excerpt(out: &mut String, line: &str, around: usize) {
    let line = line.trim_end();
    if line.len() <= LINE_CHARS || line.chars().count() <= LINE_CHARS {
        out.push_str(&clean(line));
        out.push('\n');
        return;
    }
    let around = line.floor_char_boundary(around.min(line.len()));
    let skip = line[..around].chars().count().saturating_sub(LINE_LEAD);
    let mut cut: String = line.chars().skip(skip).take(LINE_CHARS).collect();
    if skip + LINE_CHARS < line.chars().count() {
        cut.push_str("...");
    }
    if skip > 0 {
        cut.insert_str(0, "...");
    }
    out.push_str(&clean(&cut));
    out.push('\n');
}

/// One line per rule: its count, how many have a fix and the first locations; errors first, then
/// the most frequent.
fn collapsed(files: &[FileDiagnostics], total: usize) -> String {
    struct Group<'a> {
        rule: &'a str,
        severity: Severity,
        count: usize,
        fixable: usize,
        first: Vec<String>,
    }
    let mut groups: Vec<Group> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for file in files {
        let mut lines = None;
        for d in &file.diagnostics {
            let i = *index.entry(&d.rule).or_insert_with(|| {
                groups.push(Group {
                    rule: &d.rule,
                    severity: d.severity,
                    count: 0,
                    fixable: 0,
                    first: Vec::new(),
                });
                groups.len() - 1
            });
            let g = &mut groups[i];
            g.count += 1;
            g.fixable += usize::from(!d.fixes.is_empty());
            g.severity = g.severity.max(d.severity);
            if g.first.len() < FIRST_LOCATIONS {
                let lines = lines.get_or_insert_with(|| LineIndex::new(&file.text));
                let at = position(file, lines, d.range.start());
                g.first
                    .push(format!("{}:{}:{}", clean(&file.path), at.line, at.column));
            }
        }
    }
    groups.sort_by(|a, b| (b.severity, b.count, a.rule).cmp(&(a.severity, a.count, b.rule)));
    let mut out = String::new();
    for g in &groups {
        let fixable = match g.fixable {
            0 => String::new(),
            n => format!(" ({n} with a fix)"),
        };
        let more = if g.count > g.first.len() { ", ..." } else { "" };
        let plural = if g.count == 1 { "" } else { "s" };
        let _ = writeln!(
            out,
            "{}[{}]: {} finding{plural}{fixable}: {}{more}",
            g.severity.as_str(),
            g.rule,
            g.count,
            g.first.join(", ")
        );
    }
    let _ = writeln!(
        out,
        "{total} findings of {} rule(s), collapsed above {COLLAPSE_ABOVE}: list one rule in full with \
         `--rule <id>` (lint, check), every finding with `--format concise`",
        groups.len()
    );
    out
}

#[cfg(test)]
mod tests {
    use super::excerpt;

    #[test]
    fn a_long_line_is_cut_around_the_finding() {
        let cut = |line: &str, around| {
            let mut out = String::new();
            excerpt(&mut out, line, around);
            out
        };
        let long = cut(&format!("{}X{}", "a".repeat(300), "b".repeat(300)), 300);
        assert!(long.starts_with("...") && long.ends_with("...\n"), "{long}");
        assert_eq!(long.find('X'), Some(3 + super::LINE_LEAD));
        assert_eq!(cut("short  ", 0), "short\n");
    }
}
