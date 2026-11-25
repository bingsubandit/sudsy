//! Backslash (or backtick) line continuations and heredoc bodies: the layout of one logical line
//! that the source splits over several physical lines (Dockerfile instructions, Makefile
//! statements, shell here-documents).
//!
//! concept: line-continuation-layout
//! what: Line continuations kept at the end of lines.
//! tools: shfmt, hadolint, checkmake

use crate::doc::{self, Doc};

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

    /// Text of several lines that already carries its own line breaks and indentation (code laid
    /// out by another language's formatter), kept exactly: it goes on from the current line.
    pub fn verbatim(&mut self, text: &str) {
        self.push(doc::verbatim(text));
    }

    pub fn finish(self) -> Doc {
        doc::concat(vec![
            doc::concat(self.head),
            doc::indent(doc::concat(self.tail)),
        ])
    }
}

/// The width given to the formatter of code embedded in a continued instruction: its layout is
/// the source's line breaks, never a wrap, so any valid width does.
pub const EMBED_WIDTH: usize = 80;

/// The code of an instruction whose line breaks are continuations, as the embedded formatter
/// reads it: without the continuation that starts it (`RUN \` and the command on the next line)
/// and without carriage returns.
pub fn continued_code(text: &str) -> std::borrow::Cow<'_, str> {
    let mut code = text;
    while let Some(rest) = code
        .strip_prefix("\\\n")
        .or_else(|| code.strip_prefix("\\\r\n"))
    {
        code = rest.trim_start();
    }
    if code.contains('\r') {
        code.replace('\r', "").into()
    } else {
        code.into()
    }
}

/// What the host hands the shell: [`continued_code`] without the empty lines (Docker drops them
/// from a continued instruction, also inside a quote).
pub fn read_lines(text: &str) -> String {
    let code = continued_code(text);
    let mut out = String::with_capacity(code.len());
    for (i, line) in code.split('\n').enumerate() {
        if i > 0 && line.trim().is_empty() {
            continue;
        }
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

/// The code of a continued instruction laid out by the formatter of `language` in
/// [`EmbedMode::Continued`]: every line break a continuation written by that formatter, the
/// comment lines (which the host drops before the code is read) in the tree as comments and
/// printed on lines of their own. `forward` names the host options the embedded formatter takes
/// over (indentation, layout choices). `None`: the text stays as written (no formatter, code that
/// does not parse, or a comment line that did not come out as a line of its own).
pub fn embed_continued(
    cx: &crate::FormatCtx<'_>,
    language: &str,
    text: &str,
    forward: &[core_config::OptId],
) -> Option<String> {
    let code = continued_code(text);
    let laid = cx.embed_with(
        language,
        crate::EmbedMode::Continued,
        &code,
        (EMBED_WIDTH, false),
        forward,
    )?;
    let marker = cx.grammar.comments.line.as_deref()?;
    let same = !code.contains(marker) && !laid.contains(marker)
        || comment_lines(&code, marker).eq(comment_lines(&laid, marker));
    same.then_some(laid)
}

/// The comment lines of continued code: the lines after the first that start with `marker`.
fn comment_lines<'a>(text: &'a str, marker: &'a str) -> impl Iterator<Item = &'a str> {
    let lines = text.split('\n').skip(1).map(str::trim);
    lines.filter(move |l| l.starts_with(marker))
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
