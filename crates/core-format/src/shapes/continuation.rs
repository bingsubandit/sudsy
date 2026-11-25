//! Backslash (or backtick) line continuations and heredoc bodies: the layout of one logical line
//! that the source splits over several physical lines (Dockerfile instructions, Makefile
//! statements, shell here-documents).
//!
//! concept: line-continuation-layout
//! what: Line continuations kept at the end of lines.
//! tools: shfmt, hadolint, checkmake

use crate::doc::{self, Doc};

/// Does `text` start with `word` as a whole word (not followed by a word character)?
fn starts_word(text: &str, word: &str) -> bool {
    text.strip_prefix(word)
        .is_some_and(|r| !r.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
}

/// For each line of `text`: does it start inside a quote that an earlier line opened
/// (`"a \` + line break + `b"`)? The lines of a quote stay as written.
pub fn quote_starts(text: &str) -> Vec<bool> {
    let mut quote = None::<char>;
    let mut out = Vec::new();
    for line in text.split('\n') {
        out.push(quote.is_some());
        // (a comment line outside a quote opens none)
        if quote.is_none() && line.trim_start().starts_with('#') {
            continue;
        }
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            match quote {
                _ if c == '\\' && quote != Some('\'') => {
                    chars.next();
                }
                None if c == '"' || c == '\'' => quote = Some(c),
                Some(q) if c == q => quote = None,
                _ => {}
            }
        }
    }
    out
}

/// Columns of a tab when the indentation of continued lines is compared.
const TAB_COLUMNS: usize = 4;

/// One logical line: the parts up to the first continuation, then the indented rest.
#[derive(Default)]
pub struct Continued {
    head: Vec<Doc>,
    tail: Vec<Doc>,
    broken: bool,
}

impl Continued {
    /// Appends to the current physical line; right after [`Continued::end_line`] a new one starts.
    pub fn push(&mut self, d: Doc) {
        if self.broken {
            self.tail.push(doc::hardline());
            self.broken = false;
        }
        if self.tail.is_empty() {
            self.head.push(d);
        } else {
            self.tail.push(d);
        }
    }

    /// A blank between two parts, none at the start of a physical line.
    pub fn space(&mut self) {
        if !self.broken {
            self.push(doc::lit(" "));
        }
    }

    /// Ends the physical line with ` <marker>` (the escape character).
    pub fn end_line(&mut self, marker: &str) {
        self.push(doc::text(format!(" {marker}")));
        self.broken = true;
    }

    /// A line of its own in the indented part (a comment between continued lines).
    pub fn own_line(&mut self, d: Doc) {
        self.push(d);
        self.broken = true;
    }

    /// Text of several lines, a continuation ending each but the last: the first line goes on,
    /// every further line is indented and ends in ` \` (blanks before the escape character
    /// collapse), except inside a quote, where a line stays as written. A line that starts with
    /// `#` is a comment and never opens a quote. The further lines keep their indentation
    /// relative to the least indented of them (a tab counts [`TAB_COLUMNS`]): nested levels stay
    /// nested, the base level becomes the one level of the continuation.
    pub fn text(&mut self, text: &str) {
        let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
        let esc = match lines[0].trim_end().chars().last() {
            Some('`') => '`',
            _ => '\\',
        };
        let mut quote = None::<char>;
        // (verbatim, lead columns, body) of each line
        let mut parts = Vec::with_capacity(lines.len());
        for raw in &lines {
            let verbatim = quote.is_some();
            let line = if verbatim { raw } else { raw.trim_start() };
            let lead = if verbatim {
                0
            } else {
                raw[..raw.len() - line.len()]
                    .chars()
                    .map(|c| if c == '\t' { TAB_COLUMNS } else { 1 })
                    .sum::<usize>()
            };
            let comment = !verbatim && line.starts_with('#');
            if !comment {
                let mut chars = line.chars();
                while let Some(c) = chars.next() {
                    match quote {
                        _ if c == esc && quote != Some('\'') => {
                            chars.next();
                        }
                        None if c == '"' || c == '\'' => quote = Some(c),
                        Some(q) if c == q => quote = None,
                        _ => {}
                    }
                }
            }
            // A line that ends inside a quote keeps its blanks before the escape character.
            let body = if quote.is_some() {
                line.trim_end_matches([' ', '\t'])
            } else {
                line.trim_end()
            };
            let body = match body.strip_suffix(esc) {
                Some(before) if quote.is_none() && !comment => {
                    let before = before.trim_end();
                    if before.is_empty() {
                        esc.to_string()
                    } else {
                        format!("{before} {esc}")
                    }
                }
                _ => body.to_string(),
            };
            parts.push((
                verbatim,
                if body.is_empty() { usize::MAX } else { lead },
                body,
            ));
        }
        let base = parts
            .iter()
            .skip(1)
            .filter(|(verbatim, _, body)| !verbatim && !body.starts_with('#'))
            .map(|(_, lead, _)| *lead)
            .min()
            .filter(|&b| b != usize::MAX)
            .unwrap_or(0);
        for (i, (verbatim, lead, body)) in parts.into_iter().enumerate() {
            match (i, verbatim) {
                (0, _) => self.push(doc::text(body)),
                (_, true) => self.tail.push(doc::line_start(body)),
                _ => {
                    self.tail.push(doc::hardline());
                    let nested = if body.starts_with('#') || lead == usize::MAX {
                        0
                    } else {
                        lead.saturating_sub(base)
                    };
                    self.tail.push(if nested == 0 {
                        doc::text(body)
                    } else {
                        let mut deeper = " ".repeat(nested);
                        deeper.push_str(&body);
                        doc::text(deeper)
                    });
                }
            }
        }
    }

    /// Text of several lines as written: the first line goes on, the others stay where they are.
    pub fn raw(&mut self, text: &str) {
        for (i, line) in text.split('\n').enumerate() {
            let line = line.trim_end_matches('\r');
            if i == 0 {
                self.push(doc::text(line.to_string()));
            } else {
                self.tail.push(doc::line_start(line.to_string()));
            }
        }
    }

    /// Code of another language, laid out in lines by its own formatter (its nested levels indented
    /// by `unit`, a tab or that many blanks), as the rest of the logical line: the first line goes
    /// on, every further line is indented one continuation level and ends in ` \\`. Lines of the
    /// first statement (up to the first line at column 0 that does not close a block) stand one
    /// level less in, as the first line sets the level, unless the statement is a block closed at
    /// column 0; where a line starts with one of the `operators`, only from there on. Each of the
    /// `comments` (code lines before it, text) is put on a line of its own before that code line.
    pub fn code(
        &mut self,
        code: &str,
        unit: &str,
        operators: &[&str],
        closers: &[&str],
        comments: &[(usize, String)],
    ) {
        let lines: Vec<&str> = code.split('\n').collect();
        let inside = quote_starts(code);
        let is_op = |l: &str| operators.iter().any(|o| starts_word(l.trim_start(), o));
        let mut end = lines.len();
        let mut block = false;
        let mut first_op = None;
        for (i, line) in lines.iter().enumerate().skip(1) {
            if inside[i] {
                continue;
            }
            let body = line.trim_end();
            let bare = body.trim_start();
            if body.len() == bare.len() {
                if !closers.iter().any(|c| starts_word(bare, c)) {
                    end = i;
                    break;
                }
                block = true;
            } else if first_op.is_none() && is_op(body) {
                first_op = Some(i);
            }
        }
        // (a first line with several statements: the lines after it belong to a later one)
        let later = !inside.get(1).copied().unwrap_or(false)
            && lines[0].contains("; ")
            && !lines[0].contains(['"', '\'']);
        let strip_from = match (block, first_op) {
            _ if later => end,
            (true, _) => end,
            (false, Some(i)) => i,
            (false, None) => 1,
        };
        for (i, line) in lines.iter().enumerate() {
            let last = i + 1 == lines.len();
            if inside[i] {
                // (inside a quote: as written; the line break goes before it)
                let line = line.trim_end_matches('\r');
                // (the line that closes the quote goes on to the next line)
                let ends = !last && !inside[i + 1] && !line.ends_with('\\');
                self.tail.push(doc::line_start(if ends {
                    format!("{line} \\")
                } else {
                    line.to_string()
                }));
                continue;
            }
            let ends_in_quote = !last && inside[i + 1];
            let body = if ends_in_quote {
                line.trim_end_matches([' ', '\t'])
            } else {
                line.trim_end()
            };
            let text = if ends_in_quote || last || body.ends_with('\\') {
                body.to_string()
            } else if body.is_empty() {
                // (a blank continuation line stands level with the line after it)
                let next = (i + 1..lines.len())
                    .find(|&j| !inside[j] && !lines[j].trim().is_empty())
                    .map_or("", |j| lines[j]);
                let lead = &next[..next.len() - next.trim_start().len()];
                let lead = match lead.strip_prefix(unit) {
                    Some(rest) if (strip_from..end).contains(&(i + 1)) => rest,
                    _ => lead,
                };
                format!("{lead}\\")
            } else {
                format!("{body} \\")
            };
            if i == 0 {
                self.push(doc::text(text));
                continue;
            }
            let text = match text.strip_prefix(unit) {
                Some(rest) if !body.is_empty() && (strip_from..end).contains(&i) => {
                    rest.to_string()
                }
                _ => text,
            };
            // A comment line stands where it was, at the indentation of the line after it.
            for (_, comment) in comments.iter().filter(|c| c.0 == i) {
                let indent = &text[..text.len() - text.trim_start().len()];
                self.tail.push(doc::hardline());
                self.tail.push(doc::text(format!("{indent}{comment}")));
            }
            self.tail.push(doc::hardline());
            self.tail.push(doc::text(text));
        }
    }

    /// The comment lines of the logical line `source` (code lines, with the comment lines among
    /// them) as `(index of the laid-out line before which it goes, text)`: a comment goes before
    /// the first laid-out line, at or after the one of the comment before it, that starts with
    /// the first two words of the source line after it. `None` when there is no such line.
    pub fn place_comments(source: &str, laid: &str) -> Option<Vec<(usize, String)>> {
        let words = |l: &str| {
            l.split_whitespace()
                .filter(|w| *w != "\\")
                .collect::<Vec<_>>()
                .join(" ")
        };
        let head = |l: &str| l.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        let full: Vec<String> = laid.split('\n').map(words).collect();
        let laid: Vec<String> = laid.split('\n').map(head).collect();
        let lines: Vec<&str> = source.split('\n').collect();
        let (mut at, mut after) = (0, 0);
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate().skip(1) {
            if !line.trim_start().starts_with('#') {
                continue;
            }
            let next = lines[i + 1..]
                .iter()
                .position(|l| !l.trim_start().starts_with('#'))
                .map(|n| i + 1 + n)?;
            let want = head(lines[next]);
            let exact = words(lines[next]);
            // (comments in front of the same line share its place; one in front of another line
            // goes to a later laid line, also when its words are the same)
            let from = if next == after { at } else { at + 1 };
            let start = from.min(laid.len());
            // (the source lines with the same words between the last place and this line come
            // first: a run of blank lines)
            let same = if next == after {
                0
            } else {
                lines[after + 1..next]
                    .iter()
                    .filter(|l| !l.trim_start().starts_with('#') && words(l) == exact)
                    .count()
            };
            // (the line itself first; a line the Shell formatter broke again, by its first words)
            at = start
                + full[start..]
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| **l == exact)
                    .map(|(n, _)| n)
                    .nth(same)
                    .or_else(|| laid[start..].iter().position(|l| *l == want))?;
            after = next;
            out.push((at, line.trim().to_string()));
        }
        Some(out)
    }

    pub fn finish(self) -> Doc {
        doc::concat(vec![
            doc::concat(self.head),
            doc::indent(doc::concat(self.tail)),
        ])
    }
}

/// A heredoc body and its delimiter line as written, at column 0 from the next line break on.
pub fn heredoc(raw: &str) -> Doc {
    doc::line_suffix(doc::line_start(raw))
}

/// What ends the last item of a container: the item's own terminator `existing` (written as is), or,
/// when it has none and the string option `option` is `add`, the fixed text of the `#optional`
/// token `kind` (`;;` ending the last `case` arm). It follows a blank, or a line break with
/// `own_line`. `None`: nothing is written.
pub fn item_terminator(
    cx: &crate::FormatCtx<'_>,
    existing: Option<Doc>,
    (option, kind): (core_config::OptId, core_syntax::SyntaxKind),
    own_line: bool,
) -> Option<Doc> {
    let end = match existing {
        Some(d) => d,
        None if cx.options.is(option, "add") => doc::text(cx.grammar.def(kind)?.text.clone()?),
        None => return None,
    };
    let gap = if own_line {
        doc::hardline()
    } else {
        doc::lit(" ")
    };
    Some(doc::concat(vec![gap, end]))
}

/// Does the heredoc operator of the node that holds `body` (a body or delimiter token) read
/// `dash_op` (`<<-`, whose body and delimiter ignore leading tabs)?
fn is_dash_heredoc(
    body: &core_syntax::SyntaxToken,
    op: core_syntax::SyntaxKind,
    dash_op: &str,
) -> bool {
    body.parent().is_some_and(|p| {
        p.children_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| t.kind() == op && t.text() == dash_op)
    })
}

/// The value of a heredoc body or delimiter token for the meaning check: the text, but of a
/// `dash_op` heredoc without the leading tabs of its lines (a plain heredoc keeps them: they are
/// content).
pub fn heredoc_value<'a>(
    body: &'a core_syntax::SyntaxToken,
    op: core_syntax::SyntaxKind,
    dash_op: &str,
) -> std::borrow::Cow<'a, str> {
    if !is_dash_heredoc(body, op, dash_op) {
        return body.text().into();
    }
    body.text()
        .split('\n')
        .map(|l| l.trim_start_matches('\t'))
        .collect::<Vec<_>>()
        .join("\n")
        .into()
}

/// A heredoc body and delimiter line (`raw`): with the string option `option` set to `reindent`,
/// tab indentation and the operator `op_text` being `dash_op`, [`heredoc_reindented`]; else
/// [`heredoc`].
pub fn heredoc_by_option(
    cx: &crate::FormatCtx<'_>,
    raw: &str,
    (op_text, dash_op): (&str, &str),
    option: core_config::OptId,
) -> Doc {
    if op_text == dash_op
        && cx.options.is(option, "reindent")
        && cx.options.is(core_config::opt::INDENT_STYLE, "tab")
    {
        heredoc_reindented(raw)
    } else {
        heredoc(raw)
    }
}

/// A `<<-` heredoc whose body and delimiter line lose their leading tabs (which `<<-` ignores):
/// the body is indented one level below the line of the command, the delimiter line is as deep.
fn heredoc_reindented(raw: &str) -> Doc {
    let mut lines: Vec<&str> = raw
        .split('\n')
        .map(|l| l.trim_start_matches('\t'))
        .collect();
    let end = lines.pop().unwrap_or_default();
    let body: Vec<Doc> = lines
        .into_iter()
        .flat_map(|l| {
            let line = if l.is_empty() {
                Doc::Nil
            } else {
                Doc::Verbatim(l.to_string())
            };
            [doc::hardline(), line]
        })
        .collect();
    doc::line_suffix(doc::concat(vec![
        doc::indent(doc::concat(body)),
        doc::hardline(),
        doc::text(end),
    ]))
}

/// Text of several lines as written, with no trailing blanks (but after a `\`, which they would
/// turn into a continuation) and no carriage returns; after the first line each starts at column 0
/// (`tab`: the first after a tab).
pub fn as_written(text: &str, tab: bool) -> Doc {
    let mut parts = Vec::new();
    for (i, raw) in text.split('\n').enumerate() {
        let raw = raw.trim_end_matches('\r');
        let trimmed = raw.trim_end_matches([' ', '\t']);
        let line = if trimmed.ends_with('\\') {
            raw
        } else {
            trimmed
        };
        parts.push(match (i, tab) {
            (0, false) => doc::text(line),
            (0, true) => doc::line_start(format!("\t{line}")),
            _ => doc::line_start(line),
        });
    }
    // A last line that ends in `\` continues onto the next line, which stays empty: without it the
    // next statement would be swallowed.
    if text.ends_with('\\') {
        parts.push(doc::line_start(""));
    }
    doc::concat(parts)
}
