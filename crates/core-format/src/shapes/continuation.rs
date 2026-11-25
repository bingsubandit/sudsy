//! Backslash (or backtick) line continuations and heredoc bodies: the layout of one logical line
//! that the source splits over several physical lines (Dockerfile instructions, Makefile
//! statements, shell here-documents).
//!
//! concept: line-continuation-layout
//! what: Line continuations kept at the end of lines.
//! tools: shfmt, hadolint, checkmake

use crate::doc::{self, Doc};
use std::borrow::Cow;

/// One logical line: the parts up to the first continuation, then the indented rest.
#[derive(Default)]
pub struct Continued {
    head: Vec<Doc>,
    tail: Vec<Doc>,
    broken: bool,
    /// The blanks the next physical line starts with, at column 0 (as written).
    next_blanks: Option<String>,
}

impl Continued {
    /// Appends to the current physical line; right after [`Continued::end_line`] a new one starts.
    pub fn push(&mut self, d: Doc) {
        if self.broken {
            self.tail.push(doc::hardline());
            if let Some(blanks) = self.next_blanks.take() {
                self.tail.push(doc::line_start(blanks));
            }
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

    /// [`Continued::end_line`]; the next physical line starts with `blanks` at column 0.
    pub fn end_line_at(&mut self, marker: &str, blanks: String) {
        self.end_line(marker);
        self.next_blanks = Some(blanks);
    }

    /// A line of its own in the indented part (a comment between continued lines).
    pub fn own_line(&mut self, d: Doc) {
        self.push(d);
        self.broken = true;
    }

    /// Text of several lines, a continuation ending each but the last: the first line goes on,
    /// every further line is indented and ends in ` \` (blanks before the escape character
    /// collapse), except inside a quote, where a line stays as written. A line that starts with
    /// `#` is a comment and never opens a quote.
    pub fn text(&mut self, text: &str) {
        let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
        let esc = match lines[0].trim_end().chars().last() {
            Some('`') => '`',
            _ => '\\',
        };
        let mut quote = None::<char>;
        for (i, raw) in lines.iter().enumerate() {
            let verbatim = quote.is_some();
            let line = if verbatim { raw } else { raw.trim_start() };
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
            match (i, verbatim) {
                (0, _) => self.push(doc::text(body)),
                (_, true) => self.tail.push(doc::line_start(body)),
                _ => {
                    self.tail.push(doc::hardline());
                    self.tail.push(doc::text(body));
                }
            }
        }
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
    as_written_with(text, tab, false)
}

/// [`as_written`] for a value that the blanks at its end belong to (a Makefile variable's), and
/// where the blanks before the backslash that continues a line collapse to one (an odd number of
/// backslashes ends the line).
pub fn as_written_value(text: &str) -> Doc {
    as_written_with(text, false, true)
}

/// The line with the blanks before its continuation backslash collapsed to one (an odd number of
/// backslashes continues the line); the line itself when there are none to collapse.
fn condense_line(line: &str) -> Cow<'_, str> {
    let trimmed = line.trim_end_matches([' ', '\t']);
    let slashes = trimmed.len() - trimmed.trim_end_matches('\\').len();
    let before = trimmed[..trimmed.len() - slashes].trim_end_matches([' ', '\t']);
    if slashes % 2 == 1 && !before.is_empty() && before.len() + slashes < trimmed.len() {
        Cow::Owned(format!("{before} {}", &trimmed[trimmed.len() - slashes..]))
    } else {
        Cow::Borrowed(line)
    }
}

/// `text` with the blanks before the backslash of each continued line collapsed to one.
pub fn condensed(text: &str) -> String {
    text.split('\n')
        .map(|l| condense_line(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A line continues when it ends in an odd number of backslashes (a pair is an escaped backslash).
fn continues(line: &str) -> bool {
    (line.len() - line.trim_end_matches('\\').len()) % 2 == 1
}

fn as_written_with(text: &str, tab: bool, condense: bool) -> Doc {
    let mut parts = Vec::new();
    for (i, raw) in text.split('\n').enumerate() {
        let raw = raw.trim_end_matches('\r');
        let trimmed = raw.trim_end_matches([' ', '\t']);
        let line = if continues(trimmed) {
            if condense {
                condense_line(raw)
            } else {
                Cow::Borrowed(raw)
            }
        } else if condense {
            Cow::Borrowed(raw)
        } else {
            Cow::Borrowed(trimmed)
        };
        let line = line.as_ref();
        // (blanks that are kept at the end of the text are written verbatim: the printer drops the others)
        let text_end = if condense {
            line.trim_end_matches([' ', '\t'])
        } else {
            line
        };
        parts.push(match (i, tab) {
            (0, false) => doc::text(text_end),
            (0, true) => doc::line_start(format!("\t{text_end}")),
            _ => doc::line_start(text_end),
        });
        if text_end.len() < line.len() {
            parts.push(doc::verbatim(&line[text_end.len()..]));
        }
    }
    // A last line that ends in `\` continues onto the next line, which stays empty: without it the
    // next statement would be swallowed.
    if continues(text) {
        parts.push(doc::line_start(""));
    }
    doc::concat(parts)
}
