//! A block of import statements written the way isort writes it: sections (future, standard
//! library, third party, first party, relative), `import x` before `from x import y` in each,
//! paths in natural order, the members of a `from` import sorted by constants, classes, then
//! variables; `from` imports of one module are one statement (`as` aliases and `*` apart), and a
//! line over the width is wrapped in parentheses. A comment line above a statement stays with it,
//! after a blank line. The block is a run of statements with no code between them; the rule writes
//! it that way and compares it with the text.
//!
//! concept: sorted-imports
//! what: Import statements ordered by group, name and case, with blank lines between groups.
//! tools: isort, google-java-format, ktlint import-ordering, StyleCop SA1210, PHP-CS-Fixer ordered_imports, buf format
//! reads: family:imports, grammar:imports

use super::ImportLang;
use super::entry;
use super::order::{Compare, compare};
use crate::tree::significant_range;
use core_syntax::{Grammar, SyntaxNode, TextRange, TextSize};
use std::cmp::Ordering;
use std::path::Path;

/// The sections of an import block, in the order isort writes them by default.
pub const SECTIONS: &[&str] = &[
    "future",
    "standard-library",
    "third-party",
    "first-party",
    "local-folder",
];

/// What the block is written like.
pub struct Style<'a> {
    pub sections: &'a [String],
    /// `import x` and `from x import y` sort together, by module.
    pub force_sort_within_sections: bool,
    pub width: usize,
}

/// Is `top` a module of the project: a folder or file next to the file, in the project's root or
/// in its source folders?
fn first_party<L: ImportLang>(path: &Path, top: &str) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    let marked = |d: &Path| L::MARKERS.iter().any(|m| d.join(m).exists());
    let root = dir.ancestors().find(|d| marked(d)).unwrap_or(dir);
    let mut places = vec![root.to_path_buf(), dir.to_path_buf()];
    places.extend(L::SOURCE_DIRS.iter().map(|s| root.join(s)));
    places
        .iter()
        .any(|d| d.join(top).is_dir() || d.join(format!("{top}.{}", L::EXTENSION)).is_file())
}

fn section<L: ImportLang>(style: &Style<'_>, path: &Path, level: usize, module: &str) -> usize {
    let top = module.split('.').next().unwrap_or(module);
    let name = if level > 0 {
        "local-folder"
    } else if top == L::FUTURE {
        "future"
    } else if L::STDLIB.lines().any(|m| m == top) {
        "standard-library"
    } else if first_party::<L>(path, top) {
        "first-party"
    } else {
        "third-party"
    };
    style
        .sections
        .iter()
        .position(|s| s == name)
        .unwrap_or(style.sections.len())
}

/// One written statement (before the wrapping).
struct Line {
    section: usize,
    /// `from m import ...` rather than `import m`.
    from: bool,
    level: usize,
    module: String,
    /// The `from` members, or the `as` name of an `import`.
    members: Vec<(String, Option<String>)>,
    star: bool,
    /// A trailing comma in parentheses: the statement stays one member to a line.
    magic: bool,
    comment: Option<String>,
    /// Comment lines above the statement.
    atop: Vec<String>,
}

impl Line {
    fn plain(&self) -> bool {
        self.from && !self.star && self.members.iter().all(|(_, a)| a.is_none())
    }
}

fn member_rank(name: &str) -> u8 {
    let cased = name.chars().any(char::is_alphabetic);
    if name.chars().count() > 1 && cased && !name.chars().any(char::is_lowercase) {
        0
    } else if name.starts_with(char::is_uppercase) {
        1
    } else {
        2
    }
}

fn member_cmp(a: &(String, Option<String>), b: &(String, Option<String>)) -> Ordering {
    member_rank(&a.0)
        .cmp(&member_rank(&b.0))
        .then_with(|| compare(&a.0, &b.0, Compare::Natural))
        .then_with(|| a.1.cmp(&b.1))
}

fn line_cmp(a: &Line, b: &Line, style: &Style<'_>) -> Ordering {
    let by_style = |a: &Line, b: &Line| a.from.cmp(&b.from);
    a.section
        .cmp(&b.section)
        .then_with(|| {
            if style.force_sort_within_sections {
                Ordering::Equal
            } else {
                by_style(a, b)
            }
        })
        .then_with(|| b.level.cmp(&a.level))
        .then_with(|| compare(&a.module, &b.module, Compare::Natural))
        .then_with(|| by_style(a, b))
        .then_with(|| match (a.members.first(), b.members.first()) {
            (Some(x), Some(y)) => member_cmp(x, y),
            (x, y) => x.is_some().cmp(&y.is_some()),
        })
}

fn member_text((name, alias): &(String, Option<String>)) -> String {
    match alias {
        Some(a) => format!("{name} as {a}"),
        None => name.clone(),
    }
}

fn render(e: &Line, indent: &str, width: usize, out: &mut Vec<String>) {
    let comment = e.comment.as_deref().map(|c| format!("  {c}"));
    let tail = comment.clone().unwrap_or_default();
    if !e.from {
        let name = match e.members.first().and_then(|(_, a)| a.as_ref()) {
            Some(a) => format!("{} as {a}", e.module),
            None => e.module.clone(),
        };
        out.push(format!("{indent}import {name}{tail}"));
        return;
    }
    let head = format!("from {}{} import ", ".".repeat(e.level), e.module);
    if e.star {
        out.push(format!("{indent}{head}*{tail}"));
        return;
    }
    let members: Vec<String> = e.members.iter().map(member_text).collect();
    let line = format!("{indent}{head}{}", members.join(", "));
    if !e.magic && line.chars().count() <= width {
        out.push(format!("{line}{tail}"));
        return;
    }
    out.push(format!("{indent}{head}({}", comment.unwrap_or_default()));
    out.extend(members.iter().map(|m| format!("{indent}    {m},")));
    out.push(format!("{indent})"));
}

/// What a statement of the block says: its lines, or `Err(flag)` when the block need not be
/// written out (`true`: it needs changes whatever the rest says).
fn parse<L: ImportLang>(
    stmts: &[SyntaxNode],
    text: &str,
    path: &Path,
    g: &Grammar,
    style: &Style<'_>,
) -> Result<Vec<Line>, bool> {
    let marker = g.comments.line.as_deref().unwrap_or("#");
    let mut out: Vec<Line> = Vec::new();
    let mut before = None;
    for stmt in stmts {
        let range = significant_range(stmt, g);
        let atop: Vec<String> = before.map_or_else(Vec::new, |from: usize| {
            text[from..usize::from(range.start())]
                .split('\n')
                .skip(1)
                .map(str::trim)
                .filter(|l| l.starts_with(marker))
                .map(str::to_owned)
                .collect()
        });
        before = Some(usize::from(range.end()));
        let (start, end) = (usize::from(range.start()), usize::from(range.end()));
        if text[start..end].contains(marker) {
            return Err(false);
        }
        let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
        let rest = text[end..line_end].trim();
        let comment = if rest.starts_with(marker) {
            Some(rest.to_owned())
        } else if rest.is_empty() {
            None
        } else {
            return Err(true);
        };
        if comment.as_deref().is_some_and(|c| c.contains("isort:")) {
            return Err(false);
        }
        for e in entry::read(stmt, g) {
            let make = |members: Vec<(String, Option<String>)>, star: bool, magic: bool| Line {
                section: section::<L>(style, path, e.level, &e.path),
                from: e.from,
                level: e.level,
                module: e.path.clone(),
                members,
                star,
                magic,
                comment: comment.clone(),
                atop: atop.clone(),
            };
            if !e.from {
                out.push(make(vec![(e.path.clone(), e.alias.clone())], false, false));
                continue;
            }
            let star = e.members.is_empty() || e.members.iter().any(|(n, _)| n == "*");
            let (aliased, mut plain): (Vec<_>, Vec<_>) = e
                .members
                .iter()
                .filter(|(n, _)| n != "*")
                .cloned()
                .partition(|(_, a)| a.is_some());
            if star {
                out.push(make(Vec::new(), true, false));
            }
            for m in aliased {
                out.push(make(vec![m], false, false));
            }
            if !plain.is_empty() {
                plain.sort_by(member_cmp);
                out.push(make(plain, false, e.magic));
            }
        }
    }
    Ok(out)
}

/// The block as it should be written, or `Err(flag)` as in [`parse`].
fn written<L: ImportLang>(
    stmts: &[SyntaxNode],
    text: &str,
    path: &Path,
    g: &Grammar,
    style: &Style<'_>,
    indent: &str,
) -> Result<Vec<String>, bool> {
    let mut lines = parse::<L>(stmts, text, path, g, style)?;
    // Statements to combine, and imports written twice, need changes whatever else is right.
    for (i, e) in lines.iter().enumerate() {
        let same = lines[..i].iter().any(|o| {
            o.from == e.from
                && o.level == e.level
                && o.module == e.module
                && o.members == e.members
                && o.star == e.star
        });
        let combine = e.plain()
            && lines[..i]
                .iter()
                .any(|o| o.plain() && o.level == e.level && o.module == e.module);
        if same || combine {
            return Err(true);
        }
    }
    lines.sort_by(|a, b| line_cmp(a, b, style));
    let mut out = Vec::new();
    let mut last = None;
    for e in &lines {
        if last.is_some_and(|s| s != e.section) {
            out.push(String::new());
        }
        last = Some(e.section);
        if !e.atop.is_empty() {
            if out.last().is_some_and(|l: &String| !l.is_empty()) {
                out.push(String::new());
            }
            out.extend(e.atop.iter().map(|c| format!("{indent}{c}")));
        }
        render(e, indent, style.width, &mut out);
    }
    Ok(out)
}

/// The blank lines after a block at the top of the file: one, two before a definition.
fn blanks_wrong(next: &SyntaxNode, text: &str, line_end: usize, g: &Grammar) -> bool {
    let rest = &text[line_end..];
    let count = rest
        .split('\n')
        .skip(1)
        .take_while(|l| l.trim().is_empty())
        .count();
    let code = rest.split('\n').skip(1).find(|l| !l.trim().is_empty());
    // Comments between the block and a definition: either number is fine.
    let comment = code.is_some_and(|l| l.trim_start().starts_with(g.comments.line.as_deref().unwrap_or("#")));
    let two = g.has_tag(next.kind(), "function") || g.has_tag(next.kind(), "type-declaration");
    let ok = match (two, comment) {
        (true, true) => matches!(count, 1 | 2),
        (true, false) => count == 2,
        (false, _) => count == 1,
    };
    code.is_some() && !ok
}

/// The range of the block when it is not written the way isort writes it.
pub fn misplaced<L: ImportLang>(
    run: &[SyntaxNode],
    text: &str,
    path: &Path,
    g: &Grammar,
    style: &Style<'_>,
) -> Option<TextRange> {
    let (first, last) = (run.first()?, run.last()?);
    let start = usize::from(significant_range(first, g).start());
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[line_start..start];
    if !indent.chars().all(char::is_whitespace) {
        return None;
    }
    let end = usize::from(significant_range(last, g).end());
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
    if text[line_start..line_end].contains("isort:") {
        return None;
    }
    let wrong = match written::<L>(run, text, path, g, style, indent) {
        Err(flag) => flag,
        Ok(lines) => {
            let actual = text[line_start..line_end].lines().map(str::trim_end);
            let same = actual.eq(lines.iter().map(|l| l.trim_end()));
            let parent = first.parent()?;
            let at_top = parent.parent().is_none();
            let next = last.next_sibling();
            !same || (at_top && next.is_some_and(|nx| blanks_wrong(&nx, text, line_end, g)))
        }
    };
    wrong.then(|| {
        TextRange::new(
            TextSize::try_from(start).unwrap_or_default(),
            TextSize::try_from(end).unwrap_or_default(),
        )
    })
}
